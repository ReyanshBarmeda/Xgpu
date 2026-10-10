//! PS/2 auxiliary device (mouse) — polled packets.

use crate::cpu::{inb, outb, io_wait};
use crate::sync::Mutex;

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const CMD: u16 = 0x64;

#[derive(Clone, Copy, Debug, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
}

struct Inner {
    packet: [u8; 3],
    idx: usize,
}

static MOUSE: Mutex<Inner> = Mutex::new(Inner {
    packet: [0; 3],
    idx: 0,
});
static STATE: Mutex<MouseState> = Mutex::new(MouseState {
    x: 0,
    y: 0,
    buttons: 0,
});

unsafe fn wait_write() {
    for _ in 0..100_000 {
        if inb(STATUS) & 2 == 0 {
            return;
        }
    }
}

unsafe fn write_cmd(b: u8) {
    wait_write();
    outb(CMD, b);
}

unsafe fn write_data(b: u8) {
    wait_write();
    outb(DATA, b);
}

pub fn init() {
    unsafe {
        write_cmd(0xA8); // enable aux port
        write_cmd(0x20);
        io_wait();
        let status = inb(DATA);
        write_cmd(0x60);
        write_data(status | 2);
        write_cmd(0xD4);
        write_data(0xF4); // enable streaming
    }
    serial_println!("[ ok ] PS/2 mouse enabled (polled)");
}

pub fn poll() {
    unsafe {
        while inb(STATUS) & 1 != 0 {
            let b = inb(DATA);
            let mut inner = MOUSE.lock();
            inner.packet[inner.idx] = b;
            inner.idx += 1;
            if inner.idx == 3 {
                inner.idx = 0;
                let flags = inner.packet[0];
                let dx = inner.packet[1] as i8 as i32;
                let dy = inner.packet[2] as i8 as i32;
                let mut st = STATE.lock();
                st.x += dx;
                st.y -= dy;
                st.buttons = flags & 0x07;
            }
        }
    }
}

pub fn state() -> MouseState {
    MOUSE.lock(); // drain not required
    *STATE.lock()
}
