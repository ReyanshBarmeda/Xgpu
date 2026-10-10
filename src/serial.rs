//! 16550 UART driver for COM1.
//!
//! The Makefile runs QEMU with `-serial stdio`, so anything written here
//! shows up in your terminal. That makes it far more useful than VGA for
//! debugging: you can scroll back, pipe it to a file, and it survives a
//! screen full of scrolling.

use crate::cpu::{inb, outb, without_interrupts};
use crate::sync::Mutex;
use core::fmt;

const COM1: u16 = 0x3F8;

pub struct SerialPort {
    base: u16,
    initialized: bool,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        SerialPort {
            base,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        unsafe {
            outb(self.base + 1, 0x00); // disable interrupts
            outb(self.base + 3, 0x80); // enable DLAB (divisor latch)
            outb(self.base, 0x03); // divisor low  = 3 => 38400 baud
            outb(self.base + 1, 0x00); // divisor high = 0
            outb(self.base + 3, 0x03); // 8 bits, no parity, one stop bit
            outb(self.base + 2, 0xC7); // enable+clear FIFO, 14-byte threshold
            outb(self.base + 4, 0x0B); // IRQs off, RTS/DSR set
        }
        self.initialized = true;
    }

    fn is_transmit_empty(&self) -> bool {
        unsafe { inb(self.base + 5) & 0x20 != 0 }
    }

    pub fn write_byte(&mut self, byte: u8) {
        if !self.initialized {
            return;
        }
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        unsafe { outb(self.base, byte) };
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

pub static SERIAL1: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1));

pub fn init() {
    SERIAL1.lock().init();
}

pub fn write_byte(byte: u8) {
    SERIAL1.lock().write_byte(byte);
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    without_interrupts(|| {
        let _ = SERIAL1.lock().write_fmt(args);
    });
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
