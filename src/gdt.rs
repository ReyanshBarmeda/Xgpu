//! Global Descriptor Table and Task State Segment.
//!
//! boot.S loaded a bare-minimum GDT (null + code) just to get into long
//! mode. We replace it here with a proper one that also carries a TSS.
//!
//! The TSS matters for one specific reason: the Interrupt Stack Table.
//! If the kernel stack overflows, the CPU tries to push an exception
//! frame onto the already-broken stack, which faults again — a double
//! fault — and pushing *that* frame faults a third time, which the CPU
//! cannot recover from. A triple fault reboots the machine instantly,
//! with no error message. Pointing the double-fault handler at a
//! separate IST stack means it always has somewhere safe to land, so a
//! stack overflow produces a readable panic instead of a reboot loop.

use crate::cpu::{lgdt, load_tss, set_cs, DescriptorTablePointer};
use core::mem::size_of;
use core::ptr::addr_of;

/// IST slot (1-based, as the IDT encodes it) used by the double-fault
/// handler.
pub const DOUBLE_FAULT_IST_INDEX: u8 = 1;

const IST_STACK_SIZE: usize = 4096 * 5;

/// The dedicated double-fault stack. `static mut` because its *address*
/// is what matters; it is never read or written through Rust.
static mut DOUBLE_FAULT_STACK: [u8; IST_STACK_SIZE] = [0; IST_STACK_SIZE];

#[repr(C, packed(4))]
#[derive(Clone, Copy)]
pub struct Tss {
    reserved_1: u32,
    pub privilege_stack_table: [u64; 3],
    reserved_2: u64,
    pub interrupt_stack_table: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    pub iomap_base: u16,
}

impl Tss {
    pub const fn new() -> Self {
        Tss {
            reserved_1: 0,
            privilege_stack_table: [0; 3],
            reserved_2: 0,
            interrupt_stack_table: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            // No I/O permission bitmap: point past the end of the TSS.
            iomap_base: size_of::<Tss>() as u16,
        }
    }
}

static mut TSS: Tss = Tss::new();

/// null, kernel code/data, 16-byte TSS, user code/data (ring 3).
static mut GDT: [u64; 7] = [0; 7];

pub const KERNEL_CODE_SELECTOR: u16 = 0x08; // index 1
pub const KERNEL_DATA_SELECTOR: u16 = 0x10; // index 2
pub const TSS_SELECTOR: u16 = 0x18; // index 3
pub const USER_CODE_SELECTOR: u16 = 0x28 | 3; // index 5, ring 3
pub const USER_DATA_SELECTOR: u16 = 0x30 | 3; // index 6, ring 3

/// Long-mode kernel code segment: present, ring 0, executable,
/// readable, with the L (64-bit) flag set.
const KERNEL_CODE_DESCRIPTOR: u64 = 0x00AF_9A00_0000_FFFF;
/// Kernel data segment: present, ring 0, writable.
const KERNEL_DATA_DESCRIPTOR: u64 = 0x00CF_9200_0000_FFFF;
/// User code/data: same types as kernel segments but DPL 3 (for SYSRET targets).
const USER_CODE_DESCRIPTOR: u64 = 0x00AF_FA00_0000_FFFF;
const USER_DATA_DESCRIPTOR: u64 = 0x00CF_F200_0000_FFFF;

/// Build the two 8-byte halves of a 64-bit TSS descriptor.
fn tss_descriptor(base: u64, limit: u32) -> (u64, u64) {
    let mut low: u64 = 0;

    low |= u64::from(limit & 0xFFFF); // limit 0..15
    low |= (base & 0x00FF_FFFF) << 16; // base 0..23
    low |= 0x89 << 40; // present, type 0b1001 (available 64-bit TSS)
    low |= u64::from((limit >> 16) & 0xF) << 48; // limit 16..19
    low |= ((base >> 24) & 0xFF) << 56; // base 24..31

    let high: u64 = (base >> 32) & 0xFFFF_FFFF; // base 32..63

    (low, high)
}

/// Install the new GDT and TSS, then reload CS and the task register.
pub fn init() {
    unsafe {
        // Point IST slot 1 at the top of the double-fault stack.
        // x86 stacks grow downward, so the pointer is the *end* of the
        // array, not the start.
        let stack_start = addr_of!(DOUBLE_FAULT_STACK) as u64;
        let stack_end = stack_start + IST_STACK_SIZE as u64;

        let tss = &mut *core::ptr::addr_of_mut!(TSS);
        tss.interrupt_stack_table[(DOUBLE_FAULT_IST_INDEX - 1) as usize] = stack_end;

        let tss_base = addr_of!(TSS) as u64;
        let tss_limit = (size_of::<Tss>() - 1) as u32;
        let (tss_low, tss_high) = tss_descriptor(tss_base, tss_limit);

        let gdt = &mut *core::ptr::addr_of_mut!(GDT);
        gdt[0] = 0;
        gdt[1] = KERNEL_CODE_DESCRIPTOR;
        gdt[2] = KERNEL_DATA_DESCRIPTOR;
        gdt[3] = tss_low;
        gdt[4] = tss_high;
        gdt[5] = USER_CODE_DESCRIPTOR;
        gdt[6] = USER_DATA_DESCRIPTOR;

        let pointer = DescriptorTablePointer {
            limit: (size_of::<[u64; 7]>() - 1) as u16,
            base: addr_of!(GDT) as u64,
        };

        lgdt(&pointer);
        set_cs(KERNEL_CODE_SELECTOR);
        load_data_segments(KERNEL_DATA_SELECTOR);
        load_tss(TSS_SELECTOR);
    }
}

unsafe fn load_data_segments(selector: u16) {
    use core::arch::asm;
    asm!(
        "mov ss, {0:x}",
        "mov ds, {0:x}",
        "mov es, {0:x}",
        in(reg) selector,
        options(nostack, preserves_flags),
    );
}
