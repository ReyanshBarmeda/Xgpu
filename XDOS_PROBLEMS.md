# XDOS PROBLEMS

## Rule
Every AI working on xdos must:
- Read this file before working.
- Add every newly discovered problem here.
- Include the affected file, problem, impact, and suggested fix.
- Mark problems as fixed when verified.

## Known Problems

1. User address-space mapping conflicts with 2 MiB boot huge pages.
2. User tasks are not preemptively scheduled.
3. Ring-3 context switching does not yet preserve a full `iretq` frame.
4. Syscall entry ABI/register mapping needs verification.
5. `sys_exit` does not actually terminate a task.
6. `sys_yield` does not actually yield.
7. User pointer validation uses a broad fixed range instead of page-table validation.
8. ELF loader needs overflow, bounds, address, and permission hardening.
9. User memory layout needs a defined non-overlapping design.
10. Per-process address-space isolation needs stronger verification.
11. Scheduler/process lifecycle is incomplete.
12. Ring-3 interrupt handling needs safety review.
13. IPC lifecycle needs hardening.
14. VFS/file-descriptor lifecycle needs hardening.
15. Driver/PCI support is partial.
16. Mouse/HID support is partial.
17. Framebuffer/GUI support is partial.
18. Windows WSL/.NET build environment is incomplete.
19. Testing coverage is insufficient.

## Priority
Fix mapping first, then syscall ABI, user context switching, preemptive scheduling, exit/yield, memory validation, ELF hardening, and process lifecycle.
