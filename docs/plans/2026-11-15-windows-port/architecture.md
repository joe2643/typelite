# Architecture

Windows integrations for the [Windows port](index.md). Source paths below are relative to
`apps/desktop`.

| Area | macOS | Windows |
| --- | --- | --- |
| Shortcuts | Core Graphics event tap | `src-tauri/src/hotkey/listener/windows.rs` runs `WH_KEYBOARD_LL` on a message-loop thread. `hotkey.rs` and `hotkey/listener.rs` share registration, chord matching and capture. |
| Paste and selection | Accessibility and System Events, `NSPasteboard` snapshot | `output/clipboard.rs`, `output/pasteboard.rs` and `selection.rs` use atomic `SendInput`, a modifier guard and a sequence check before clipboard restore. |
| Foreground | `NSWorkspace` and Accessibility | `app_detector/platform/windows.rs` identifies the active executable for app profiles. `output/windows_focus.rs` tracks a window handle and process/thread IDs and logs handle and process ID only. |
| Floating windows | Nonactivating `NSPanel` | `overlay_window.rs` uses `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW` and `HWND_TOPMOST`. Ask can temporarily take focus so its text can be selected. |
| Main window | Sidebar material and native titlebar | `windows_backdrop.rs` selects Mica or an opaque fallback. The frontend styles dark selects for the Windows backdrop. |
| Microphone and mute | AVFoundation permissions, Core Audio | `commands/permissions.rs` reads microphone ConsentStore values. `audio/output_mute.rs` uses `IMMDeviceEnumerator` and `IAudioEndpointVolume` with a crash marker. |
| Local STT and AI | Metal for Whisper and llama-server | `stt/hardware.rs` detects hardware with DXGI and filters software and virtual adapters. `stt/builtin.rs` and `llm/builtin.rs` support Vulkan and CPU choices. Qwen3-ASR uses the shared duration-scaled limits and timeouts. |
| Built-in web search | macOS uv archive and Python venv | `search_server.rs` extracts the Windows uv ZIP and SearXNG tarball, checks paths and link entries and uses the Windows Python venv layout. |
| Credentials and paths | Keychain, Application Support and Library Logs | Windows Credential Manager service `Munia`, `%APPDATA%\dev.koelony.munia` for configuration, `munia.db` and models, and `%LOCALAPPDATA%\Munia\Logs\munia.log`. |
| Packaging | Signed macOS release and updater | `scripts/build-portable-windows.ps1` stages the verified llama-server executable and DLLs next to `munia.exe`. The Windows draft has no self-update path. |

The Windows listener decodes Win32 key edges and tracks auto-repeat before passing them to the
shared matcher. It ignores injected events during ordinary operation so Munia's own keyboard
output does not trigger a shortcut. Elevated targets still have Windows integrity-level
restrictions.

Paste restoration checks that the recording window still has the same handle, process and thread.
Reading editability, selected ranges and text before the caret through Windows UI Automation
remains outside this implementation. Selection capture can fall back to the clipboard.
