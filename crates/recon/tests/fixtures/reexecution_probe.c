#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <spawn.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static int reexecuted;

/* Where to record that recon's re-execution really spawned a child, or NULL when no test asked.
   Only a recon that has not re-executed yet records; the child never does. */
static const char *spawn_marker;

/* Which of recon's child-output reader threads must fail to start, counted from the moment recon
   prepares the child's file actions, or 0 to let every thread start. */
static int refused_reader;
static int preparing_child;
static int threads_started;

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
    const char *marker = getenv("RECON_TEST_CHILD_MARKER");
    const char *reader = getenv("RECON_TEST_READER_FAILURE");
    if (reexecuted && marker != NULL) {
        /* A test that proves no child exists has to stay safe when one does: this child does
           nothing at all, and its parent records that it exists. */
        _exit(0);
    }
    /* Only the recon that re-executes itself is a target. The context fallback legitimately runs
       mdfind, which inherits this library, keeps every thread it asks for, and is not a trace. */
    if (!reexecuted && strcmp(getprogname(), "recon") == 0) {
        spawn_marker = marker;
        if (reader != NULL) refused_reader = atoi(reader);
    }
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

/* Whether these are the arguments of recon's disclaimed re-execution: only that child is given
   the marker recon reads back as "you already re-executed". */
static int disclaimed(char *const envp[]) {
    static const char entry[] = "RECON_RESPONSIBILITY_DISCLAIMED=";
    if (envp == NULL) return 0;
    for (unsigned i = 0; envp[i] != NULL; ++i) {
        if (strncmp(envp[i], entry, sizeof(entry) - 1) == 0) return 1;
    }
    return 0;
}

/* The trace of recon's own re-execution, recorded once a child really exists rather than when the
   call was made. The mdfind the context fallback runs is not disclaimed and leaves no trace. */
static int observed_spawn(pid_t *child, const char *path,
                          const posix_spawn_file_actions_t *actions,
                          const posix_spawnattr_t *attributes, char *const argv[],
                          char *const envp[]) {
    int spawned = posix_spawn(child, path, actions, attributes, argv, envp);
    if (spawned == 0 && spawn_marker != NULL && disclaimed(envp)) {
        int marked = open(spawn_marker, O_WRONLY | O_CREAT | O_TRUNC, 0600);
        if (marked >= 0) close(marked);
    }
    return spawned;
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} spawn_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)&observed_spawn, (const void *)&posix_spawn
};

/* recon prepares one set of file actions for the child whose output its readers drain, so that
   call is where counting the threads it starts for that child begins. */
static int observed_file_actions_init(posix_spawn_file_actions_t *actions) {
    if (!reexecuted && refused_reader != 0) preparing_child = 1;
    return posix_spawn_file_actions_init(actions);
}

static int observed_pthread_create(pthread_t *thread, const pthread_attr_t *attributes,
                                   void *(*start)(void *), void *argument) {
    if (preparing_child && ++threads_started == refused_reader) return EAGAIN;
    return pthread_create(thread, attributes, start, argument);
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} file_actions_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)&observed_file_actions_init, (const void *)&posix_spawn_file_actions_init
};

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} thread_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)&observed_pthread_create, (const void *)&pthread_create
};
