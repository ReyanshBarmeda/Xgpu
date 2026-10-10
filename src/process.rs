//! Processes — address space + user/kernel mode metadata.

use crate::address_space::AddressSpace;
use crate::elf::{load, ElfError};
use crate::gdt::USER_DATA_SELECTOR;
use crate::scheduler;
use core::sync::atomic::{AtomicU64, Ordering};

static NEXT_PID: AtomicU64 = AtomicU64::new(2);

extern "C" {
    fn enter_user_mode(entry: u64, stack: u64) -> !;
}

pub struct Process {
    pub pid: u64,
    pub name: &'static str,
    pub kind: crate::task::TaskKind,
    pub address_space: AddressSpace,
    pub user_entry: u64,
    pub user_stack: u64,
}

impl Process {
    pub fn spawn_kernel_thread(
        name: &'static str,
        entry: extern "C" fn(u64) -> !,
        arg: u64,
    ) -> Result<u64, crate::memory::MapError> {
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
        scheduler::spawn_with_pid(pid, name, entry, arg, crate::task::TaskKind::KernelThread, 0)?;
        Ok(pid)
    }

    pub fn spawn_elf(name: &'static str, image: &'static [u8]) -> Result<u64, ElfError> {
        let aspace = AddressSpace::new().map_err(ElfError::Map)?;
        let loaded = load(&aspace, image)?;
        let stack = aspace.map_user_stack().map_err(ElfError::Map)?;
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);

        let proc = Process {
            pid,
            name,
            kind: crate::task::TaskKind::User,
            address_space: aspace,
            user_entry: loaded.entry,
            user_stack: stack,
        };

        scheduler::spawn_user_process(proc)?;
        Ok(pid)
    }
}

pub extern "C" fn user_process_wrapper(proc_ptr: u64) -> ! {
    user_launch_trampoline(proc_ptr as *const Process);
}

/// Trampoline task: activate process CR3 and iretq into user code.
pub(crate) fn user_launch_trampoline(proc: *const Process) -> ! {
    let proc = unsafe { &*proc };
    proc.address_space.activate();
    unsafe {
        // Load user data segments before iretq (ignored in 64-bit user except SS).
        core::arch::asm!(
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            ds = in(reg) USER_DATA_SELECTOR,
            options(nostack, preserves_flags),
        );
        enter_user_mode(proc.user_entry, proc.user_stack);
    }
}

/// Stored on the heap for the lifetime of the user task.
pub fn leak_process_box(p: Process) -> *const Process {
    let b = alloc::boxed::Box::new(p);
    alloc::boxed::Box::into_raw(b)
}
