//! Virtual filesystem — RAMFS today; block devices later.

mod ramfs;

use crate::sync::Mutex;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

pub struct Vfs {
    files: BTreeMap<String, Vec<u8>>,
    next_fd: u32,
    open: BTreeMap<u32, OpenFile>,
}

struct OpenFile {
    path: String,
    offset: usize,
}

impl Vfs {
    const fn new() -> Self {
        Vfs {
            files: BTreeMap::new(),
            next_fd: 3,
            open: BTreeMap::new(),
        }
    }

    pub fn mount_builtin(&mut self) {
        ramfs::seed(self);
    }

    pub fn open(&mut self, path: &str) -> Result<u32, i64> {
        if !self.files.contains_key(path) {
            return Err(-2); // ENOENT
        }
        let fd = self.next_fd;
        self.next_fd += 1;
        self.open.insert(
            fd,
            OpenFile {
                path: String::from(path),
                offset: 0,
            },
        );
        Ok(fd)
    }

    pub fn close(&mut self, fd: u32) -> Result<(), i64> {
        if self.open.remove(&fd).is_some() {
            Ok(())
        } else {
            Err(-9) // EBADF
        }
    }

    pub fn read(&mut self, fd: u32, buf: &mut [u8]) -> Result<usize, i64> {
        let open = self.open.get_mut(&fd).ok_or(-9i64)?;
        let data = self.files.get(&open.path).ok_or(-9i64)?;
        if open.offset >= data.len() {
            return Ok(0);
        }
        let n = (data.len() - open.offset).min(buf.len());
        buf[..n].copy_from_slice(&data[open.offset..open.offset + n]);
        open.offset += n;
        Ok(n)
    }

    pub fn write_file(&mut self, path: &str, data: Vec<u8>) {
        self.files.insert(String::from(path), data);
    }
}

static VFS: Mutex<Vfs> = Mutex::new(Vfs::new());

pub fn init() {
    VFS.lock().mount_builtin();
    serial_println!("[ ok ] VFS (ramfs) mounted");
}

pub fn open(path: &str) -> Result<u32, i64> {
    VFS.lock().open(path)
}

pub fn close(fd: u32) -> Result<(), i64> {
    VFS.lock().close(fd)
}

pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, i64> {
    VFS.lock().read(fd, buf)
}
