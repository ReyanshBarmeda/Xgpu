//! Raw CPU primitives: port I/O, interrupt flag control, control
//! registers, and descriptor-table loading. Everything here is a thin
//! wrapper over a single instruction.

use core::arch::asm;

// ---------------------------------------------------------------------
// Port I/O
// ---------------------------------------------------------------------

/// Write a byte to an I/O port.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value,
         options(nomem, nostack, preserves_flags));
}

/// Read a byte from an I/O port.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", out("al") value, in("dx") port,
         options(nomem, nostack, preserves_flags));
    value
}

/// Burn one I/O cycle on an unused port. Some legacy hardware (notably
/// the 8259 PIC) needs a moment to settle between command bytes.
#[inline]
pub unsafe fn io_wait() {
    outb(0x80, 0);
}

// ---------------------------------------------------------------------
// Interrupt flag
// ---------------------------------------------------------------------

#[inline]
pub fn enable_interrupts() {
    unsafe { asm!("sti", options(nomem, nostack)) };
}

#[inline]
pub fn disable_interrupts() {
    unsafe { asm!("cli", options(nomem, nostack)) };
}

/// Read RFLAGS and test the interrupt-enable bit (bit 9).
#[inline]
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe {
        asm!("pushfq; pop {}", out(reg) flags, options(nomem, preserves_flags));
    }
    flags & (1 << 9) != 0
}

/// Run `f` with interrupts disabled, restoring the previous state after.
///
/// This is what keeps a spinlock taken in normal kernel code from
/// deadlocking against an interrupt handler that wants the same lock.
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let was_enabled = interrupts_enabled();
    if was_enabled {
        disable_interrupts();
    }
    let result = f();
    if was_enabled {
        enable_interrupts();
    }
    result
}

/// Halt until the next interrupt arrives. Used instead of a busy loop so
/// an idle Xdos doesn't peg the CPU.
#[inline]
pub fn hlt() {
    unsafe { asm!("hlt", options(nomem, nostack)) };
}

pub fn halt_loop() -> ! {
    loop {
        hlt();
    }
}

// ---------------------------------------------------------------------
// Control registers
// ---------------------------------------------------------------------

/// Read CR3, which holds the physical address of the active level-4
/// page table (plus some flags in the low 12 bits).
#[inline]
pub fn read_cr3() -> u64 {
    let value: u64;
    unsafe { asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

#[inline]
pub unsafe fn write_cr3(value: u64) {
    asm!("mov cr3, {}", in(reg) value, options(nomem, nostack, preserves_flags));
}

/// Early CPU setup: enable SSE and sane FPU state for future user code.
pub fn init_boot() {
    unsafe {
        // Clear EM (bit 2), set MP (bit 1) — required before using XMM.
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 &= !(1 << 2);
        cr0 |= 1 << 1;
        asm!("mov cr0, {}", in(reg) cr0);

        asm!("fninit");
        // OSFXSR + OSXMMEXCPT in CR4
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);
        cr4 |= (1 << 9) | (1 << 10);
        asm!("mov cr4, {}", in(reg) cr4);
    }
}

/// Read CR2, which holds the faulting address after a page fault.
#[inline]
pub fn read_cr2() -> u64 {
    let value: u64;
    unsafe { asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags)) };
    value
}

/// Invalidate a single page's TLB entry after changing its mapping.
#[inline]
pub fn flush_tlb(virt_addr: u64) {
    unsafe {
        asm!("invlpg [{}]", in(reg) virt_addr, options(nostack, preserves_flags));
    }
}

// ---------------------------------------------------------------------
// Descriptor tables
// ---------------------------------------------------------------------

/// The operand format expected by `lgdt` and `lidt`.
#[repr(C, packed)]
pub struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

#[inline]
pub unsafe fn lgdt(ptr: &DescriptorTablePointer) {
    asm!("lgdt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
}

#[inline]
pub unsafe fn lidt(ptr: &DescriptorTablePointer) {
    asm!("lidt [{}]", in(reg) ptr, options(readonly, nostack, preserves_flags));
}

/// Load the task register with a TSS selector.
#[inline]
pub unsafe fn load_tss(selector: u16) {
    asm!("ltr {0:x}", in(reg) selector, options(nomem, nostack, preserves_flags));
}

/// Reload CS. In 64-bit mode there is no far jump with an immediate
/// operand, so the canonical trick is to push the new selector and a
/// return address, then `retfq` into it.
#[inline]
const MSR_STAR: u32 = 0xC000_0081;
const MSR_LSTAR: u32 = 0xC000_0082;
const MSR_SFMASK: u32 = 0xC000_0084;

#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack, preserves_flags),
    );
}

/// Configure SYSCALL/SYSRET targets for userland (`syscall` instruction).
pub unsafe fn init_syscall_msrs(
    entry: u64,
    kernel_cs: u16,
    user_code: u16,
    user_data: u16,
) {
    let _ = user_data;
    // STAR: kernel CS at 63:48; user CS base (cleared RPL) at 47:32 for SYSRET.
    let star = (u64::from(kernel_cs) << 48) | (u64::from(user_code & !3) << 32);
    wrmsr(MSR_STAR, star);
    wrmsr(MSR_LSTAR, entry);
    // Mask IF on syscall entry so critical sections stay predictable.
    wrmsr(MSR_SFMASK, 1 << 9);
}

#[inline]
pub unsafe fn set_cs(selector: u16) {
    asm!(
        "push {sel}",
        "lea {tmp}, [rip + 2f]",
        "push {tmp}",
        "retfq",
        "2:",
        sel = in(reg) u64::from(selector),
        tmp = lateout(reg) _,
        options(preserves_flags),
    );
}
