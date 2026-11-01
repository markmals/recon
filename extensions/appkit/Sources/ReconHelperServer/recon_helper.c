extern int recon_server_main(const char *socket_path);

__attribute__((visibility("default")))
int recon_helper_main(const char *socket_path) {
    return recon_server_main(socket_path);
}
