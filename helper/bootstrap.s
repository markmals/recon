// The loader's in-target bootstrap, position-independent and free of relocations.
//
// This is the authoritative source for the shellcode loaded by `loader/recon_loader.c`.
// The loader writes these assembled bytes into the target and starts a bare Mach thread at
// `_bootstrap` with x0 pointing at a context it also wrote. Only plain `blr` is used, which is
// valid on arm64 and arm64e alike; pointer authentication for the initial state is the loader's job.
//
// Context layout, one 8-byte slot each:
//   0  pthread_create_from_mach_thread   8  dlopen                16  dlsym
//  24  mach_thread_self                  32  thread_terminate      40  trampoline address
//  48  pthread_t storage                 56  dylib path            64  entry name
//  72  entry argument                    80  bootstrap thread port  88  arm64e target flag
//
// Rebuild after editing and update the embedded bytes:
//   clang -c -arch arm64 helper/bootstrap.s -o /tmp/bootstrap.o && otool -tvj /tmp/bootstrap.o

.text
.align 2
.globl _bootstrap
_bootstrap:
    mov  x19, x0            // keep the context pointer
    ldr  x8, [x19, #24]     // mach_thread_self
    blr  x8
    str  x0, [x19, #80]     // publish our port before the pthread can finish
    ldr  x2, [x19, #40]     // start routine = trampoline
    ldr  x9, [x19, #88]     // allocated code needs an IA signature in arm64e targets
    cbz  x9, 1f
    paciza x2
1:
    add  x0, x19, #48       // &pthread_t storage
    mov  x1, #0             // attr = NULL
    mov  x3, x19            // arg = context
    ldr  x8, [x19, #0]      // pthread_create_from_mach_thread
    blr  x8
Lspin:
    b    Lspin              // the pthread terminates this bare thread after the payload

.align 2
.globl _trampoline
_trampoline:                // pthread start routine; x0 = context pointer
    stp  x19, x30, [sp, #-16]!
    mov  x19, x0
    ldr  x0, [x19, #56]     // dylib path
    mov  x1, #2             // RTLD_NOW
    ldr  x8, [x19, #8]      // dlopen
    blr  x8
    cbz  x0, Ldone          // dlopen failed
    ldr  x1, [x19, #64]     // entry name (x0 already holds the handle)
    ldr  x8, [x19, #16]     // dlsym
    blr  x8
    cbz  x0, Ldone          // symbol missing
    mov  x8, x0             // entry function
    xpaci x8                // dlsym returns a signed pointer on arm64e
    ldr  x0, [x19, #72]     // entry argument
    blr  x8
Ldone:
    ldr  w0, [x19, #80]     // the pthread has TLS, unlike the bare Mach thread
    ldr  x8, [x19, #32]     // thread_terminate
    blr  x8
    mov  x0, #0
    ldp  x19, x30, [sp], #16
    ret

.globl _bootstrap_end
_bootstrap_end:
