# Xdos roadmap (1–15)

Status key: **done** = usable baseline, **partial** = scaffold + some behavior, **planned** = designed only.

| # | Area | Status | Notes |
|---|------|--------|-------|
| 1 | Harden boot/CPU | partial | NXE in boot.S; `cpu::init_boot()` (FPU/SSE, CR0/CR4); Multiboot tag parse |
| 2 | Memory management | partial | `AddressSpace`, CR3 switch, `unmap_page`, `free` wired |
| 3 | Scheduler/processes | partial | `Process` + `Task` link; user tasks not preempted yet |
| 4 | User mode | partial | `enter_user_mode` (iretq); spawn via ELF |
| 5 | Syscalls | partial | write/read/yield/exit/getpid/open/close/ipc_send/recv |
| 6 | ELF loader | partial | ELF64 `PT_LOAD`, `ET_EXEC` |
| 7 | VFS | partial | RAMFS + file descriptors |
| 8 | Driver framework | partial | register/probe/init chain |
| 9 | PCI | partial | config-space scan + vendor/device log |
| 10 | Mouse/HID | partial | PS/2 aux init + packet decode (polled) |
| 11 | Framebuffer | partial | Multiboot2 FB tag → `framebuffer` module |
| 12 | IPC | partial | single kernel message queue |
| 13 | Networking | planned | `net` trait + stub driver slot |
| 14 | Security/isolation | partial | per-process CR3 clone; `security::verify_user_range` on syscalls |
| 15 | GUI | partial | `gui` fills rects on framebuffer when present |

Next hardening targets: preemptive user scheduling (save iret frames), SMP/APIC, copy-on-write fork, real NIC (virtio).
