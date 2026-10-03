//! Interrupt Descriptor Table: CPU exceptions and hardware IRQs.
//!
//! Without this, any CPU fault immediately triple-faults and the machine
//! silently reboots. With it, a fault prints something you can debug.

use crate::cpu::{lidt, read_cr2, DescriptorTablePointer};
use crate::gdt::{DOUBLE_FAULT_IST_INDEX, KERNEL_CODE_SELECTOR};
use crate::{pic, println};
use core::mem::size_of;
use core::ptr::addr_of;

/// The stack frame the CPU pushes before entering a handler.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptStackFrame {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: u64,
    pub stack_segment: u64,
}

/// One 16-byte gate descriptor.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    pub const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Point this gate at `handler`.
    ///
    /// `ist_index` is 1-based and selects an Interrupt Stack Table slot
    /// from the TSS; 0 means "keep using the current stack".
    fn set_handler(&mut self, handler: u64, ist_index: u8) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = KERNEL_CODE_SELECTOR;
        self.ist = ist_index & 0b111;
        // Present, ring 0, 64-bit interrupt gate (type 0xE). An interrupt
        // gate clears IF on entry, so handlers are not re-entered by the
        // same line while they run.
        self.type_attr = 0x8E;
        self.reserved = 0;
    }
}

#[repr(C, align(16))]
pub struct Idt {
    entries: [IdtEntry; 256],
}

impl Idt {
    pub const fn new() -> Self {
        Idt {
            entries: [IdtEntry::missing(); 256],
        }
    }

    pub unsafe fn load(&'static self) {
        let pointer = DescriptorTablePointer {
            limit: (size_of::<Idt>() - 1) as u16,
            base: self as *const _ as u64,
        };
        lidt(&pointer);
    }
}

static mut IDT: Idt = Idt::new();

// ---------------------------------------------------------------------
// Hardware interrupt vector numbers
// ---------------------------------------------------------------------

pub const TIMER_VECTOR: u8 = pic::PIC_1_OFFSET; // IRQ0
pub const KEYBOARD_VECTOR: u8 = pic::PIC_1_OFFSET + 1; // IRQ1

pub fn init_idt() {
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);

        idt.entries[0].set_handler(divide_by_zero_handler as u64, 0);
        idt.entries[3].set_handler(breakpoint_handler as u64, 0);
        idt.entries[6].set_handler(invalid_opcode_handler as u64, 0);
        idt.entries[13].set_handler(general_protection_fault_handler as u64, 0);
        idt.entries[14].set_handler(page_fault_handler as u64, 0);

        // The double-fault handler runs on its own IST stack so that a
        // kernel stack overflow produces a panic instead of a triple
        // fault and an instant reboot.
        idt.entries[8].set_handler(double_fault_handler as u64, DOUBLE_FAULT_IST_INDEX);

        idt.entries[TIMER_VECTOR as usize].set_handler(timer_interrupt_handler as u64, 0);
        idt.entries[KEYBOARD_VECTOR as usize].set_handler(keyboard_interrupt_handler as u64, 0);

        // Re-borrow as shared for the 'static lifetime `load` requires.
        let idt_ref: &'static Idt = &*addr_of!(IDT);
        idt_ref.load();
    }
}

// ---------------------------------------------------------------------
// CPU exception handlers
// ---------------------------------------------------------------------

extern "x86-interrupt" fn divide_by_zero_handler(frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", frame);
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    // Breakpoints are recoverable: report and continue.
    println!("EXCEPTION: BREAKPOINT at {:#x}", frame.instruction_pointer);
}

extern "x86-interrupt" fn invalid_opcode_handler(frame: InterruptStackFrame) {
    panic!("EXCEPTION: INVALID OPCODE\n{:#?}", frame);
}

extern "x86-interrupt" fn general_protection_fault_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!(
        "EXCEPTION: GENERAL PROTECTION FAULT (error {:#x})\n{:#?}",
        error_code, frame
    );
}

extern "x86-interrupt" fn page_fault_handler(frame: InterruptStackFrame, error_code: u64) {
    // CR2 holds the address that was accessed. The error code says why:
    // bit 0 = protection violation (else not-present), bit 1 = write,
    // bit 2 = user-mode access, bit 4 = instruction fetch.
    let accessed = read_cr2();
    panic!(
        "EXCEPTION: PAGE FAULT\n\
         accessed address: {:#x}\n\
         caused by: {}{}{}\n\
         {:#?}",
        accessed,
        if error_code & 1 == 0 {
            "page not present"
        } else {
            "protection violation"
        },
        if error_code & 2 != 0 { ", on write" } else { ", on read" },
        if error_code & 4 != 0 { ", from user mode" } else { "" },
        frame
    );
}

extern "x86-interrupt" fn double_fault_handler(
    frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    // A double fault means a fault occurred while handling another
    // fault. There is no returning from this.
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", frame);
}

// ---------------------------------------------------------------------
// Hardware interrupt handlers
// ---------------------------------------------------------------------

extern "x86-interrupt" fn timer_interrupt_handler(_frame: InterruptStackFrame) {
    crate::pit::on_tick();
    // EOI must go out before the scheduler runs: on_timer_tick() may
    // not return for a long time (another task's entire turn), and the
    // PIC will not deliver IRQ0 again until this one is acknowledged.
    // Sending EOI late would silently stop the timer from ticking at
    // all for every task after the first switch.
    pic::notify_end_of_interrupt(TIMER_VECTOR);
    crate::scheduler::on_timer_tick();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_frame: InterruptStackFrame) {
    crate::keyboard::on_key_interrupt();
    pic::notify_end_of_interrupt(KEYBOARD_VECTOR);
}

/// Trigger a breakpoint exception. Handy for checking the IDT works at
/// all without having to deliberately crash the kernel.
pub fn trigger_breakpoint() {
    unsafe { core::arch::asm!("int3", options(nomem, nostack)) };
}
