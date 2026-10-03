//! Physical and virtual memory management.
//!
//! # A note on the identity-mapped window
//!
//! boot.S identity-maps the first 1 GiB of physical memory with 2 MiB
//! huge pages, so within that window physical address == virtual
//! address. Everything here leans on that: page-table frames can be
//! dereferenced directly without first mapping them somewhere.
//!
//! The cost is that the frame allocator can only hand out frames below
//! 1 GiB. That is plenty for now, but it is a real ceiling: to use more
//! RAM you would extend the boot mapping (or switch to a higher-half
//! kernel with a proper physical-memory offset mapping) and grow the
//! bitmap accordingly.

use crate::cpu::{flush_tlb, read_cr3};
use crate::sync::Mutex;

pub const PAGE_SIZE: u64 = 4096;

/// Upper bound on physical memory we can manage, set by the boot
/// identity mapping.
pub const MAX_PHYS_ADDR: u64 = 1024 * 1024 * 1024; // 1 GiB
const MAX_FRAMES: usize = (MAX_PHYS_ADDR / PAGE_SIZE) as usize; // 262144
const BITMAP_WORDS: usize = MAX_FRAMES / 64; // 4096 u64s = 32 KiB

// Page table entry flags.
pub const PAGE_PRESENT: u64 = 1 << 0;
pub const PAGE_WRITABLE: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
pub const PAGE_HUGE: u64 = 1 << 7;
pub const PAGE_NO_EXECUTE: u64 = 1 << 63;

pub(crate) const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

// ---------------------------------------------------------------------
// Multiboot2 memory map
// ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub start: u64,
    pub length: u64,
    pub usable: bool,
}

/// Walk the Multiboot2 tag list looking for the memory-map tag (type 6)
/// and call `f` for each region it describes.
///
/// # Safety
/// `info_addr` must be the pointer GRUB handed us in ebx.
unsafe fn for_each_memory_region<F: FnMut(MemoryRegion)>(info_addr: u64, mut f: F) {
    if info_addr == 0 {
        return;
    }

    let total_size = *(info_addr as *const u32) as u64;
    // Tags start after the 8-byte header (total_size + reserved).
    let mut tag_addr = info_addr + 8;
    let end = info_addr + total_size;

    while tag_addr < end {
        let tag_type = *(tag_addr as *const u32);
        let tag_size = *((tag_addr + 4) as *const u32) as u64;

        // Type 0 with size 8 terminates the list.
        if tag_type == 0 {
            break;
        }

        if tag_type == 6 {
            // mmap tag: type, size, entry_size, entry_version, entries...
            let entry_size = *((tag_addr + 8) as *const u32) as u64;
            let mut entry = tag_addr + 16;
            while entry + entry_size <= tag_addr + tag_size {
                let base = *(entry as *const u64);
                let length = *((entry + 8) as *const u64);
                let kind = *((entry + 16) as *const u32);

                f(MemoryRegion {
                    start: base,
                    length,
                    // Type 1 is conventional available RAM. Everything
                    // else (reserved, ACPI, NVS, bad) stays untouched.
                    usable: kind == 1,
                });

                entry += entry_size;
            }
        }

        // Tags are padded to an 8-byte boundary.
        tag_addr += (tag_size + 7) & !7;
    }
}

// ---------------------------------------------------------------------
// Physical frame allocator
// ---------------------------------------------------------------------

