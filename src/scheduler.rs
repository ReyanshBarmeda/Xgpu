//! Round-robin preemptive scheduler.
//!
//! The ready queue is a `VecDeque<Box<Task>>`. Each timer tick, the
//! running task goes to the back and the task at the front runs next —
//! the simplest fair policy there is. No priorities or sleep queues
//! yet; those are natural extensions once this baseline is solid.
//!
//! # Locking around the actual switch
//!
//! The lock is held only long enough to pick the next task and extract
//! two raw pointers — never across the `task::switch` call itself. That
//! call may not return for an arbitrarily long time (until this task's
//! next turn), so holding the lock across it would freeze every other
//! caller for just as long. The pattern is: lock, mutate the queue,
//! extract pointers, unlock, *then* switch.

use crate::cpu::write_cr3;
use crate::memory::{MapError, ADDR_MASK};
use crate::process::Process;
use crate::task::TaskKind;
use crate::sync::Mutex;
use crate::task::{Task, TaskState};
use alloc::boxed::Box;
use alloc::collections::VecDeque;

struct SchedulerState {
    /// Tasks waiting for a turn. The running task is deliberately not
    /// in here — it's tracked by `current` — so a task can never appear
    /// twice no matter how this interleaves with the timer interrupt.
    ready_queue: VecDeque<Box<Task>>,
    current: Option<Box<Task>>,
}

impl SchedulerState {
    const fn new() -> Self {
        SchedulerState {
            ready_queue: VecDeque::new(),
            current: None,
        }
    }
}

static SCHEDULER: Mutex<SchedulerState> = Mutex::new(SchedulerState::new());

/// Add a task to the ready queue. Safe to call before or after the
/// scheduler has started running.
pub fn spawn(
    name: &'static str,
    entry: extern "C" fn(u64) -> !,
    arg: u64,
) -> Result<(), MapError> {
    spawn_with_pid(0, name, entry, arg, TaskKind::KernelThread, 0)
}

pub fn spawn_with_pid(
    pid: u64,
    name: &'static str,
    entry: extern "C" fn(u64) -> !,
    arg: u64,
    kind: TaskKind,
    cr3: u64,
) -> Result<(), MapError> {
    let task = if pid == 0 {
        Box::new(Task::new(name, entry, arg)?)
    } else {
        let id = task::next_id();
        Box::new(Task::new_extended(id, pid, name, entry, arg, kind, cr3)?)
    };
    crate::cpu::without_interrupts(|| {
        SCHEDULER.lock().ready_queue.push_back(task);
    });
    Ok(())
}

pub fn spawn_user_process(proc: Process) -> Result<(), MapError> {
    let pid = proc.pid;
    let name = proc.name;
    let cr3 = proc.address_space.cr3();
    let ptr = crate::process::leak_process_box(proc);
    spawn_with_pid(
        pid,
        name,
        crate::process::user_process_wrapper,
        ptr as u64,
        TaskKind::User,
        cr3,
    )
}

/// Start multitasking: take the first ready task and switch onto its
/// stack. Called once, from `kernel_main`, after every other subsystem
/// is up and at least one task has been spawned.
///
/// # Safety
/// Must be called exactly once, with interrupts already enabled. This
/// never returns: the calling stack (kernel_main's boot-time stack) is
/// not itself a `Task` the scheduler knows about, so nothing will ever
/// switch back to it.
pub unsafe fn start() -> ! {
    let mut discard_rsp: u64 = 0;

    let (next_rsp, next_cr3) = {
        let mut sched = SCHEDULER.lock();
        let mut next = sched
            .ready_queue
            .pop_front()
            .expect("scheduler::start called with no tasks spawned");
        next.state = TaskState::Running;
        let rsp = next.saved_rsp();
        let cr3 = next.cr3;
        sched.current = Some(next);
        (rsp, cr3)
    };

    activate_cr3(next_cr3);
    crate::task::switch(&mut discard_rsp as *mut u64, next_rsp);

    unreachable!("kernel_main's boot stack is never scheduled back to");
}

/// Called from the timer interrupt handler on every tick, after the
/// PIC has already been sent its EOI (see the comment in
/// `interrupts::timer_interrupt_handler`) — this may not return for
/// many ticks, and the PIC will not deliver IRQ0 again until it has
/// been acknowledged.
pub fn on_timer_tick() {
    let (current_rsp_slot, next_rsp, next_cr3) = {
        let mut sched = SCHEDULER.lock();

        // Nothing to do until start() has run and there's someone else
        // to switch to.
        if sched.current.is_none() || sched.ready_queue.is_empty() {
            return;
        }

        let mut current = sched.current.take().unwrap();
        if current.kind == TaskKind::User {
            sched.current = Some(current);
            return;
        }
        current.state = TaskState::Ready;

        let mut next = sched.ready_queue.pop_front().unwrap();
        next.state = TaskState::Running;
        let next_rsp = next.saved_rsp();

        // The pointer handed to task::switch must stay valid after this
        // lock is dropped, so the Task it points into must not move.
        // Box gives it a stable heap address; leaking that address out
        // while an owning Box lives on in the queue is exactly the
        // documented use for into_raw/from_raw.
        let current_ptr = Box::into_raw(current);
        sched
            .ready_queue
            .push_back(unsafe { Box::from_raw(current_ptr) });
        let current_rsp_slot = unsafe { (*current_ptr).rsp_slot() };

        let next_cr3 = next.cr3;
        sched.current = Some(next);

        (current_rsp_slot, next_rsp, next_cr3)
    };

    activate_cr3(next_cr3);
    unsafe {
        crate::task::switch(current_rsp_slot, next_rsp);
    }
    // Execution resumes here only when this task is scheduled again,
    // possibly many other tasks' turns later. Falling off the end of
    // this function returns into timer_interrupt_handler's own
    // epilogue — see boot/context_switch.S's header comment for why
    // that's what restores this task's interrupt-flag state.
}

/// Total number of tasks known to the scheduler (running + ready).
pub fn task_count() -> usize {
    crate::cpu::without_interrupts(|| {
        let sched = SCHEDULER.lock();
        sched.ready_queue.len() + sched.current.is_some() as usize
    })
}

pub fn current_task_id() -> Option<u64> {
    crate::cpu::without_interrupts(|| {
        SCHEDULER.lock().current.as_ref().map(|t| t.id)
    })
}

pub fn current_pid() -> Option<u64> {
    crate::cpu::without_interrupts(|| {
        SCHEDULER.lock().current.as_ref().map(|t| t.pid)
    })
}

pub fn current_process_kind() -> Option<TaskKind> {
    crate::cpu::without_interrupts(|| {
        SCHEDULER.lock().current.as_ref().map(|t| t.kind)
    })
}

fn activate_cr3(cr3: u64) {
    if cr3 != 0 {
        unsafe {
            write_cr3(cr3);
        }
    } else {
        unsafe {
            write_cr3(crate::cpu::read_cr3() & ADDR_MASK);
        }
    }
}

/// Name of the currently running task, if the scheduler has started.
pub fn current_task_name() -> Option<&'static str> {
    crate::cpu::without_interrupts(|| {
        SCHEDULER.lock().current.as_ref().map(|t| t.name)
    })
}
