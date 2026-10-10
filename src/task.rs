//! A task: its own stack, saved stack pointer, and bookkeeping.
//!
use crate::memory::{self, MapError, PAGE_NO_EXECUTE, PAGE_PRESENT, PAGE_WRITABLE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    KernelThread,
    User,
}
use core::sync::atomic::{AtomicU64, Ordering};

/// 16 KiB per task. Small enough that several tasks don't dent the
/// 1 MiB heap, large enough for ordinary recursion and formatting.
pub const STACK_SIZE: u64 = 16 * 1024;

/// Virtual region task stacks are carved out of — clear of both the
/// boot identity map (below 1 GiB) and the kernel heap (0x4444_...).
const TASK_STACK_REGION_BASE: u64 = 0x_5555_5555_0000;

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_id() -> u64 {
    NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
}
static NEXT_STACK_SLOT: AtomicU64 = AtomicU64::new(0);

extern "C" {
    fn context_switch(current_rsp_slot: *mut u64, next_rsp: u64);
    fn task_trampoline();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
}

pub struct Task {
    pub id: u64,
    pub pid: u64,
    pub name: &'static str,
    pub state: TaskState,
    pub kind: TaskKind,
    /// CR3 for user processes; 0 keeps the kernel root table.
    pub cr3: u64,
    rsp: u64,
}

impl Task {
    /// Create a new task that will begin by calling `entry(arg)`.
    /// `entry` must never return — there is nothing to return to.
    pub fn new(
        name: &'static str,
        entry: extern "C" fn(u64) -> !,
        arg: u64,
    ) -> Result<Task, MapError> {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        Self::new_extended(id, id, name, entry, arg, TaskKind::KernelThread, 0)
    }

    pub fn new_extended(
        id: u64,
        pid: u64,
        name: &'static str,
        entry: extern "C" fn(u64) -> !,
        arg: u64,
        kind: TaskKind,
        cr3: u64,
    ) -> Result<Task, MapError> {
        let stack_top = allocate_task_stack()? + STACK_SIZE;
        let rsp = unsafe { init_stack(stack_top, entry, arg) };

        Ok(Task {
            id,
            pid,
            name,
            state: TaskState::Ready,
            kind,
            cr3,
            rsp,
        })
    }

    /// Pointer to this task's saved-rsp slot, for `context::switch`.
    pub fn rsp_slot(&mut self) -> *mut u64 {
        &mut self.rsp as *mut u64
    }

    pub fn saved_rsp(&self) -> u64 {
        self.rsp
    }
}

/// Switch from the task whose saved stack pointer lives at
/// `current_rsp_slot` onto the task whose saved stack pointer is
/// `next_rsp`.
///
/// # Safety
/// - `current_rsp_slot` must point at the live saved-rsp slot of the
///   task actually running right now.
/// - `next_rsp` must be a value previously produced by this module's
///   stack setup, or saved by a prior call to this function — never a
///   stack currently in use by another task.
pub unsafe fn switch(current_rsp_slot: *mut u64, next_rsp: u64) {
    context_switch(current_rsp_slot, next_rsp);
}

/// Build the initial stack contents for a task that has never run, so
/// the first switch into it lands in `task_trampoline` with `entry`
/// and `arg` ready in the registers it expects them in.
///
/// # Safety
/// `stack_top` must be the top (highest address) of a stack allocation
/// with room for at least 7 machine words below it, 16-byte aligned.
unsafe fn init_stack(stack_top: u64, entry: extern "C" fn(u64) -> !, arg: u64) -> u64 {
    // context_switch's pop sequence reads, in order: r15, r14, r13,
    // r12, rbp, rbx, then `ret` reads one more word as the return
    // address. So in memory, from the lowest address (where rsp will
    // point) to the highest:
    //   [+0]  r15   [+8]  r14   [+16] r13   [+24] r12
    //   [+32] rbp   [+40] rbx   [+48] return address
    // task_trampoline expects the entry fn in r12 and its argument in
    // r13, so those two slots carry them; everything else is unused
    // before a task's first run and is zeroed.
    let mut sp = stack_top as *mut u64;

    sp = sp.offset(-1);
    *sp = task_trampoline as u64; // return address, popped by `ret`

    sp = sp.offset(-1);
    *sp = 0; // rbx

    sp = sp.offset(-1);
    *sp = 0; // rbp

    sp = sp.offset(-1);
    *sp = entry as u64; // r12 -- task_trampoline calls this

    sp = sp.offset(-1);
    *sp = arg; // r13 -- task_trampoline passes this as the first arg

    sp = sp.offset(-1);
    *sp = 0; // r14

    sp = sp.offset(-1);
    *sp = 0; // r15

    sp as u64
}

/// Map a fresh `STACK_SIZE` region for a new task's stack and return
/// its base (lowest) address.
///
/// Each task gets its own private virtual region, never reused, with
/// one unmapped guard page after it: a stack overflow page-faults
/// instead of silently corrupting whatever memory came next.
fn allocate_task_stack() -> Result<u64, MapError> {
    let slot = NEXT_STACK_SLOT.fetch_add(1, Ordering::Relaxed);
    let stack_base = TASK_STACK_REGION_BASE + slot * (STACK_SIZE + memory::PAGE_SIZE);

    let page_count = STACK_SIZE / memory::PAGE_SIZE;
    for i in 0..page_count {
        let virt = stack_base + i * memory::PAGE_SIZE;
        let frame = memory::FRAME_ALLOCATOR
            .lock()
            .allocate()
            .ok_or(MapError::OutOfFrames)?;
        memory::map_page(virt, frame, PAGE_PRESENT | PAGE_WRITABLE | PAGE_NO_EXECUTE)?;
    }
    // The page at stack_base + STACK_SIZE is deliberately left unmapped
    // as a guard page; the next task's stack starts one page beyond it.

    Ok(stack_base)
}
