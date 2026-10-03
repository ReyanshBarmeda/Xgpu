//! PS/2 keyboard driver (scancode set 1).
//!
//! The controller raises IRQ1 whenever a key is pressed or released, and
//! the scancode is read from port 0x60. Bit 7 set means "release".
//!
//! Decoded characters go into a small ring buffer rather than being
//! printed from the handler, so that consuming input stays the caller's
//! decision. `pop_char` is how the rest of the kernel reads keystrokes.

use crate::cpu::inb;
use crate::sync::Mutex;

const PS2_DATA_PORT: u16 = 0x60;

/// Unshifted characters for scancode set 1, indexed by scancode.
/// A zero byte means "no printable character for this key".
const SCANCODE_MAP: [u8; 58] = [
    0, 0, // 0x00 none, 0x01 escape
    b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 0x08, // backspace
    b'\t', b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n', // enter
    0,    // left control
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, // left shift
    b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, // right shift
    b'*', 0, // left alt
    b' ', // space
];

/// Shifted characters, same indexing.
const SCANCODE_MAP_SHIFTED: [u8; 58] = [
    0, 0, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 0x08, b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n', 0, b'A', b'S',
    b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~', 0, b'|', b'Z', b'X', b'C', b'V',
    b'B', b'N', b'M', b'<', b'>', b'?', 0, b'*', 0, b' ',
];

const SCANCODE_LEFT_SHIFT: u8 = 0x2A;
const SCANCODE_RIGHT_SHIFT: u8 = 0x36;
const SCANCODE_CAPS_LOCK: u8 = 0x3A;
const RELEASE_FLAG: u8 = 0x80;

const BUFFER_CAPACITY: usize = 128;

struct KeyboardState {
    shift_pressed: bool,
    caps_lock: bool,
    buffer: [u8; BUFFER_CAPACITY],
    head: usize,
    tail: usize,
}

impl KeyboardState {
    const fn new() -> Self {
        KeyboardState {
            shift_pressed: false,
            caps_lock: false,
            buffer: [0; BUFFER_CAPACITY],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        let next = (self.head + 1) % BUFFER_CAPACITY;
        // Drop the keystroke if the buffer is full rather than
        // overwriting unread input.
        if next != self.tail {
            self.buffer[self.head] = byte;
            self.head = next;
        }
    }

    fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            return None;
        }
        let byte = self.buffer[self.tail];
        self.tail = (self.tail + 1) % BUFFER_CAPACITY;
        Some(byte)
    }
}

static KEYBOARD: Mutex<KeyboardState> = Mutex::new(KeyboardState::new());

/// Called from the IRQ1 handler.
///
/// The scancode byte must be read from port 0x60 on every interrupt even
/// if we do nothing with it, or the controller will not send another.
pub fn on_key_interrupt() {
    let scancode = unsafe { inb(PS2_DATA_PORT) };

    // Interrupts are already disabled (interrupt gate), so taking the
    // lock directly is safe here.
    let mut kb = KEYBOARD.lock();

    let released = scancode & RELEASE_FLAG != 0;
    let code = scancode & !RELEASE_FLAG;

    match code {
        SCANCODE_LEFT_SHIFT | SCANCODE_RIGHT_SHIFT => {
            kb.shift_pressed = !released;
            return;
        }
        SCANCODE_CAPS_LOCK => {
            if !released {
                kb.caps_lock = !kb.caps_lock;
            }
            return;
        }
        _ => {}
    }

    if released {
        return;
    }

    let index = code as usize;
    if index >= SCANCODE_MAP.len() {
        return;
    }

    let mut ch = if kb.shift_pressed {
        SCANCODE_MAP_SHIFTED[index]
    } else {
        SCANCODE_MAP[index]
    };

    // Caps lock affects letters only, and inverts whatever shift did.
    if kb.caps_lock {
        if ch.is_ascii_lowercase() {
            ch = ch.to_ascii_uppercase();
        } else if ch.is_ascii_uppercase() {
            ch = ch.to_ascii_lowercase();
        }
    }

    if ch != 0 {
        kb.push(ch);
    }
}

/// Read one buffered character, if any. Returns `None` when no key has
/// been pressed since the last call.
pub fn pop_char() -> Option<u8> {
    crate::cpu::without_interrupts(|| KEYBOARD.lock().pop())
}
