#include <bsm/libbsm.h>
#include <dlfcn.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <mach-o/dyld.h>
#include <mach-o/dyld_images.h>
#include <mach-o/loader.h>
#include <ptrauth.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern unsigned char bootstrap[], trampoline[], bootstrap_end[];

#define LOADER_PAGE_SIZE 16384
#define STACK_SIZE 32768
#define MAX_PATH 4096

struct load_context {
  uint64_t pthread_create;
  uint64_t dlopen_function;
  uint64_t dlsym_function;
  uint64_t mach_thread_self_function;
  uint64_t thread_terminate_function;
  uint64_t entry;
  uint64_t pthread_handle;
  uint64_t path;
  uint64_t symbol;
  uint64_t argument;
  uint64_t bootstrap_thread;
  uint64_t authenticated;
};

static int task_identity(mach_port_t task, audit_token_t *token) {
  mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
  return task_info(task, TASK_AUDIT_TOKEN, (task_info_t)token, &count);
}

static int same_incarnation(mach_port_t task, audit_token_t expected) {
  audit_token_t observed = {{0}};
  return task_identity(task, &observed) == KERN_SUCCESS &&
      memcmp(&observed, &expected, sizeof(observed)) == 0;
}

static uint64_t raw_symbol(const char *name) {
  void *address = dlsym(RTLD_DEFAULT, name);
#if __has_feature(ptrauth_calls)
  address = ptrauth_strip(address, ptrauth_key_function_pointer);
#endif
  return (uint64_t)(uintptr_t)address;
}

static int remote_read(mach_port_t task, uint64_t address, void *bytes, size_t size) {
  mach_vm_size_t actual = 0;
  kern_return_t status = mach_vm_read_overwrite(
      task, address, size, (mach_vm_address_t)bytes, &actual);
  return status == KERN_SUCCESS && actual == size ? KERN_SUCCESS : KERN_FAILURE;
}

static int image_infos(mach_port_t task, struct dyld_all_image_infos *infos) {
  struct task_dyld_info location = {0};
  mach_msg_type_number_t count = TASK_DYLD_INFO_COUNT;
  int status = task_info(task, TASK_DYLD_INFO, (task_info_t)&location, &count);
  if (status != KERN_SUCCESS) return status;
  return remote_read(task, location.all_image_info_addr, infos, sizeof(*infos));
}

static int shared_cache_matches(mach_port_t task) {
  struct dyld_all_image_infos own = {0}, remote = {0};
  return image_infos(mach_task_self(), &own) == KERN_SUCCESS &&
      image_infos(task, &remote) == KERN_SUCCESS && remote.version >= 15 &&
      remote.libSystemInitialized &&
      memcmp(own.sharedCacheUUID, remote.sharedCacheUUID, 16) == 0 &&
      own.sharedCacheSlide == remote.sharedCacheSlide;
}

static int target_is_arm64e(mach_port_t task, int *arm64e) {
  struct task_dyld_info location = {0};
  mach_msg_type_number_t count = TASK_DYLD_INFO_COUNT;
  int status = task_info(task, TASK_DYLD_INFO, (task_info_t)&location, &count);
  if (status != KERN_SUCCESS) return status;

  struct dyld_all_image_infos infos = {0};
  struct dyld_image_info image = {0};
  struct mach_header_64 header = {0};
  status = remote_read(task, location.all_image_info_addr, &infos, sizeof(infos));
  if (status != KERN_SUCCESS) return status;
  status = remote_read(task, (uint64_t)infos.infoArray, &image, sizeof(image));
  if (status != KERN_SUCCESS) return status;
  status = remote_read(task, (uint64_t)image.imageLoadAddress, &header, sizeof(header));
  if (status != KERN_SUCCESS) return status;
  if (header.magic != MH_MAGIC_64 || header.cputype != CPU_TYPE_ARM64) return KERN_FAILURE;
  *arm64e = (header.cpusubtype & ~CPU_SUBTYPE_MASK) == CPU_SUBTYPE_ARM64E;
  return KERN_SUCCESS;
}

