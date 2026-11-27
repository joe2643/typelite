# Compatibility and hardware coverage

Windows-specific boundaries and test coverage for the [Windows port](index.md).

- **Keyboards and elevated targets.** Check End swallowing, side-specific modifiers and Escape
  cancellation in ordinary editors. AltGr generates synthetic Control edges on some layouts.
  The native hook and `SendInput` are constrained by Windows integrity levels.
- **Window and clipboard behavior.** Check the capsule and Ask panel on mixed-DPI displays,
  keyboard/copy selection in Ask, dark select readability, and clipboard restoration when another
  app updates the clipboard. A borderless full-screen app can share the desktop with an overlay.
  Exclusive full-screen games control their own presentation.
- **Audio.** Check recording, permission reporting, default-output mute and crash recovery,
  and hands-free capture while other apps use the microphone.
- **Local inference and search.** Exercise built-in speech on CPU and Vulkan and confirm a long
  Qwen3-ASR recording finishes with the shared duration-scaled budget. Verify the AI and search
  loopback listeners and child-process cleanup. Compare CPU and GPU latency on representative PCs.
- **Portable package.** Launch the unpacked x64 archive on Windows 10 and 11 with WebView2 and
  the appropriate drivers installed. Authenticode, installer upgrades and signed Windows updates
  are future distribution work.
