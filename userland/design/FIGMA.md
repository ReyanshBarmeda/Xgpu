# Figma ↔ Xdos userland

Design shell and app chrome in Figma; ship tokens the kernel and userland can consume without hand-copying hex values.

## Recommended Figma setup

1. Create a file **Xdos Userland** with pages **Shell**, **Components**, **Tokens**.
2. Use **Variables** (not only local styles):
   - Collection `color` — modes: `default`, `dark` (optional).
   - Collection `layout` — numbers for margins, 80×25 grid spacing.
3. Name variables to match `userland/design/tokens.json` paths, e.g. `color/fg/default`, `layout/shell/marginX`.

## Export → repo

After changing variables in Figma:

1. **Plugins → Variables export** (or your team’s JSON export) to a file such as `figma-export.json`.
2. From the repo root (Windows):

   ```powershell
   dotnet run --project userland/host/Xdos.Host -- sync-figma path\to\figma-export.json
   ```

   This merges recognized keys into `userland/design/tokens.json` and regenerates C# `ThemeTokens.cs` for host-side previews.

## Using tokens in code

| Layer | File |
|-------|------|
| Host / C# tools | `userland/host/Xdos.Host/Generated/ThemeTokens.cs` |
| Future GUI userland | Read JSON or generated constants |
| VGA text (today) | `color.*.vga` → `vga_buffer::Color` index |

When you add a real framebuffer UI, treat Figma frames as **80×25 character cells** first so designs match what QEMU shows today.
