// A cooperative injection target.
//
// The process is signed with `get-task-allow`, so a same-user Recon loader can acquire its task
// control port without the debugger entitlement. That is the "cooperative" profile: the target
// consents to inspection through its own signature rather than relying on a weakened host.
//
// It keeps a live main-thread CoreFoundation run loop so a later injected helper can hop to the
// main thread, and prints its pid so a launcher can target the exact running incarnation.

#include <CoreFoundation/CoreFoundation.h>
#include <stdio.h>
#include <unistd.h>

int main(void) {
    printf("%d\n", getpid());
    fflush(stdout);
    CFRunLoopRun();
    return 0;
}
