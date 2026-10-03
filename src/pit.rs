//! 8253/8254 Programmable Interval Timer.
//!
//! This is the kernel's heartbeat. Configured for 100 Hz, it fires IRQ0
//! every 10 ms. Once Xdos has a scheduler, this tick is what will drive
//! preemption; for now it just counts uptime.
//!
//! (The APIC timer is the modern replacement and is per-core, which
//! matters once you go multiprocessor. The PIT is simpler, always
//! present, and enough to build a scheduler on, so it comes first.)

use crate::cpu::outb;
use core::sync::atomic::{AtomicU64, Ordering};

const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

/// The PIT's fixed input frequency, ~1.193182 MHz.
const PIT_BASE_FREQUENCY: u32 = 1_193_182;

/// Ticks per second. 100 Hz = one tick every 10 ms.
pub const TICK_HZ: u32 = 100;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    let divisor = PIT_BASE_FREQUENCY / TICK_HZ;

    unsafe {
        // Channel 0, access mode lo/hi byte, mode 3 (square wave),
        // binary counting.
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
        outb(PIT_CHANNEL0, ((divisor >> 8) & 0xFF) as u8);
    }
}

/// Called from the IRQ0 handler. Must stay tiny and allocation-free:
/// it runs with interrupts disabled on every single tick.
pub fn on_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Total ticks since the timer was initialized.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Milliseconds of uptime since the timer was initialized.
pub fn uptime_ms() -> u64 {
    ticks() * 1000 / u64::from(TICK_HZ)
}

/// Busy-wait for roughly `ms` milliseconds.
///
/// This spins rather than sleeping, because there is no scheduler yet to
/// yield to. Interrupts must be enabled or this will never return.
pub fn sleep_ms(ms: u64) {
    let target = ticks() + (ms * u64::from(TICK_HZ) / 1000);
    while ticks() < target {
        crate::cpu::hlt();
    }
}