/// A bitmap allocator: one bit per 4 KiB frame, 1 = used.
///
/// A bitmap (rather than a bump pointer) means `free` actually works,
/// which the heap allocator below depends on.
pub struct FrameAllocator {
    bitmap: [u64; BITMAP_WORDS],
    total_frames: usize,
    used_frames: usize,
    /// Where to resume scanning, so repeated allocations don't rescan
    /// the whole bitmap from zero every time.
    next_hint: usize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        FrameAllocator {
            // Start with everything marked used; init() frees what the
            // firmware says is actually available. Failing closed like
            // this means an unparsed region is never handed out.
            bitmap: [u64::MAX; BITMAP_WORDS],
            total_frames: 0,
            used_frames: MAX_FRAMES,
            next_hint: 0,
        }
    }

    #[inline]
    fn set_used(&mut self, frame: usize) {
        if frame >= MAX_FRAMES {
            return;
        }
        let (word, bit) = (frame / 64, frame % 64);
        if self.bitmap[word] & (1 << bit) == 0 {
            self.bitmap[word] |= 1 << bit;
            self.used_frames += 1;
        }
    }

    #[inline]
    fn set_free(&mut self, frame: usize) -> bool {
        if frame >= MAX_FRAMES {
            return false;
        }
        let (word, bit) = (frame / 64, frame % 64);
        if self.bitmap[word] & (1 << bit) != 0 {
            self.bitmap[word] &= !(1 << bit);
            self.used_frames -= 1;
            return true;
        }
        false
    }

    #[inline]
    fn is_used(&self, frame: usize) -> bool {
        if frame >= MAX_FRAMES {
            return true;
        }
        self.bitmap[frame / 64] & (1 << (frame % 64)) != 0
    }

    fn mark_range_used(&mut self, start: u64, end: u64) {
        let first = (start / PAGE_SIZE) as usize;
        // Round the end up so a partially covered frame is still
        // reserved rather than half handed out.
        let last = ((end + PAGE_SIZE - 1) / PAGE_SIZE) as usize;
        for frame in first..last.min(MAX_FRAMES) {
            self.set_used(frame);
        }
    }

    fn mark_range_free(&mut self, start: u64, end: u64) {
        // Round the start up and the end down: only fully contained
        // frames are safe to hand out.
        let first = ((start + PAGE_SIZE - 1) / PAGE_SIZE) as usize;
        let last = (end / PAGE_SIZE) as usize;
        for frame in first..last.min(MAX_FRAMES) {
            if self.set_free(frame) {
                self.total_frames += 1;
            }
        }
    }

    /// Allocate one frame, returning its physical address.
    pub fn allocate(&mut self) -> Option<u64> {
        for frame in self.next_hint..MAX_FRAMES {
            if !self.is_used(frame) {
                self.set_used(frame);
                self.next_hint = frame + 1;
                return Some(frame as u64 * PAGE_SIZE);
            }
        }
        // Wrap around and retry from the beginning before giving up.
        for frame in 0..self.next_hint.min(MAX_FRAMES) {
            if !self.is_used(frame) {
                self.set_used(frame);
                self.next_hint = frame + 1;
                return Some(frame as u64 * PAGE_SIZE);
            }
        }
        None
    }

    /// Return a frame to the pool.
    pub fn free(&mut self, phys_addr: u64) {
        let frame = (phys_addr / PAGE_SIZE) as usize;
        self.set_free(frame);
        if frame < self.next_hint {
            self.next_hint = frame;
        }
    }

    pub fn used_frames(&self) -> usize {
        self.used_frames
    }

    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
}

pub static FRAME_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::new());

/// Allocate a frame and zero it. Page tables must start zeroed, or the
/// CPU will follow garbage entries.
pub(crate) fn allocate_zeroed_frame() -> Option<u64> {
    let frame = FRAME_ALLOCATOR.lock().allocate()?;
    unsafe {
        core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE as usize);
    }
    Some(frame)
}

// ---------------------------------------------------------------------
// Page tables
// ---------------------------------------------------------------------

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

impl PageTable {
    /// # Safety
    /// Only valid while `phys` is inside the identity-mapped window.
    unsafe fn from_phys<'a>(phys: u64) -> &'a mut PageTable {
        &mut *(phys as *mut PageTable)
    }
}

/// The level-4 table currently loaded in CR3.
///
/// # Safety
/// Caller must not create aliasing mutable references to it.
pub unsafe fn active_level4_table() -> &'static mut PageTable {
    PageTable::from_phys(read_cr3() & ADDR_MASK)
}

#[derive(Debug)]
pub enum MapError {
    OutOfFrames,
    /// The virtual address falls inside a 2 MiB huge page, which this
    /// simple mapper will not split.
    ParentIsHugePage,
    AlreadyMapped,
}

/// Map one 4 KiB virtual page to one 4 KiB physical frame.
pub fn map_page(virt: u64, phys: u64, flags: u64) -> Result<(), MapError> {
    let l4_index = ((virt >> 39) & 0x1FF) as usize;
    let l3_index = ((virt >> 30) & 0x1FF) as usize;
    let l2_index = ((virt >> 21) & 0x1FF) as usize;
    let l1_index = ((virt >> 12) & 0x1FF) as usize;

    unsafe {
        let l4 = active_level4_table();
        let l3 = next_table(l4, l4_index)?;
        let l2 = next_table(l3, l3_index)?;
        let l1 = next_table(l2, l2_index)?;

        if l1.entries[l1_index] & PAGE_PRESENT != 0 {
            return Err(MapError::AlreadyMapped);
        }

        l1.entries[l1_index] = (phys & ADDR_MASK) | flags | PAGE_PRESENT;
    }

    flush_tlb(virt);
    Ok(())
}

