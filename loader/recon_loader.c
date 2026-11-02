#include <bsm/libbsm.h>
#include <dlfcn.h>
#include <errno.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <mach-o/dyld.h>
#include <mach-o/dyld_images.h>
#include <mach-o/loader.h>
#include <libproc.h>
#include <ptrauth.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/fileport.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
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

// Bind the helper's endpoint at `path` without listening, or return -1. The helper listens once it
// claims the socket, so a client that connects first is refused and retries rather than queuing on
// a listener the target does not own. Nothing but the directory protects the endpoint before it
// exists, so that directory must be private to this user and the path must be new.
static int bind_endpoint(const char *path) {
  struct sockaddr_un address = {0};
  address.sun_family = AF_UNIX;
  if (strlen(path) >= sizeof(address.sun_path)) return -1;
  strlcpy(address.sun_path, path, sizeof(address.sun_path));

  char directory[MAX_PATH] = {0};
  strlcpy(directory, path, sizeof(directory));
  char *slash = strrchr(directory, '/');
  if (slash == NULL || slash == directory) return -1;
  *slash = '\0';
  struct stat status;
  if (lstat(directory, &status) != 0 || !S_ISDIR(status.st_mode) || status.st_uid != geteuid() ||
      (status.st_mode & 0777) != 0700) return -1;
  if (lstat(path, &status) == 0 || errno != ENOENT) return -1;

  int endpoint = socket(AF_UNIX, SOCK_STREAM, 0);
  if (endpoint < 0) return -1;
  if (bind(endpoint, (struct sockaddr *)&address, sizeof(address)) != 0) {
    close(endpoint);
    return -1;
  }
  if (chmod(path, 0600) != 0 || lstat(path, &status) != 0 || !S_ISSOCK(status.st_mode) ||
      status.st_uid != geteuid()) {
    close(endpoint);
    unlink(path);
    return -1;
  }
  return endpoint;
}

// Queue `endpoint` as a fileport on a new receive right in the target and return that right's name
// there, which the helper's entry point receives. The target may be sandboxed and unable to create a
// socket where Recon looks for one, but it can claim and listen on one it is given.
static kern_return_t hand_over(mach_port_t task, int endpoint, mach_port_name_t *remote) {
  mach_port_t fileport = MACH_PORT_NULL;
  if (fileport_makeport(endpoint, &fileport) != 0) return KERN_FAILURE;
  kern_return_t status = mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, remote);
  mach_port_t send = MACH_PORT_NULL;
  mach_msg_type_name_t acquired = 0;
  if (status == KERN_SUCCESS) {
    status = mach_port_extract_right(task, *remote, MACH_MSG_TYPE_MAKE_SEND, &send, &acquired);
  }
  if (status == KERN_SUCCESS) {
    struct {
      mach_msg_header_t header;
      mach_msg_body_t body;
      mach_msg_port_descriptor_t endpoint;
    } message;
    memset(&message, 0, sizeof(message));
    message.header.msgh_bits =
        MACH_MSGH_BITS_SET(MACH_MSG_TYPE_MOVE_SEND, 0, 0, MACH_MSGH_BITS_COMPLEX);
    message.header.msgh_size = sizeof(message);
    message.header.msgh_remote_port = send;
    message.body.msgh_descriptor_count = 1;
    message.endpoint.name = fileport;
    message.endpoint.disposition = MACH_MSG_TYPE_MOVE_SEND;
    message.endpoint.type = MACH_MSG_PORT_DESCRIPTOR;
    status = mach_msg(&message.header, MACH_SEND_MSG | MACH_SEND_TIMEOUT, sizeof(message), 0,
        MACH_PORT_NULL, 1000, MACH_PORT_NULL);
    if (status == KERN_SUCCESS) return KERN_SUCCESS;
    mach_port_deallocate(mach_task_self(), send);
  }
  mach_port_deallocate(mach_task_self(), fileport);
  if (*remote != MACH_PORT_NULL) mach_port_mod_refs(task, *remote, MACH_PORT_RIGHT_RECEIVE, -1);
  *remote = MACH_PORT_NULL;
  return status;
}