static kern_return_t start_thread(mach_port_t task, uint64_t entry, uint64_t stack,
    uint64_t context, mach_port_t *thread) {
  kern_return_t status = thread_create(task, thread);
  if (status != KERN_SUCCESS) return status;

#if __has_feature(ptrauth_calls)
  arm_unified_thread_state_t state = {0};
  state.ash.flavor = ARM_THREAD_STATE64;
  state.ash.count = ARM_THREAD_STATE64_COUNT;
  state.ts_64.__x[0] = context;
  __darwin_arm_thread_state64_set_sp(state.ts_64, stack);
  void *signed_pc = ptrauth_sign_unauthenticated((void *)(uintptr_t)entry,
      ptrauth_key_process_independent_code, ptrauth_string_discriminator("pc"));
  __darwin_arm_thread_state64_set_pc_presigned_fptr(state.ts_64, signed_pc);

  arm_unified_thread_state_t converted = {0};
  mach_msg_type_number_t count = ARM_UNIFIED_THREAD_STATE_COUNT;
  status = thread_convert_thread_state(*thread, THREAD_CONVERT_THREAD_STATE_FROM_SELF,
      ARM_UNIFIED_THREAD_STATE, (thread_state_t)&state, ARM_UNIFIED_THREAD_STATE_COUNT,
      (thread_state_t)&converted, &count);
  if (status == KERN_SUCCESS) {
    status = thread_set_state(*thread, ARM_UNIFIED_THREAD_STATE,
        (thread_state_t)&converted, count);
  }
#else
  arm_thread_state64_t state = {0};
  state.__x[0] = context;
  __darwin_arm_thread_state64_set_sp(state, stack);
  __darwin_arm_thread_state64_set_pc_fptr(state, (void *)(uintptr_t)entry);
  status = thread_set_state(*thread, ARM_THREAD_STATE64,
      (thread_state_t)&state, ARM_THREAD_STATE64_COUNT);
#endif
  return status;
}

static void release_regions(mach_port_t task, mach_vm_address_t code,
    mach_vm_address_t storage, mach_vm_size_t storage_size) {
  if (code) mach_vm_deallocate(task, code, LOADER_PAGE_SIZE);
  if (storage) mach_vm_deallocate(task, storage, storage_size);
}

