#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int selected_descriptor = -1;
static int stage;
static const char *accepted_marker;
static struct stat selected_identity;
static int identified;

__attribute__((constructor)) static void prepare(void) {
    if (getenv("RECON_RESPONSIBILITY_DISCLAIMED") != NULL) return;
    if (strcmp(getprogname(), "recon") != 0) return;
    const char *descriptor = getenv("RECON_TEST_OUTPUT_DESCRIPTOR");
    if (descriptor != NULL) selected_descriptor = atoi(descriptor);
    accepted_marker = getenv("RECON_TEST_ACCEPTED_PREFIX");
    identified = selected_descriptor >= 0 && fstat(selected_descriptor, &selected_identity) == 0;
}

static void record(unsigned char byte, int flags) {
    int marker = open(accepted_marker, O_WRONLY | O_CREAT | flags, 0600);
    if (marker >= 0) {
        (void)write(marker, &byte, sizeof(byte));
        close(marker);
    }
}

static int same_sink(int descriptor) {
    if (!identified) return 0;
    if (descriptor == selected_descriptor) return 1;
    struct stat identity;
    return fstat(descriptor, &identity) == 0
        && identity.st_dev == selected_identity.st_dev
        && identity.st_ino == selected_identity.st_ino
        && identity.st_mode == selected_identity.st_mode;
}

/* The sink recovers after one failure, making a retry or a second diagnostic observable. */
static ssize_t observed_write(int descriptor, const void *bytes, size_t length) {
    if (!same_sink(descriptor) || length == 0) return write(descriptor, bytes, length);
    if (stage == 0) {
        size_t prefix = length < 16 ? length : 16;
        ssize_t accepted = write(descriptor, bytes, prefix);
        if (accepted > 0) {
            stage = 1;
            record((unsigned char)accepted, O_TRUNC);
        }
        return accepted;
    }
    if (stage == 1) {
        stage = 2;
        record(0, O_APPEND);
        errno = EIO;
        return -1;
    }
    return write(descriptor, bytes, length);
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} write_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)&observed_write, (const void *)&write
};
