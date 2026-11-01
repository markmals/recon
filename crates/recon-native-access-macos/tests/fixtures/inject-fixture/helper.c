#include <stdio.h>
#include <unistd.h>

__attribute__((visibility("default"))) void recon_helper_main(const char *report_path) {
  if (report_path == NULL) return;
  FILE *report = fopen(report_path, "w");
  if (report == NULL) return;
  fprintf(report, "%d\n", getpid());
  fclose(report);
}
