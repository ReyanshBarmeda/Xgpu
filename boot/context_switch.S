.intel_syntax noprefix

# ------------------------------------------------------------------
# Task context switch.
#
# A switch means: push every callee-saved register onto the *current*
# stack, save that stack pointer, load the *next* task's saved stack
# pointer, and pop its callee-saved registers back off. Execution then
# resumes wherever that stack's return address points: task_trampoline
# for a task that has never run, or the instruction right after a
# previous call to context_switch for one that was switched out before.
#
# Only callee-saved registers (rbx, rbp, r12-r15) need saving. System V
# AMD64 guarantees the caller-saved ones are already dead across any
# `call`, and context_switch is called like any ordinary extern "C" fn.
#
# RFLAGS (and so the interrupt flag) is deliberately NOT saved or
# restored here. A task is normally preempted from inside the timer
# interrupt handler, where the CPU already cleared IF on entry via the
# IDT's interrupt gate. When that task is later resumed, execution lands
# back inside its own suspended timer_interrupt_handler call, which
# finishes and returns through the compiler-generated iretq — and *that*
# is what restores the original RFLAGS/IF. context_switch just needs to
# stay out of the way of that pairing. The one case it doesn't cover is
# a task's very first run, which arrives via a plain `ret`, not `iretq`;
# task_trampoline below issues its own `sti` for exactly that reason.
# ------------------------------------------------------------------

.section .text
.code64
.global context_switch

# extern "C" fn context_switch(current_rsp_slot: *mut u64, next_rsp: u64)
# System V: rdi = current_rsp_slot, rsi = next_rsp
context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    mov [rdi], rsp        # save outgoing stack pointer
    mov rsp, rsi           # switch onto the incoming stack

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    ret                    # jumps to whatever return address is now on top

.global task_trampoline
task_trampoline:
    # See the file-level comment: nothing else will ever re-enable
    # interrupts for a task's first run, since it never passes back
    # through an iretq the way a resumed task does.
    sti

    # init_task_stack (in task.rs) placed the entry function pointer in
    # r12 and its argument in r13 before this task ever ran, precisely
    # because context_switch's pop sequence above loads them there.
    mov rdi, r13           # System V: first argument goes in rdi
    call r12               # entry(arg) -- declared to never return

    # Defensive only: entry() should never return. If it somehow does,
    # halt rather than run off into whatever garbage follows in memory.
    cli
9:
    hlt
    jmp 9b
