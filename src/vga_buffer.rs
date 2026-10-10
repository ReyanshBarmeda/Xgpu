//! VGA text-mode driver.
//!
//! Writes to the memory-mapped text buffer at 0xb8000: 80x25 cells of
//! two bytes each (ASCII byte, then a colour attribute byte).

use crate::cpu::without_interrupts;
use crate::sync::Mutex;
use core::fmt;

const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
const BUFFER_WIDTH: usize = 80;
const BUFFER_HEIGHT: usize = 25;

#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

pub const fn color_code(fg: Color, bg: Color) -> u8 {
    ((bg as u8) << 4) | (fg as u8)
}

pub struct Writer {
    column: usize,
    row: usize,
    color: u8,
}

impl Writer {
    pub const fn new() -> Self {
        Writer {
            column: 0,
            row: 0,
            color: color_code(Color::LightGray, Color::Black),
        }
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.color = color_code(fg, bg);
    }

    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.write_at(row, col, b' ', self.color);
            }
        }
        self.column = 0;
        self.row = 0;
    }

    fn write_at(&self, row: usize, col: usize, byte: u8, color: u8) {
        let offset = (row * BUFFER_WIDTH + col) * 2;
        unsafe {
            VGA_BUFFER.add(offset).write_volatile(byte);
            VGA_BUFFER.add(offset + 1).write_volatile(color);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\r' => self.column = 0,
            // Backspace: step back one cell and blank it.
            0x08 => {
                if self.column > 0 {
                    self.column -= 1;
                    self.write_at(self.row, self.column, b' ', self.color);
                }
            }
            byte => {
                if self.column >= BUFFER_WIDTH {
                    self.new_line();
                }
                self.write_at(self.row, self.column, byte, self.color);
                self.column += 1;
            }
        }
    }

    /// Write raw CP437 bytes straight to the screen.
    ///
    /// The `fmt::Write` path below only passes printable ASCII and folds
    /// everything else to a placeholder glyph, which is right for
    /// formatted text but would destroy box/block characters. This
    /// bypasses that, so callers can use the CP437 glyphs the VGA font
    /// actually has (0xDB is a solid block).
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_byte(byte);
        }
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row + 1 >= BUFFER_HEIGHT {
            self.scroll();
        } else {
            self.row += 1;
        }
    }

    fn scroll(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let offset = (row * BUFFER_WIDTH + col) * 2;
                let (byte, color) = unsafe {
                    (
                        VGA_BUFFER.add(offset).read_volatile(),
                        VGA_BUFFER.add(offset + 1).read_volatile(),
                    )
                };
                self.write_at(row - 1, col, byte, color);
            }
        }
        for col in 0..BUFFER_WIDTH {
            self.write_at(BUFFER_HEIGHT - 1, col, b' ', self.color);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | b'\r' | 0x08 => self.write_byte(byte),
                _ => self.write_byte(0xfe), // non-printable -> block glyph
            }
        }
        Ok(())
    }
}

pub static WRITER: Mutex<Writer> = Mutex::new(Writer::new());

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // Interrupts stay off for the whole write: the timer and keyboard
    // handlers also print, and re-entering this lock would deadlock.
    without_interrupts(|| {
        let _ = WRITER.lock().write_fmt(args);
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
