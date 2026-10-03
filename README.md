# Xdos

A from-scratch x86_64 kernel. Hand-written assembly takes the CPU from
the 32-bit protected mode a Multiboot2 loader hands you, through page
table setup, into 64-bit long mode, then calls into a `no_std` Rust
kernel that owns everything from there.

No `bootloader` crate, no `bootimage`, and **zero crates.io
dependencies** — the spinlock, VGA driver, serial driver, IDT, GDT, PIC,
PIT, keyboard, frame allocator, page mapper and heap are all here in
readable form. GRUB does the Multiboot2 loading; the assembly and linker
script do the rest.

## What works

| Subsystem | File | Notes |
|---|---|---|
| Boot to long mode | `boot/boot.S` | Multiboot2, CPUID checks, 1 GiB identity map, PAE + long mode |
| GDT + TSS | `src/gdt.rs` | Includes an IST stack so stack overflow panics instead of rebooting |
| Interrupts (IDT) | `src/interrupts.rs` | Divide-by-zero, breakpoint, invalid opcode, GPF, page fault, double fault |
| PIC remapping | `src/pic.rs` | Moves IRQs off the exception vectors to 0x20-0x2F |
| Timer | `src/pit.rs` | PIT at 100 Hz; uptime counter and `sleep_ms` |
| Keyboard | `src/keyboard.rs` | PS/2 scancode set 1, shift + caps lock, 128-byte ring buffer |
| Physical memory | `src/memory.rs` | Multiboot2 memory map parsing, bitmap frame allocator (alloc **and** free) |
| Virtual memory | `src/memory.rs` | 4 KiB page mapper, table walking, address translation |
| Kernel heap | `src/allocator.rs` | Linked-list allocator with coalescing; `Box`, `Vec`, `String` all work |
| Output | `src/vga_buffer.rs`, `src/serial.rs` | `println!` to VGA, `serial_println!` to COM1 |

On boot it runs a self-test (breakpoint exception, heap alloc/free/
realloc, page translation, timer tick check), then echoes your keystrokes.

## Prerequisites

- **Rust nightly** with `rust-src` (needed for `-Z build-std`):
  ```
  rustup toolchain install nightly
  rustup component add rust-src --toolchain nightly
  ```
  The included `rust-toolchain.toml` selects nightly automatically.
- **GNU binutils** (`as`, `ld`) — standard on Linux.
- **`grub-mkrescue`** + **`xorriso`** — `apt install grub-pc-bin xorriso`
- **QEMU** — `apt install qemu-system-x86`

Linux is by far the easiest environment here. On macOS or Windows, use a
VM or container rather than fighting cross-toolchain differences.

## Build & run

```
make run          # QEMU window (VGA) + serial in your terminal
make run-serial   # serial only, no window - good over SSH
make debug        # QEMU waits for gdb on :1234
make clean
```

Expected output:

```
Xdos
================================
[ ok ] GDT + TSS loaded
[ ok ] IDT loaded
[ ok ] PIC remapped, PIT at 100 Hz
[ ok ] frame allocator: ... KiB usable, ... KiB in use
[ ok ] kernel heap: 1024 KiB mapped
[ ok ] interrupts enabled
EXCEPTION: BREAKPOINT at 0x...
[ ok ] self-test passed
[ ok ] timer ticking (5 ticks in ~50 ms)

Xdos ready. Type something:
```

## Boot sequence

1. **`boot/boot.S`** — GRUB loads this at 1 MiB in 32-bit protected mode.
   It stashes the Multiboot2 info pointer from `ebx` (CPUID clobbers
   `ebx`, so this happens first), verifies Multiboot2/CPUID/long-mode
   support, identity-maps the first 1 GiB with 2 MiB pages, enables
   PAE + long mode + paging, and far-jumps to 64-bit code.
2. **`boot/long_mode_start.S`** — reloads segment registers, puts the
   Multiboot2 pointer in `rdi`, calls `kernel_main`.
