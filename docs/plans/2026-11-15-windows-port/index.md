# Windows port (Windows 10/11 x64)

Munia's Windows desktop app uses the existing Tauri frontend, Rust pipeline and local AI providers.
It adds Windows keyboard, audio, window and clipboard integration and a portable x64 build from
`apps/desktop`.

Status: building - 2026-11-28.

## Goals and non-goals

- Support Dictate, Translate and Ask on Windows 10/11 x64.
- Use native Windows tests and a portable build to verify the port. Device and desktop integration
  still need testing on Windows hardware.
- Preserve the macOS bundle identifier, Keychain entries, signed updater and release workflow.
- The draft covers a portable build. Installer packaging, Authenticode and Windows self-updates
  remain outside its scope.

## Key decisions

- Use `Ctrl+/` (Dictate), `Ctrl+.` (Ask), and `Ctrl+Shift+/` (Translate) as Windows defaults
  because PC keyboards expose different keys from Mac keyboards.
- Keep native shortcut matching and registration shared. Use a Win32 `WH_KEYBOARD_LL` hook for
  side-specific modifiers, keys such as End, and Escape cancellation during a run.
- Use Win32 clipboard and `SendInput` for paste, and preserve an ordinary foreground window if
  the user switched apps. The paste helper restores the recording window only when focus is
  missing or on a Munia overlay.
- Use Vulkan for compatible GPUs and offer a CPU choice for built-in speech and AI. DXGI and the
  bundled llama-server report different views of the available hardware, so the port reconciles
  the device selection before setting each engine's options.
- Build the portable package with native x64 MSVC, CMake and the Vulkan SDK. Download the pinned
  Windows llama-server runtime and verify its digest before packaging.
- Use the Windows-only app identifier `dev.koelony.munia`, Windows Credential Manager, and a
  separate log under `%LOCALAPPDATA%\Munia\Logs`. Windows has no Typelite release data to migrate.
- Use Mica on Windows where supported and an opaque main window elsewhere. The floating capsule and
  Ask window use Win32 overlay styles.

## Parts

| File | What it covers |
| --- | --- |
| [architecture.md](architecture.md) | Windows integrations and their macOS counterparts |
| [roadmap.md](roadmap.md) | Compatibility limits and hardware test coverage |
| [Building the Windows desktop app](../../building-windows.md) | Native toolchain and portable packaging |

## Open questions

- Which Windows editors and layouts need additional clipboard or key-mapping work, especially
  elevated targets, AltGr and mixed-DPI setups?
- Does Vulkan performance on representative Intel, AMD and NVIDIA machines justify a separate
  CUDA package later?
- Which installer, signing and update strategy should follow a validated portable build?
