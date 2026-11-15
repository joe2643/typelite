# OraType Windows v3 — Task Checklist

Base branch: `draft/windows-port` (HEAD `33543f2`)
Target portable artifact: `dist-win/oratype-win-portable-v3.zip` (do not modify v1/v2 zips)

- [x] **Item 1 (P0)**: Fix dictated text not reaching target text field on Windows
  - [x] Prevent `capsule` / `ask` overlay `show()` from stealing foreground (`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, `SW_SHOWNOACTIVATE`)
  - [x] Capture non-overlay foreground `HWND` + PID + exe before `Preparing` state & exclude OraType overlays in `app_detector`
  - [x] Attempt focus restore (`SetForegroundWindow` + `AttachThreadInput`) before paste if foreground drifted
  - [x] Migrate Windows default output to `clipboard` + `clipboardPaste`, route Windows `keyboard` mode to `WindowsSendInputOutput`, harden clipboard retries (`OpenClipboard`), `SendInput` scan codes + count check, `>= 750ms` sequence-checked clipboard restore
  - [x] Add `INFO` telemetry logs (start/pre-paste foreground `HWND`+exe, clipboard write timing & seq, `SendInput` return count, restore timing & seq)
  - [x] Unit tests covering focus restore, Windows migration, sequence guard, retry, and `SendInput` validation
- [x] **Item 2 (P1)**: Fix `stt/silence.rs` Windows voice check falsely rejecting Razer Seiren Mini quiet speech
  - [x] Keep macOS behavior/constants 100% unchanged; add Windows profile (`-58 dBFS` floor, SNR-aware short-voiced acceptance)
  - [x] Unit tests with exact log tuples (`-37/-100/160ms`, `-42/-100/180ms`, `-37/-69/200ms`) + silence/hiss/clicks/bumps negative tests
- [x] **Item 3 (P2)**: Built-in AI & Speech backend status & persisted selector (Auto / Vulkan GPU / CPU)
  - [x] Persist `builtin_ai_backend` and `builtin_speech_backend` in `AppConfig` + TypeScript `appStore`
  - [x] `llama-server` arg selection (`Auto`, specific `VulkanN`, CPU `--n-gpu-layers 0 --device none --no-mmproj-offload --mmproj-device none`) + restart running servers when config changes
  - [x] `whisper-rs` GPU/CPU selection + unload/reload when config changes
  - [x] Tauri `get_builtin_backend_info` command + reusable UI card in both AI (`LlmPane`) and Speech (`BuiltinSettings`) settings panes + 7 locales
  - [x] Rust unit tests + Vitest tests
- [x] **Item 4 (P3)**: Fix main window semi-transparency on Windows
  - [x] Windows 11 Mica (`DwmSetWindowAttribute` `DWMSBT_MAINWINDOW`) + Windows 10 / Mica failure solid opaque fallback (do not alter `tauri.conf.json` macOS config or overlay transparency)
  - [x] Theme dark/light sync (`DWMWA_USE_IMMERSIVE_DARK_MODE` + solid background color) + crisp CSS surfaces
  - [x] Unit tests for backdrop selection
- [x] **Item 5 (P4)**: Fix `<select>` / `<option>` white-on-white dropdown readability
  - [x] CSS `color-scheme` following dark/light theme + opaque dark backgrounds + explicit `option` / `optgroup` colors across all `<select>` elements
  - [x] Vitest CSS regression test
- [x] **Item 6 (P5)**: Fix Built-in search SearXNG `.tar.gz` unpack failure on Windows
  - [x] Pure-Rust `.tar.gz` extractor on Windows (`flate2` + `tar`) skipping `utils/templates`, symlinks, and Windows-illegal characters while rejecting path traversal
  - [x] Keep macOS `tar` path untouched + unit tests
- [x] **Item 7 (P6)**: Fix "Mac" wording on Windows + implement Windows "Mute other audio while recording"
  - [x] Windows Core Audio COM (`IMMDeviceEnumerator` + `IAudioEndpointVolume`) mute/unmute + startup crash recovery in `audio/output_mute.rs`
  - [x] Windows PC copy variants across all 7 locales (`en`, `zh-Hant`, `zh`, `ja`, `de`, `es`, `fr`) + Vitest parity/render tests
- [x] **Item 8 (P7)**: Measure/diagnose & safely optimize Built-in speech server (`Qwen3-ASR`) slow scaling
  - [x] Send `temperature=0` and bounded `max_tokens` for `LlamaAsrProvider` (`greedy_asr_limits: true`) to eliminate unbounded stochastic decoding on longer/trailing-silence clips
  - [x] Forward `llama-server` `WARN` and `mtmd`/`clip`/encode diagnostics in `llm/builtin.rs` `log_level_for`
  - [x] Document exact code-line root cause & unit tests
- [x] **Build, QA, Package & Commit**
  - [x] `cargo fmt --all -- --check`
  - [x] `cargo clippy --lib --bins` (0 new warnings on Linux & `x86_64-pc-windows-gnu`)
  - [x] `cargo test --lib`
  - [x] `npm test` (Vitest) & `npx tsc --noEmit`
  - [x] `tauri build --target x86_64-pc-windows-gnu --no-bundle` + package `dist-win/oratype-win-portable-v3.zip` (verify v1/v2 untouched)
  - [x] `x86_64-w64-mingw32-objdump -p` all `.exe`/`.dll` + Wine smoke test
  - [x] Local commit on `draft/windows-port` via `git commit -F` (no push, no PR, no binaries committed)