3. **`src/lib.rs`** — brings up subsystems in dependency order: GDT (the
   IDT references its selectors) -> IDT (so faults are reportable) -> PIC
   (remap before enabling interrupts) -> frame allocator -> heap -> `sti`.

## Verification status — please read

I verified everything I could without a Rust toolchain or emulator in my
environment:

- `boot.S` and `long_mode_start.S` assemble cleanly, and I checked the
  disassembly: the pre-long-mode code really is 32-bit encoded, the far
  jump encodes correctly, and the Multiboot2 pointer plumbing works.
- The full image links against the linker script, with `__kernel_start`
  and `__kernel_end` resolving correctly.
- The Multiboot2 header was validated byte-for-byte: magic, checksum
  arithmetic summing to zero, end tag, 8-byte alignment, and placement
  within the spec's 32 KiB limit.
- GDT/TSS/IDT descriptor bit-packing was checked against the Intel
  layout (TSS size 104, access byte 0x89, code-segment L bit, gate type
  0xE, base/limit round-trips).
- Page-table index math, heap/identity-map collision safety, canonical
  address form, bitmap sizing, and the PIT divisor were all verified
  numerically.
- Both keyboard scancode tables were checked for exact length and index
  alignment against real scancode set 1.

**What I could not do: compile the Rust, or boot it.** There is no
`rustc` or `qemu` available to me and no network access to install them.
The Rust is written carefully and the logic is checked, but it has never
been through a compiler. Expect to hit some compile errors on the first
`make run` — paste them to me and I'll fix them fast.

## Known limitations

- **1 GiB memory ceiling.** The frame allocator only manages memory
  inside the boot identity map, because page tables are dereferenced
  directly at their physical addresses. Going beyond this means
  extending the boot mapping or moving to a higher-half kernel with a
  physical-memory offset mapping.
- **Panicking while holding the VGA or serial lock will deadlock.** A
  fault raised inside `_print` would try to re-take a lock it already
  holds. The real fix is a `force_unlock` on the panic path.
- **`active_level4_table()` hands out `&'static mut` repeatedly**, which
  is technically aliasing UB. It is the usual hobby-kernel shortcut and
  works in practice, but it is not sound.
- **No APIC, no SMP.** The PIC and PIT are single-core legacy hardware.
  Multiprocessing needs the APIC and IO-APIC.
- **Keyboard is US layout, no extended (0xE0) scancodes** — so no arrow
  keys yet.
- **The heap is fixed at 1 MiB** and cannot grow.

## Userland (C#, Figma, easy apps)

User-facing work lives under **`userland/`**:

- **ABI** — `userland/abi/syscall_numbers.json` (single source of truth for C, Rust, C#).
- **Windows host tools** — `.\build.ps1 host` runs .NET codegen and scaffolds apps.
- **Figma** — export variables → `.\build.ps1 sync-figma -FigmaExport export.json` → `userland/design/tokens.json`.
- **Kernel** — ring-3 GDT segments, `syscall`/`sysret` MSRs, and `src/syscall.rs` dispatch (`write`, `read`, `yield`, `getpid`, `exit`).

See **`userland/README.md`** for the full checklist. Kernel milestone status: **`ROADMAP.md`** (items 1–15).

## Next steps

The natural order from here:

1. **Ring-3 entry** — `iretq` into mapped flat binaries at `0x0040_0000` (`src/userland.rs`).
2. **True processes** — separate page tables per user task; `getpid` already stubbed.
3. **A filesystem** — an in-memory one first, then a real block driver.
4. **A shell userland app** — C# or C app using the stable syscall ABI + Figma tokens for chrome.

Adding C modules is also straightforward whenever you want them: a
`build.rs` using the `cc` crate compiles `csrc/*.c` freestanding, and
Rust calls in through `extern "C"`.
