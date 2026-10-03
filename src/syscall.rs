//! System call dispatch — see `userland/abi/syscall_numbers.json`.

use crate::{ipc, keyboard, scheduler, security, serial, vfs};
use core::slice;

pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_YIELD: u64 = 3;
pub const SYS_OPEN: u64 = 5;
pub const SYS_CLOSE: u64 = 6;
pub const SYS_IPC_SEND: u64 = 24;
pub const SYS_IPC_RECV: u64 = 25;
pub const SYS_GETPID: u64 = 39;
pub const SYS_EXIT: u64 = 60;

pub const ENOSYS: i64 = 38;
pub const EINVAL: i64 = 22;
pub const EFAULT: i64 = 14;

const FD_SERIAL: u64 = 1;
const FD_VGA: u64 = 2;
const FD_STDIN: u64 = 0;

extern "C" {
    fn syscall_entry();
}

pub fn init() {
    unsafe {
        crate::cpu::init_syscall_msrs(
            syscall_entry as u64,
            crate::gdt::KERNEL_CODE_SELECTOR,
            crate::gdt::USER_CODE_SELECTOR,
            crate::gdt::USER_DATA_SELECTOR,
        );
    }
    serial_println!("[ ok ] syscalls enabled (amd64 syscall/sysret)");
}

#[no_mangle]
pub extern "C" fn syscall_dispatch(
    num: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> i64 {
    let _ = (a4, a5, a6);
    match num {
        SYS_WRITE => sys_write(a1, a2, a3),
        SYS_READ => sys_read(a1, a2, a3),
        SYS_YIELD => sys_yield(),
        SYS_OPEN => sys_open(a1),
        SYS_CLOSE => sys_close(a1),
        SYS_IPC_SEND => sys_ipc_send(a1, a2),
        SYS_IPC_RECV => sys_ipc_recv(a1, a2),
        SYS_GETPID => sys_getpid(),
        SYS_EXIT => sys_exit(a1),
        _ => -ENOSYS,
    }
}

fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> i64 {
    if security::verify_user_buffer(buf_ptr, len).is_err() {
        return -EFAULT;
    }
    if len == 0 {
        return 0;
    }
    let buf = unsafe { slice::from_raw_parts(buf_ptr as *const u8, len as usize) };
    match fd {
        FD_SERIAL => {
            for &b in buf {
                serial::write_byte(b);
            }
            len as i64
        }
        FD_VGA => {
            for &b in buf {
                print!("{}", b as char);
            }
            len as i64
        }
        _ => -EINVAL,
    }
}

fn sys_read(fd: u64, buf_ptr: u64, len: u64) -> i64 {
    if security::verify_user_buffer(buf_ptr, len).is_err() {
        return -EFAULT;
    }
    if len == 0 {
        return 0;
    }
    match fd {
        FD_STDIN => match keyboard::pop_char() {
            Some(ch) => {
                unsafe {
                    *(buf_ptr as *mut u8) = ch as u8;
                }
                1
            }
            None => 0,
        },
        _ => {
            let buf = unsafe { slice::from_raw_parts_mut(buf_ptr as *mut u8, len as usize) };
            match vfs::read(fd as u32, buf) {
                Ok(n) => n as i64,
                Err(e) => e,
            }
        }
    }
}

fn sys_open(path_ptr: u64) -> i64 {
    if path_ptr == 0 {
        return -EFAULT;
    }
    let path = match user_cstr(path_ptr) {
        Some(p) => p,
        None => return -EFAULT,
    };
    match vfs::open(path) {
        Ok(fd) => fd as i64,
        Err(e) => e,
    }
}

fn sys_close(fd: u64) -> i64 {
    match vfs::close(fd as u32) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn sys_ipc_send(buf_ptr: u64, len: u64) -> i64 {
    if security::verify_user_buffer(buf_ptr, len).is_err() {
        return -EFAULT;
    }
    let buf = unsafe { slice::from_raw_parts(buf_ptr as *const u8, len as usize) };
    match ipc::send(buf) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn sys_ipc_recv(buf_ptr: u64, len: u64) -> i64 {
    if security::verify_user_buffer(buf_ptr, len).is_err() {
        return -EFAULT;
    }
    let buf = unsafe { slice::from_raw_parts_mut(buf_ptr as *mut u8, len as usize) };
    match ipc::recv(buf) {
        Ok(n) => n as i64,
        Err(e) => e,
    }
}

fn sys_yield() -> i64 {
    crate::cpu::hlt();
    0
}

fn sys_getpid() -> i64 {
    scheduler::current_pid().map(i64::from).unwrap_or(-1)
}

fn sys_exit(code: u64) -> i64 {
    serial_println!("[xdos] process exit {}", code);
    crate::cpu::halt_loop();
}

fn user_cstr(ptr: u64) -> Option<&'static str> {
    unsafe {
        let mut len = 0usize;
        while *(ptr as *const u8).add(len) != 0 {
            len += 1;
            if len > 256 {
                return None;
            }
        }
        let bytes = slice::from_raw_parts(ptr as *const u8, len);
        core::str::from_utf8(bytes).ok()
    }
}
