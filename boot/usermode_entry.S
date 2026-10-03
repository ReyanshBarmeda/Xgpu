.intel_syntax noprefix

# void enter_user_mode(uint64_t entry_rip, uint64_t user_stack_top);
# Ring-3 segments must match src/gdt.rs (USER_DATA 0x33, USER_CODE 0x2B).

.section .text
.code64
.global enter_user_mode
enter_user_mode:
    mov r8, rdi                 # entry RIP
    mov r9, rsi                 # user RSP

    push 0x33                   # SS (user data + RPL3)
    push r9                     # RSP
    pushfq
    pop rax
    or rax, (1 << 9)            # IF=1 in user mode
    push rax                    # RFLAGS
    push 0x2B                   # CS (user code + RPL3)
    push r8                     # RIP
    iretq
