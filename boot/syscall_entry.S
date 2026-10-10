# AMD64 SYSCALL entry — must match userland/abi/syscall_numbers.json
# At entry: rax=num; rdi, rsi, rdx, r10, r8, r9 = args 1..6; rcx=return RIP; r11=RFLAGS.

    .global syscall_entry
    .code64
syscall_entry:
    push    %rcx
    push    %r11
    push    %r9
    push    %r8
    push    %r10
    push    %rdx
    push    %rsi
    push    %rdi

    mov     %rax, %rdi          # num -> rdi
    mov     %rsi, 0(%rsp)       # a1
    mov     %rdx, 8(%rsp)       # a2
    mov     %rcx, 16(%rsp)      # a3
    mov     %r8, 24(%rsp)       # a4
    mov     %r9, 32(%rsp)       # a5
    push    40(%rsp)            # a6 (slot below saved rcx/r11)

    call    syscall_dispatch

    add     $8, %rsp            # drop a6 push

    add     $48, %rsp           # drop saved arg registers (keep rcx/r11)

    pop     %rcx                # user RIP for sysret
    pop     %r11                # user RFLAGS for sysret
    sysretq
