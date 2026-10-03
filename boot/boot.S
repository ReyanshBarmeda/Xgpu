.intel_syntax noprefix

# ------------------------------------------------------------------
# Xdos boot stub (32-bit)
# Entered by a Multiboot2-compliant loader (e.g. GRUB) in protected
# mode. Verifies the CPU, builds identity-mapped page tables for the
# first 1 GiB, switches on long mode, then far-jumps into the 64-bit
# trampoline (long_mode_start.S) which hands off to Rust.
# ------------------------------------------------------------------

.section .multiboot_header, "a"
header_start:
    .long 0xe85250d6                          # magic
    .long 0                                    # architecture: i386
    .long header_end - header_start            # header length
    .long -(0xe85250d6 + 0 + (header_end - header_start))  # checksum

    # framebuffer tag (type 5) — request linear FB when firmware provides one
    .align 8
    .word 5
    .word 0
    .long 20
    .long 0
    .long 0
    .long 0

    # end tag
    .word 0
    .word 0
    .long 8
header_end:

.section .text
.code32
.global start
.extern long_mode_start

start:
    mov esp, offset stack_top

    # GRUB leaves the Multiboot2 info structure address in ebx. CPUID
    # clobbers ebx, so stash it immediately; long_mode_start.S reloads it
    # and passes it to kernel_main. The high 4 bytes stay zero because
    # the ELF loader zeroes .bss and the pointer is always below 4 GiB.
    mov [multiboot_info_ptr], ebx

    call check_multiboot
    call check_cpuid
    call check_long_mode

    call set_up_page_tables
    call enable_paging

    lgdt [gdt64_pointer]

    jmp 0x08:long_mode_start

check_multiboot:
    cmp eax, 0x36d76289
    jne .no_multiboot
    ret
.no_multiboot:
    mov al, '0'
    jmp error

check_cpuid:
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 1 << 21
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov al, '1'
    jmp error

check_long_mode:
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode

    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode
    ret
.no_long_mode:
    mov al, '2'
    jmp error

set_up_page_tables:
    # L4[0] -> L3
    mov eax, offset page_table_l3
    or eax, 0b11
    mov [page_table_l4], eax

    # L3[0] -> L2
    mov eax, offset page_table_l2
    or eax, 0b11
    mov [page_table_l3], eax

    # Identity-map the first 1 GiB with 2 MiB pages
    mov ecx, 0
.map_l2_loop:
    mov eax, 0x200000
    mul ecx
    or eax, 0b10000011          # present + writable + huge page
    mov [page_table_l2 + ecx * 8], eax

    inc ecx
    cmp ecx, 512
    jne .map_l2_loop

    ret

enable_paging:
    mov eax, offset page_table_l4
    mov cr3, eax

    mov eax, cr4
    or eax, 1 << 5               # PAE
    mov cr4, eax

    mov ecx, 0xC0000080          # EFER MSR
    rdmsr
    or eax, 1 << 8               # LME: long mode enable
    or eax, 1 << 11              # NXE: allow the no-execute bit (63) in
                                 # page table entries. Without this, any
                                 # entry with bit 63 set is treated as
                                 # having a reserved bit and faults.
    wrmsr

    mov eax, cr0
    or eax, 1 << 31              # paging
    mov cr0, eax

    ret

# al holds an ASCII error code; prints "ERR: <code>" in red on white
error:
    mov dword ptr [0xb8000], 0x4f524f45
    mov dword ptr [0xb8004], 0x4f3a4f52
    mov dword ptr [0xb8008], 0x4f204f20
    mov byte ptr  [0xb800a], al
    hlt

.section .bss
.balign 4096
page_table_l4:
    .skip 4096
page_table_l3:
    .skip 4096
page_table_l2:
    .skip 4096
stack_bottom:
    .skip 4096 * 4
stack_top:

.global multiboot_info_ptr
multiboot_info_ptr:
    .skip 8

.section .rodata
.balign 8
gdt64:
    .quad 0                                            # null descriptor
gdt64_code:
    .quad (1<<43) | (1<<44) | (1<<47) | (1<<53)         # exec, code/data, present, long-mode
gdt64_end:
gdt64_pointer:
    .word gdt64_end - gdt64 - 1
    .quad gdt64
