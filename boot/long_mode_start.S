.intel_syntax noprefix

# ------------------------------------------------------------------
# Xdos boot stub (64-bit)
# Reached via a far jump from boot.S once long mode is active.
# Reloads the data segment registers, then calls into the Rust
# kernel entry point defined in src/main.rs.
# ------------------------------------------------------------------

.section .text
.code64
.global long_mode_start
.extern kernel_main
.extern multiboot_info_ptr

long_mode_start:
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    # Pass the Multiboot2 info address as the first argument (rdi).
    # A 32-bit load zero-extends into the full 64-bit register.
    mov edi, [multiboot_info_ptr]

    call kernel_main

    # kernel_main should never return, but halt safely if it does
.hang:
    cli
    hlt
    jmp .hang
