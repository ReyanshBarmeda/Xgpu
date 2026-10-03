//! Per-process page tables (CR3). Clones the active kernel map today;
//! user mappings are added with `PAGE_USER`.

use crate::cpu::{flush_tlb, write_cr3};
use crate::memory::{
    self, active_level4_table, allocate_zeroed_frame, map_page, unmap_page, MapError, PageTable,
    ADDR_MASK, PAGE_NO_EXECUTE, PAGE_PRESENT, PAGE_USER, PAGE_WRITABLE,
};

pub const USER_STACK_TOP: u64 = 0x0000_0000_0070_0000;
pub const USER_STACK_SIZE: u64 = 64 * 1024;

pub struct AddressSpace {
    l4_phys: u64,
}

impl AddressSpace {
    /// New top-level table sharing existing lower-level tables with the kernel.
    pub fn new() -> Result<Self, MapError> {
        let l4 = allocate_zeroed_frame().ok_or(MapError::OutOfFrames)?;
        unsafe {
            let active = active_level4_table();
            let new_l4 = PageTable::from_phys(l4);
            new_l4.entries.copy_from_slice(&active.entries);
        }
        Ok(AddressSpace { l4_phys: l4 })
    }

    pub fn cr3(&self) -> u64 {
        self.l4_phys
    }

    pub fn activate(&self) {
        unsafe {
            write_cr3(self.l4_phys);
        }
    }

    pub fn map_user(&self, virt: u64, phys: u64, executable: bool) -> Result<(), MapError> {
        let mut flags = PAGE_PRESENT | PAGE_WRITABLE | PAGE_USER;
        if !executable {
            flags |= PAGE_NO_EXECUTE;
        }
        self.with_active(|| map_page(virt, phys, flags))
    }

    pub fn unmap(&self, virt: u64) -> Result<(), MapError> {
        self.with_active(|| unmap_page(virt))
    }

    pub fn map_user_stack(&self) -> Result<u64, MapError> {
        let base = USER_STACK_TOP - USER_STACK_SIZE;
        let pages = USER_STACK_SIZE / memory::PAGE_SIZE;
        for i in 0..pages {
            let virt = base + i * memory::PAGE_SIZE;
            let frame = memory::FRAME_ALLOCATOR
                .lock()
                .allocate()
                .ok_or(MapError::OutOfFrames)?;
            self.map_user(virt, frame, false)?;
        }
        Ok(USER_STACK_TOP)
    }

    pub fn write_bytes(&self, virt: u64, src: &[u8]) {
        self.with_active(|| unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), virt as *mut u8, src.len());
        });
    }

    fn with_active<R>(&self, f: impl FnOnce() -> R) -> R {
        let prev = crate::cpu::read_cr3() & ADDR_MASK;
        self.activate();
        let out = f();
        unsafe {
            write_cr3(prev);
        }
        flush_tlb(0);
        out
    }
}

// Re-export for sibling modules in memory.rs — keep mask local.
pub use crate::memory::PAGE_SIZE;