int main(int argc, char **argv) {
  if (argc != 5) {
    fprintf(stderr, "usage: %s PID PIDVERSION HELPER_DYLIB SOCKET_PATH\n", argv[0]);
    return 64;
  }
  char *end = NULL;
  long pid = strtol(argv[1], &end, 10);
  if (*argv[1] == '\0' || *end != '\0' || pid <= 0 || pid > INT32_MAX) return 64;
  long pidversion = strtol(argv[2], &end, 10);
  if (*argv[2] == '\0' || *end != '\0' || pidversion < 0 || pidversion > UINT32_MAX) return 64;
  if (strlen(argv[3]) >= MAX_PATH || strlen(argv[4]) >= MAX_PATH) return 64;

  mach_port_t task = MACH_PORT_NULL;
  kern_return_t status = task_for_pid(mach_task_self(), (pid_t)pid, &task);
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "task_for_pid failed: %d\n", status);
    return 1;
  }

  audit_token_t expected = {{0}};
  status = task_identity(task, &expected);
  if (status != KERN_SUCCESS || audit_token_to_pid(expected) != pid ||
      audit_token_to_pidversion(expected) != (int)pidversion) {
    fprintf(stderr, "target incarnation does not match the selected process\n");
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }
  if (!shared_cache_matches(task)) {
    fprintf(stderr, "target does not share the loader's dyld shared cache\n");
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  int arm64e = 0;
  status = target_is_arm64e(task, &arm64e);
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "target architecture is unavailable or unsupported\n");
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  struct load_context context = {0};
  context.pthread_create = raw_symbol("pthread_create_from_mach_thread");
  context.dlopen_function = raw_symbol("dlopen");
  context.dlsym_function = raw_symbol("dlsym");
  context.mach_thread_self_function = raw_symbol("mach_thread_self");
  context.thread_terminate_function = raw_symbol("thread_terminate");
  context.authenticated = (uint64_t)arm64e;
  if (!context.pthread_create || !context.dlopen_function || !context.dlsym_function ||
      !context.mach_thread_self_function || !context.thread_terminate_function) {
    fprintf(stderr, "required libSystem symbols are unavailable\n");
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  const char *entry_name = "recon_helper_main";
  size_t strings = strlen(argv[3]) + 1 + strlen(entry_name) + 1 + strlen(argv[4]) + 1;
  size_t storage_size = ((STACK_SIZE + sizeof(context) + strings + LOADER_PAGE_SIZE - 1) / LOADER_PAGE_SIZE) * LOADER_PAGE_SIZE;
  mach_vm_address_t code = 0, storage = 0;
  status = mach_vm_allocate(task, &code, LOADER_PAGE_SIZE, VM_FLAGS_ANYWHERE);
  if (status == KERN_SUCCESS) {
    status = mach_vm_allocate(task, &storage, storage_size, VM_FLAGS_ANYWHERE);
  }
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "target allocation failed: %d\n", status);
    release_regions(task, code, storage, storage_size);
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  uint64_t context_address = storage + STACK_SIZE;
  context.entry = code + (trampoline - bootstrap);
  context.path = context_address + sizeof(context);
  context.symbol = context.path + strlen(argv[3]) + 1;
  context.argument = context.symbol + strlen(entry_name) + 1;

  size_t code_size = bootstrap_end - bootstrap;
  status = mach_vm_write(task, code, (vm_offset_t)bootstrap, (mach_msg_type_number_t)code_size);
  if (status == KERN_SUCCESS) {
    status = mach_vm_write(task, context_address, (vm_offset_t)&context, sizeof(context));
  }
  if (status == KERN_SUCCESS) {
    status = mach_vm_write(task, context.path, (vm_offset_t)argv[3], strlen(argv[3]) + 1);
  }
  if (status == KERN_SUCCESS) {
    status = mach_vm_write(task, context.symbol, (vm_offset_t)entry_name, strlen(entry_name) + 1);
  }
  if (status == KERN_SUCCESS) {
    status = mach_vm_write(task, context.argument, (vm_offset_t)argv[4], strlen(argv[4]) + 1);
  }
  if (status == KERN_SUCCESS) {
    status = mach_vm_protect(task, code, LOADER_PAGE_SIZE, FALSE, VM_PROT_READ | VM_PROT_EXECUTE);
  }
  if (status != KERN_SUCCESS || !same_incarnation(task, expected)) {
    fprintf(stderr, "target write, protection, or incarnation validation failed\n");
    release_regions(task, code, storage, storage_size);
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  mach_port_t thread = MACH_PORT_NULL;
  status = start_thread(task, code, context_address, context_address, &thread);
  if (status == KERN_SUCCESS && same_incarnation(task, expected)) {
    status = thread_resume(thread);
  } else if (status == KERN_SUCCESS) {
    status = KERN_FAILURE;
  }
  if (thread != MACH_PORT_NULL) {
    if (status != KERN_SUCCESS) thread_terminate(thread);
    mach_port_deallocate(mach_task_self(), thread);
  }
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "target thread start failed: %d\n", status);
    release_regions(task, code, storage, storage_size);
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  printf("{\"pid\":%ld,\"pidversion\":%ld,\"architecture\":\"%s\",\"started\":true}\n",
      pid, pidversion, arm64e ? "arm64e" : "arm64");
  mach_port_deallocate(mach_task_self(), task);
  return 0;
}
