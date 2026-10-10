use super::Vfs;

pub fn seed(vfs: &mut Vfs) {
    vfs.write_file("/readme.txt", b"Xdos ramfs — open via syscall open/read.\n".to_vec());
    vfs.write_file("/motd.txt", b"Welcome to userland.\n".to_vec());
}
