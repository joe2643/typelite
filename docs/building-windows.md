# Building the Windows desktop app

The Windows draft packages Munia as an unsigned portable ZIP for Windows 10/11 x64.

## Build on Windows

Use Node.js 24, Git for Windows with Git Bash, stable x64 Rust (MSVC), Visual Studio 2022 with
Desktop development with C++, CMake, LLVM/libclang and the x64 Vulkan SDK (headers, import library and
`glslc`). Open a Visual Studio x64 developer PowerShell with those tools on `PATH` and
`VULKAN_SDK` set to the SDK directory.

From the repository root:

```powershell
cd apps/desktop
npm ci
./scripts/build-portable-windows.ps1
```

The script writes `Munia-<version>-windows-x64.zip` under the Cargo target directory's
`portable` folder. Extract the entire `Munia` folder before running `munia.exe`. Keep the
llama-server files next to it. The machine needs the Microsoft Edge WebView2 Runtime, the x64
Visual C++ runtime, and the Vulkan loader supplied by a compatible graphics driver. Selecting
CPU in Settings changes where inference runs.
On a fresh Windows profile, launch at login is opt-in.

Use a short absolute `CARGO_TARGET_DIR` if MSBuild hits the Windows path length limit. With Ninja
available in the x64 developer shell, setting `CMAKE_GENERATOR=Ninja` avoids the nested MSBuild
project paths. Set `LIBCLANG_PATH` to the folder containing `libclang.dll` if bindgen cannot find
it. Whisper's bindings must be generated for the Windows ABI.

For MinGW development builds, `ORATYPE_WIN_VULKAN_SDK` can point to a Vulkan SDK with `include/`
headers and a GNU `lib/libvulkan.dll.a` import library. CMake finds `glslc` on `PATH` or uses the
compiler supplied through `ORATYPE_WIN_GLSLC`. The portable package uses MSVC.

## Test

From `apps/desktop`, run `npm run build` and `npm test`. Then run the Rust checks from
`apps/desktop/src-tauri`:

```powershell
cargo fmt --check
cargo test --locked --target x86_64-pc-windows-msvc --lib -- --test-threads=1
```

For a debug run with local AI, use Git Bash from `apps/desktop` to run
`bash scripts/fetch-llama-server-windows.sh` first. It stages the verified runtime under
`src-tauri/binaries/win-vulkan-x64`. You can also reuse the pinned archive by setting
`ORATYPE_LLAMA_ZIP_CACHE` to its path.
