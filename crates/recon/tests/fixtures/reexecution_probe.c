#include <errno.h>
#include <signal.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static int reexecuted;

static void emit(int descriptor, unsigned char byte) {
    unsigned char block[4096];
    for (unsigned i = 0; i < sizeof(block); ++i) block[i] = byte;
    for (unsigned i = 0; i < 256; ++i) {
        unsigned offset = 0;
        while (offset < sizeof(block)) {
            ssize_t written = write(descriptor, block + offset, sizeof(block) - offset);
            if (written < 0 && errno == EINTR) continue;
            if (written <= 0) _exit(99);
            offset += (unsigned)written;
        }
    }
}

__attribute__((constructor)) static void prepare(void) {
    reexecuted = getenv("RECON_RESPONSIBILITY_DISCLAIMED") != NULL;
    if (reexecuted && getenv("RECON_TEST_CHILD_SIGNAL") != NULL) {
        kill(getpid(), SIGTERM);
        _exit(99);
    }
    if (reexecuted && getenv("RECON_TEST_LARGE_CHILD_OUTPUT") != NULL) {
        emit(STDOUT_FILENO, 'o');
        emit(STDERR_FILENO, 'e');
        _exit(7);
    }
}

static pid_t observed_wait(pid_t child, int *status, int options) {
    pid_t waited = waitpid(child, status, options);
    if (!reexecuted && waited > 0 && getenv("RECON_TEST_FAIL_WAIT") != NULL) {
        errno = ECHILD;
        return -1;
    }
    return waited;
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} wait_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)&observed_wait, (const void *)&waitpid
};