/// Unmap one 4 KiB page and return its frame to the allocator.
pub fn unmap_page(virt: u64) -> Result<(), MapError> {
    let l4_index = ((virt >> 39) & 0x1FF) as usize;
    let l3_index = ((virt >> 30) & 0x1FF) as usize;
    let l2_index = ((virt >> 21) & 0x1FF) as usize;
    let l1_index = ((virt >> 12) & 0x1FF) as usize;

    unsafe {
        let l4 = active_level4_table();
        let e4 = l4.entries[l4_index];
        if e4 & PAGE_PRESENT == 0 {
            return Err(MapError::AlreadyMapped);
        }
        let l3 = PageTable::from_phys(e4 & ADDR_MASK);
        let e3 = l3.entries[l3_index];
        if e3 & PAGE_PRESENT == 0 || e3 & PAGE_HUGE != 0 {
            return Err(MapError::ParentIsHugePage);
        }
        let l2 = PageTable::from_phys(e3 & ADDR_MASK);
        let e2 = l2.entries[l2_index];
        if e2 & PAGE_PRESENT == 0 || e2 & PAGE_HUGE != 0 {
            return Err(MapError::ParentIsHugePage);
        }
        let l1 = PageTable::from_phys(e2 & ADDR_MASK);
        let e1 = l1.entries[l1_index];
        if e1 & PAGE_PRESENT == 0 {
            return Err(MapError::AlreadyMapped);
        }
        let phys = e1 & ADDR_MASK;
        l1.entries[l1_index] = 0;
        FRAME_ALLOCATOR.lock().free(phys);
    }
    flush_tlb(virt);
    Ok(())
}

/// Follow an entry to the next level, allocating the table if needed.
unsafe fn next_table(
    table: &mut PageTable,
    index: usize,
) -> Result<&'static mut PageTable, MapError> {
    let entry = table.entries[index];

    if entry & PAGE_PRESENT != 0 {
        // A huge page here means there is no next level to walk into.
        if entry & PAGE_HUGE != 0 {
            return Err(MapError::ParentIsHugePage);
        }
        return Ok(PageTable::from_phys(entry & ADDR_MASK));
    }

    let frame = allocate_zeroed_frame().ok_or(MapError::OutOfFrames)?;
    // Intermediate entries are permissive; the final L1 entry is what
    // actually restricts access.
    table.entries[index] = frame | PAGE_PRESENT | PAGE_WRITABLE;
    Ok(PageTable::from_phys(frame))
}

/// Translate a virtual address to a physical one by walking the tables.
pub fn translate(virt: u64) -> Option<u64> {
    let l4_index = ((virt >> 39) & 0x1FF) as usize;
    let l3_index = ((virt >> 30) & 0x1FF) as usize;
    let l2_index = ((virt >> 21) & 0x1FF) as usize;
    let l1_index = ((virt >> 12) & 0x1FF) as usize;

    unsafe {
        let l4 = active_level4_table();
        let e4 = l4.entries[l4_index];
        if e4 & PAGE_PRESENT == 0 {
            return None;
        }

        let l3 = PageTable::from_phys(e4 & ADDR_MASK);
        let e3 = l3.entries[l3_index];
        if e3 & PAGE_PRESENT == 0 {
            return None;
        }
        if e3 & PAGE_HUGE != 0 {
            // 1 GiB page
            return Some((e3 & ADDR_MASK) + (virt & 0x3FFF_FFFF));
        }

        let l2 = PageTable::from_phys(e3 & ADDR_MASK);
        let e2 = l2.entries[l2_index];
        if e2 & PAGE_PRESENT == 0 {
            return None;
        }
        if e2 & PAGE_HUGE != 0 {
            // 2 MiB page — this is what the boot identity map uses.
            return Some((e2 & ADDR_MASK) + (virt & 0x1F_FFFF));
        }

        let l1 = PageTable::from_phys(e2 & ADDR_MASK);
        let e1 = l1.entries[l1_index];
        if e1 & PAGE_PRESENT == 0 {
            return None;
        }
        Some((e1 & ADDR_MASK) + (virt & 0xFFF))
    }
}

// ---------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------

/// Build the frame allocator from the firmware memory map.
///
/// # Safety
/// `multiboot_info_addr` must be the address GRUB passed in ebx.
pub unsafe fn init(multiboot_info_addr: u64) {
    let kernel_start = &__kernel_start as *const u8 as u64;
    let kernel_end = &__kernel_end as *const u8 as u64;

    let mut alloc = FRAME_ALLOCATOR.lock();

    // Step 1: release every region the firmware reports as usable.
    for_each_memory_region(multiboot_info_addr, |region| {
        if region.usable {
            let start = region.start;
            let end = (region.start + region.length).min(MAX_PHYS_ADDR);
            if start < end {
                alloc.mark_range_free(start, end);
            }
        }
    });

    // Step 2: take back everything that must not be handed out.

    // The first 1 MiB holds the BIOS/IVT/EBDA and the VGA buffer.
    alloc.mark_range_used(0, 0x100_000);

    // The kernel image itself, including .bss (which holds this very
    // bitmap and the boot page tables).
    alloc.mark_range_used(kernel_start, kernel_end);

    // The Multiboot2 info structure — we may still want to read it.
    if multiboot_info_addr != 0 {
        let total_size = *(multiboot_info_addr as *const u32) as u64;
        alloc.mark_range_used(multiboot_info_addr, multiboot_info_addr + total_size);
    }
}

/// Human-readable summary, for the boot banner.
pub fn stats() -> (usize, usize) {
    let alloc = FRAME_ALLOCATOR.lock();
    (alloc.used_frames(), alloc.total_frames())
}