// Undo a handover the loader will not start: destroying the receive right releases the queued
// endpoint, and the socket path goes with it.
static void withdraw(mach_port_t task, mach_port_name_t remote, const char *path) {
  mach_port_mod_refs(task, remote, MACH_PORT_RIGHT_RECEIVE, -1);
  unlink(path);
}

int main(int argc, char **argv) {
  if (argc != 6) {
    fprintf(stderr, "usage: %s PID PIDVERSION HELPER_DYLIB SOCKET_PATH EXPECTED_EXECUTABLE\n", argv[0]);
    return 64;
  }
  char *end = NULL;
  long pid = strtol(argv[1], &end, 10);
  if (*argv[1] == '\0' || *end != '\0' || pid <= 0 || pid > INT32_MAX) return 64;
  long pidversion = strtol(argv[2], &end, 10);
  if (*argv[2] == '\0' || *end != '\0' || pidversion < 0 || pidversion > UINT32_MAX) return 64;
  if (strlen(argv[3]) >= MAX_PATH || strlen(argv[4]) >= MAX_PATH || strlen(argv[5]) >= MAX_PATH) return 64;

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
  char observed_executable[MAX_PATH] = {0};
  if (proc_pidpath((pid_t)pid, observed_executable, sizeof(observed_executable)) <= 0 ||
      strcmp(observed_executable, argv[5]) != 0) {
    fprintf(stderr, "the bound process is no longer running the selected executable\n");
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

  int endpoint = bind_endpoint(argv[4]);
  if (endpoint < 0) {
    fprintf(stderr, "the helper endpoint could not be bound in its private directory\n");
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }
  mach_port_name_t handoff = MACH_PORT_NULL;
  status = hand_over(task, endpoint, &handoff);
  close(endpoint);
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "the helper endpoint could not be handed to the target: %d\n", status);
    unlink(argv[4]);
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }
  char handoff_name[16] = {0};
  snprintf(handoff_name, sizeof(handoff_name), "%u", handoff);

  const char *entry_name = "recon_helper_main";
  size_t strings = strlen(argv[3]) + 1 + strlen(entry_name) + 1 + strlen(handoff_name) + 1;
  size_t storage_size = ((STACK_SIZE + sizeof(context) + strings + LOADER_PAGE_SIZE - 1) / LOADER_PAGE_SIZE) * LOADER_PAGE_SIZE;
  mach_vm_address_t code = 0, storage = 0;
  status = mach_vm_allocate(task, &code, LOADER_PAGE_SIZE, VM_FLAGS_ANYWHERE);
  if (status == KERN_SUCCESS) {
    status = mach_vm_allocate(task, &storage, storage_size, VM_FLAGS_ANYWHERE);
  }
  if (status != KERN_SUCCESS) {
    fprintf(stderr, "target allocation failed: %d\n", status);
    release_regions(task, code, storage, storage_size);
    withdraw(task, handoff, argv[4]);
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
    status = mach_vm_write(task, context.argument, (vm_offset_t)handoff_name, strlen(handoff_name) + 1);
  }
  if (status == KERN_SUCCESS) {
    status = mach_vm_protect(task, code, LOADER_PAGE_SIZE, FALSE, VM_PROT_READ | VM_PROT_EXECUTE);
  }
  if (status != KERN_SUCCESS || !same_incarnation(task, expected)) {
    fprintf(stderr, "target write, protection, or incarnation validation failed\n");
    release_regions(task, code, storage, storage_size);
    withdraw(task, handoff, argv[4]);
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
    withdraw(task, handoff, argv[4]);
    mach_port_deallocate(mach_task_self(), task);
    return 1;
  }

  printf("{\"pid\":%ld,\"pidversion\":%ld,\"architecture\":\"%s\",\"started\":true}\n",
      pid, pidversion, arm64e ? "arm64e" : "arm64");
  mach_port_deallocate(mach_task_self(), task);
  return 0;
}
