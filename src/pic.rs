//! 8259 Programmable Interrupt Controller.
//!
//! On boot the two cascaded PICs deliver IRQs on vectors 0x08-0x0F and
//! 0x70-0x77. That first range collides head-on with the CPU's own
//! exception vectors — a timer tick would be indistinguishable from a
//! double fault. So the first thing any kernel does is remap them; we
//! move them to 0x20-0x2F.

use crate::cpu::{inb, io_wait, outb};

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

const CMD_END_OF_INTERRUPT: u8 = 0x20;

/// Vector where master PIC IRQs begin after remapping.
pub const PIC_1_OFFSET: u8 = 0x20;
/// Vector where slave PIC IRQs begin after remapping.
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// Remap both PICs and mask every IRQ line except those we handle.
pub fn init() {
    unsafe {
        // Save the existing masks; we restore a modified version below.
        let _mask1 = inb(PIC1_DATA);
        let _mask2 = inb(PIC2_DATA);

        // Start the initialization sequence (cascade mode).
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // ICW2: vector offsets.
        outb(PIC1_DATA, PIC_1_OFFSET);
        io_wait();
        outb(PIC2_DATA, PIC_2_OFFSET);
        io_wait();

        // ICW3: tell master the slave is on IRQ2; tell slave its identity.
        outb(PIC1_DATA, 4);
        io_wait();
        outb(PIC2_DATA, 2);
        io_wait();

        // ICW4: 8086/88 mode.
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Unmask only IRQ0 (timer) and IRQ1 (keyboard) on the master;
        // mask everything on the slave. A masked line is simply ignored,
        // which is what we want for hardware Xdos has no driver for yet.
        outb(PIC1_DATA, 0b1111_1100);
        outb(PIC2_DATA, 0b1111_1111);
    }
}

/// Acknowledge an interrupt so the PIC will deliver the next one.
///
/// Forgetting this is the classic "my timer fired exactly once" bug: the
/// PIC waits forever for an EOI that never comes.
pub fn notify_end_of_interrupt(vector: u8) {
    unsafe {
        // IRQs 8-15 arrive via the slave, which needs its own EOI first.
        if vector >= PIC_2_OFFSET {
            outb(PIC2_COMMAND, CMD_END_OF_INTERRUPT);
        }
        outb(PIC1_COMMAND, CMD_END_OF_INTERRUPT);
    }
}
