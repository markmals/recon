#include <CoreFoundation/CoreFoundation.h>
#include <Security/SecBase.h>
#include <Security/SecCode.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <string.h>
#include <unistd.h>

extern int csops(pid_t pid, unsigned int operation, void *buffer, size_t length);

static unsigned char kernel_hash[20];
static int have_kernel_hash;
static atomic_int paused;

static int selected(const char *mode) {
    const char *actual = getenv("RECON_SIGNING_TEST_MODE");
    return actual != NULL && strcmp(actual, mode) == 0;
}

static int is_recon(void) {
    return strcmp(getprogname(), "recon") == 0;
}

static void record_query(pid_t pid, unsigned int operation, size_t length) {
    const char *path = getenv("RECON_SIGNING_TEST_TRACE");
    if (path == NULL) return;
    int trace = open(path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (trace >= 0) {
        dprintf(trace, "%d %d %u %zu\n", pid, getpid(), operation, length);
        close(trace);
    }
}

static int observed_csops(pid_t pid, unsigned int operation, void *buffer, size_t length) {
    record_query(pid, operation, length);
    if (pid != getpid() || (operation != 0 && operation != 5)) {
        errno = EACCES;
        return -1;
    }
    if ((operation == 0 && selected("status-eio"))
        || (operation == 5 && selected("hash-eio"))) {
        errno = EIO;
        return -1;
    }
    if (operation == 5 && selected("hash-ebadexec")) {
        errno = EBADEXEC;
        return -1;
    }
    if (operation == 5 && selected("unsigned-status")) {
        errno = EIO;
        return -1;
    }
    int status = csops(pid, operation, buffer, length);
    if (status == 0 && operation == 0 && length == sizeof(uint32_t)) {
        uint32_t flags;
        memcpy(&flags, buffer, sizeof(flags));
        if (selected("unsigned-status")) flags &= ~UINT32_C(0x20000000);
        if (selected("invalid-dynamic-status")) flags &= ~UINT32_C(1);
        memcpy(buffer, &flags, sizeof(flags));
    }
    if (status == 0 && operation == 5 && length == sizeof(kernel_hash)) {
        memcpy(kernel_hash, buffer, sizeof(kernel_hash));
        have_kernel_hash = 1;
    }
    return status;
}

static void *observed_dlsym(void *handle, const char *name) {
    if (is_recon() && strcmp(name, "csops") == 0) {
        return selected("missing-query") ? NULL : (void *)&observed_csops;
    }
    if (is_recon() && selected("missing-window-server")
        && strcmp(name, "CGSMainConnectionID") == 0) return NULL;
    return dlsym(handle, name);
}

/* The barrier is reached by an already-executing image, before its first public signing lookup. */
static void before_signing(void) {
    const char *ready_path = getenv("RECON_SIGNING_TEST_READY");
    const char *release_path = getenv("RECON_SIGNING_TEST_RELEASE");
    if (ready_path == NULL || release_path == NULL) return;
    int ready = open(ready_path, O_WRONLY);
    if (ready < 0 || write(ready, "R", 1) != 1) _exit(90);
    close(ready);
    int release = open(release_path, O_RDONLY);
    char marker;
    if (release < 0 || read(release, &marker, 1) != 1 || marker != 'G') _exit(91);
    close(release);
}

static OSStatus observed_copy_self(SecCSFlags flags, SecCodeRef *code) {
    if (is_recon()) {
        if (selected("unsigned-status")) return errSecCSSignatureInvalid;
        if (atomic_exchange(&paused, 1) == 0) before_signing();
    }
    return SecCodeCopySelf(flags, code);
}

static void change_metadata(CFMutableDictionaryRef information) {
    if (selected("missing-unique")) CFDictionaryRemoveValue(information, kSecCodeInfoUnique);
    if (selected("invalid-unique")) {
        CFDictionarySetValue(information, kSecCodeInfoUnique, CFSTR("not hash data"));
    }
    if (selected("missing-identifier")) {
        CFDictionaryRemoveValue(information, kSecCodeInfoIdentifier);
    }
    if (selected("missing-flags")) CFDictionaryRemoveValue(information, kSecCodeInfoFlags);
    if (selected("invalid-team")) {
        CFDictionarySetValue(information, kSecCodeInfoTeamIdentifier, kCFBooleanTrue);
    }
    const char *requested_length = getenv("RECON_SIGNING_TEST_HASH_LENGTH");
    const char *flipped_bit = getenv("RECON_SIGNING_TEST_FLIPPED_BIT");
    if (requested_length != NULL || flipped_bit != NULL) {
        CFDataRef original = CFDictionaryGetValue(information, kSecCodeInfoUnique);
        unsigned char bytes[64] = {0};
        if (original != NULL && CFGetTypeID(original) == CFDataGetTypeID()
            && CFDataGetLength(original) == 20) {
            memcpy(bytes, CFDataGetBytePtr(original), 20);
        }
        unsigned length = requested_length == NULL ? 20 : (unsigned)atoi(requested_length);
        if (length > sizeof(bytes)) _exit(92);
        if (flipped_bit != NULL) {
            unsigned bit = (unsigned)atoi(flipped_bit);
            if (bit >= 160) _exit(93);
            bytes[bit / 8] ^= (unsigned char)(1u << (bit % 8));
        }
        CFDataRef replacement = CFDataCreate(NULL, bytes, length);
        CFDictionarySetValue(information, kSecCodeInfoUnique, replacement);
        CFRelease(replacement);
    }
    if (selected("matching-alternate") && have_kernel_hash) {
        CFDataRef own = CFDataCreate(NULL, kernel_hash, sizeof(kernel_hash));
        const void *hashes[] = {own};
        CFArrayRef alternatives = CFArrayCreate(NULL, hashes, 1, &kCFTypeArrayCallBacks);
        CFDictionarySetValue(information, kSecCodeInfoCdHashes, alternatives);
        CFRelease(alternatives);
        CFRelease(own);
    }
}

static OSStatus observed_signing_information(SecStaticCodeRef code, SecCSFlags flags,
                                            CFDictionaryRef *information) {
    if (is_recon() && selected("public-unsigned-error")) return errSecCSUnsigned;
    OSStatus status = SecCodeCopySigningInformation(code, flags, information);
    if (status != errSecSuccess || !is_recon() || information == NULL || *information == NULL) {
        return status;
    }
    CFMutableDictionaryRef changed = CFDictionaryCreateMutableCopy(NULL, 0, *information);
    change_metadata(changed);
    CFRelease(*information);
    *information = changed;
    return status;
}

static OSStatus refused_validity(SecCodeRef code, SecCSFlags flags, SecRequirementRef requirement) {
    (void)code;
    (void)flags;
    (void)requirement;
    return errSecCSSignatureUntrusted;
}

static OSStatus refused_validity_with_errors(SecCodeRef code, SecCSFlags flags,
                                             SecRequirementRef requirement, CFErrorRef *errors) {
    (void)errors;
    return refused_validity(code, flags, requirement);
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} interpositions[] __attribute__((section("__DATA,__interpose"))) = {
    {(const void *)&observed_dlsym, (const void *)&dlsym},
    {(const void *)&observed_copy_self, (const void *)&SecCodeCopySelf},
    {(const void *)&observed_signing_information, (const void *)&SecCodeCopySigningInformation},
    {(const void *)&refused_validity, (const void *)&SecCodeCheckValidity},
    {(const void *)&refused_validity_with_errors, (const void *)&SecCodeCheckValidityWithErrors}
};
