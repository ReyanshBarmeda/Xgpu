//! Kernel heap.
//!
//! Maps a fixed region of virtual memory, backs it with physical frames,
//! and runs a free-list allocator over it. Once `init` has run, `alloc`
//! types work: `Box`, `Vec`, `String`, `BTreeMap`.
//!
//! The allocator is a sorted singly-linked list of free regions with
//! coalescing on free. That is slower than a slab or buddy allocator
//! under heavy load, but it reclaims memory properly and is small enough
//! to audit by eye, which matters more at this stage.

use crate::memory::{self, PAGE_SIZE, PAGE_NO_EXECUTE, PAGE_PRESENT, PAGE_WRITABLE};
use crate::sync::Mutex;
use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::ptr;

/// Virtual base of the heap. Chosen well away from the identity-mapped
/// low 1 GiB so it cannot collide with it.
pub const HEAP_START: u64 = 0x_4444_4444_0000;
/// 1 MiB. Enough for real data structures without eating the frame pool.
pub const HEAP_SIZE: u64 = 1024 * 1024;

struct FreeRegion {
    size: usize,
    next: Option<&'static mut FreeRegion>,
}

impl FreeRegion {
    const fn new(size: usize) -> Self {
        FreeRegion { size, next: None }
    }

    fn start(&self) -> usize {
        self as *const Self as usize
    }

    fn end(&self) -> usize {
        self.start() + self.size
    }
}

pub struct LinkedListAllocator {
    /// Dummy head node; `head.next` is the first real free region.
    head: FreeRegion,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: FreeRegion::new(0),
        }
    }

    /// # Safety
    /// The caller must guarantee `[start, start + size)` is mapped,
    /// writable, and otherwise unused.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.add_free_region(start, size);
    }

    /// Push a region onto the free list, keeping it sorted by address
    /// and merging with neighbours where possible.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // Every region must be able to hold a FreeRegion node, and be
        // aligned for one.
        assert_eq!(align_up(addr, mem::align_of::<FreeRegion>()), addr);
        assert!(size >= mem::size_of::<FreeRegion>());

        // Walk to the insertion point, keeping the list address-ordered
        // so that adjacent regions can be found and merged.
        let mut current = &mut self.head;
        loop {
            let next_start = match current.next.as_ref() {
                Some(next) => next.start(),
                None => usize::MAX,
            };
            if next_start >= addr {
                break;
            }
            current = current.next.as_mut().unwrap();
        }

        // Merge forward: does the new region run straight into the next?
        let mut size = size;
        let merge_forward = match current.next.as_ref() {
            Some(next) => addr + size == next.start(),
            None => false,
        };
        if merge_forward {
            let mut next_node = current.next.take().unwrap();
            size += next_node.size;
            current.next = next_node.next.take();
        }

        // Merge backward: does the previous region run into this one?
        // (`current` is the dummy head when there is no previous.)
        if current.size > 0 && current.end() == addr {
            current.size += size;
            return;
        }

        let node_ptr = addr as *mut FreeRegion;
        let mut node = FreeRegion::new(size);
        node.next = current.next.take();
        node_ptr.write(node);
        current.next = Some(&mut *node_ptr);
    }

    /// Find a region big enough for `size`/`align`, unlink it, and
    /// return it together with the aligned start address.
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut FreeRegion, usize)> {
        let mut current = &mut self.head;

        while current.next.is_some() {
            // Borrow-checker dance: inspect, then decide.
            let fits = {
                let region = current.next.as_ref().unwrap();
                Self::check_region(region, size, align).is_ok()
            };

            if fits {
                let alloc_start = {
                    let region = current.next.as_ref().unwrap();
                    Self::check_region(region, size, align).unwrap()
                };
                let next = current.next.as_mut().unwrap().next.take();
                let region = current.next.take().unwrap();
                current.next = next;
                return Some((region, alloc_start));
            }

            current = current.next.as_mut().unwrap();
        }

        None
    }

    /// Can `region` satisfy this request? Returns the aligned start.
    fn check_region(region: &FreeRegion, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end() {
            return Err(());
        }

        // Any leftover tail must itself be large enough to hold a
        // FreeRegion node, or it would be unreclaimable.
        let excess = region.end() - alloc_end;
        if excess > 0 && excess < mem::size_of::<FreeRegion>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    /// Round a request up so every allocation can later be turned back
    /// into a FreeRegion node when freed.
    fn adjust_layout(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<FreeRegion>())
            .expect("alignment adjustment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<FreeRegion>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Mutex<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::adjust_layout(layout);

        // Interrupt handlers can allocate, so this lock must not be
        // held with interrupts enabled.
        crate::cpu::without_interrupts(|| {
            let mut allocator = self.lock();
            match allocator.find_region(size, align) {
                Some((region, alloc_start)) => {
                    let alloc_end = alloc_start + size;
                    let excess = region.end() - alloc_end;
                    if excess > 0 {
                        allocator.add_free_region(alloc_end, excess);
                    }
                    alloc_start as *mut u8
                }
                None => ptr::null_mut(),
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::adjust_layout(layout);
        crate::cpu::without_interrupts(|| {
            self.lock().add_free_region(ptr as usize, size);
        });
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[global_allocator]
pub static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

/// Map the heap region and hand it to the allocator.
pub fn init() -> Result<(), memory::MapError> {
    let page_count = HEAP_SIZE / PAGE_SIZE;

    for i in 0..page_count {
        let virt = HEAP_START + i * PAGE_SIZE;
        let frame = memory::FRAME_ALLOCATOR
            .lock()
            .allocate()
            .ok_or(memory::MapError::OutOfFrames)?;

        // Heap memory holds data, never code.
        memory::map_page(virt, frame, PAGE_PRESENT | PAGE_WRITABLE | PAGE_NO_EXECUTE)?;
    }

    unsafe {
        ALLOCATOR
            .lock()
            .init(HEAP_START as usize, HEAP_SIZE as usize);
    }

    Ok(())
}
