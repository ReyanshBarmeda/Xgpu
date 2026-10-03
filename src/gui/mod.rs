//! Minimal GUI — draws to linear framebuffer when available.

use crate::framebuffer;
use crate::mouse;

pub fn init() {
    if framebuffer::info().is_some() {
        serial_println!("[ ok ] GUI compositor (framebuffer rects)");
        demo_chrome();
    }
}

fn demo_chrome() {
    // Title bar + panel using theme-like colors.
    framebuffer::fill_rect(0, 0, 800.min(fb_width()), 24, 0x00335577);
    let st = mouse::state();
    framebuffer::fill_rect(
        (st.x.max(0) as u32).min(fb_width().saturating_sub(8)),
        (st.y.max(0) as u32).min(fb_height().saturating_sub(8)),
        8,
        8,
        0x00FFCC00,
    );
}

fn fb_width() -> u32 {
    framebuffer::info().map(|f| f.width).unwrap_or(0)
}

fn fb_height() -> u32 {
    framebuffer::info().map(|f| f.height).unwrap_or(0)
}

pub fn redraw_cursor() {
    if framebuffer::info().is_some() {
        demo_chrome();
    }
}
