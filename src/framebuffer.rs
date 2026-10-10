//! Linear framebuffer from Multiboot2 tag 8.

use crate::boot_info;
use crate::sync::Mutex;

pub use crate::boot_info::FramebufferInfo;

struct State {
    info: Option<FramebufferInfo>,
}

static FB: Mutex<State> = Mutex::new(State { info: None });

pub fn init_from_boot() {
    let info = boot_info::get().framebuffer;
    FB.lock().info = info;
    if let Some(fb) = info {
        serial_println!(
            "[ ok ] framebuffer {}x{} @ {:#x} pitch {} bpp {}",
            fb.width,
            fb.height,
            fb.addr,
            fb.pitch,
            fb.bpp
        );
    } else {
        serial_println!("[warn] no linear framebuffer (text mode only)");
    }
}

pub fn info() -> Option<FramebufferInfo> {
    FB.lock().info
}

/// Write a 32-bit RGB pixel (0x00RRGGBB). No-op if FB missing or bpp != 32.
pub fn put_pixel(x: u32, y: u32, rgb: u32) {
    let Some(fb) = info() else {
        return;
    };
    if fb.bpp != 32 || x >= fb.width || y >= fb.height {
        return;
    }
    let offset = fb.pitch * y as u64 + x as u64 * 4;
    unsafe {
        *((fb.addr + offset) as *mut u32) = rgb;
    }
}

pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, rgb: u32) {
    for dy in 0..h {
        for dx in 0..w {
            put_pixel(x + dx, y + dy, rgb);
        }
    }
}
