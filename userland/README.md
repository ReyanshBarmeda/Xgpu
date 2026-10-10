# Xdos userland

Add apps without touching the kernel every time. One ABI drives **C**, **Rust**, and **C# (Native AOT)**; **Figma** variables feed shell chrome via `design/tokens.json`.

## Layout

```
userland/
  abi/           syscall_numbers.json + include/xdos/syscall.h
  apps/          one folder per app (_template = copy source)
  design/        tokens.json + FIGMA.md
  host/          .NET 8 tools (Windows-friendly codegen & scaffolding)
```

## Windows workflow (C# + Figma)

From `e:\xdos`:

```powershell
.\build.ps1 host          # codegen + build Xdos.Host
.\build.ps1 new-app Shell # scaffold userland/apps/Shell
.\build.ps1 sync-figma path\to\figma-export.json
.\build.ps1 kernel        # optional: WSL make run
```

## Add an app (checklist)

1. **Pick a language**
   - **C#**: `dotnet run --project userland/host/Xdos.Host -- new-app MyApp` then implement `Program.cs` using `Xdos.Abstractions.Syscall`.
   - **C**: include `userland/abi/include/xdos/syscall.h`, link as flat binary at `0x0040_0000`.
   - **Rust**: copy `_template` patterns from the kernel `extern "C"` tasks (full ring-3 crate coming next).

2. **Keep syscalls in sync** — edit `abi/syscall_numbers.json`, then:

   ```powershell
   dotnet run --project userland/host/Xdos.Host -- codegen
   ```

3. **Load into the kernel** (flat image today):

   ```rust
   userland::load_flat_binary("myapp", include_bytes!("../apps/myapp/build/app.bin"))?;
   ```

   Ring-3 `iretq` entry is the next kernel milestone; syscalls and GDT user segments are already wired.

## Syscall surface (v1)

| # | Name | Purpose |
|---|------|---------|
| 1 | write | fd 1 = serial, 2 = VGA text |
| 2 | read | one key from keyboard buffer |
| 3 | yield | cooperative hint |
| 39 | getpid | current kernel task id |
| 60 | exit | stop task (Linux-compatible #) |

Calling convention: `syscall` with number in `rax`, args in `rdi, rsi, rdx, r10, r8, r9`.

## Figma

See [design/FIGMA.md](design/FIGMA.md). Variable names should mirror `design/tokens.json` so `sync-figma` can merge exports without manual edits.
