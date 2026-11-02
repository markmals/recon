// A stand-in for Recon's helper. It claims the endpoint the loader hands it, as the real helper
// does, and answers every connection with its own pid, so a test can tell which process it reached.

#include <mach/mach.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/fileport.h>
#include <sys/socket.h>
#include <unistd.h>

static int listener = -1;

static void *answer(void *unused) {
  (void)unused;
  for (;;) {
    int client = accept(listener, NULL, NULL);
    if (client < 0) return NULL;
    int enabled = 1;
    setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled));
    char line[32];
    int length = snprintf(line, sizeof(line), "%d\n", getpid());
    write(client, line, (size_t)length);
    close(client);
  }
}

__attribute__((visibility("default"))) int recon_helper_main(const char *argument) {
  if (argument == NULL) return -1;
  mach_port_t port = (mach_port_t)strtoul(argument, NULL, 10);
  struct {
    mach_msg_header_t header;
    mach_msg_body_t body;
    mach_msg_port_descriptor_t endpoint;
    mach_msg_max_trailer_t trailer;
  } message;
  memset(&message, 0, sizeof(message));
  kern_return_t received = mach_msg(&message.header, MACH_RCV_MSG | MACH_RCV_TIMEOUT, 0,
      sizeof(message), port, 0, MACH_PORT_NULL);
  mach_port_mod_refs(mach_task_self(), port, MACH_PORT_RIGHT_RECEIVE, -1);
  if (received != KERN_SUCCESS) return -1;
  listener = fileport_makefd(message.endpoint.name);
  mach_port_deallocate(mach_task_self(), message.endpoint.name);
  if (listener < 0 || listen(listener, 4) != 0) return -1;
  pthread_t thread;
  if (pthread_create(&thread, NULL, answer, NULL) != 0) return -1;
  pthread_detach(thread);
  return 0;
}
