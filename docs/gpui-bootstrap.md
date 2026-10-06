# GPUI bootstrap provenance and qualification

Sela pins Zed/GPUI at
[`a84689073d296dfd39987bc7dd478e43ef76d83a`](https://github.com/zed-industries/zed/commit/a84689073d296dfd39987bc7dd478e43ef76d83a)
(2026-10-03), Rust 1.98.1. Inspected before implementation:

- `crates/gpui/README.md`, `examples/hello_world.rs`: standalone window setup.
- `crates/gpui/examples/testing.rs`: actions, key contexts, retained focus handles,
  `TestAppContext` and `VisualTestContext`. These are not native GPU tests.
- `crates/gpui_platform/src/gpui_platform.rs`: native platform construction.
- `crates/gpui_linux/src/linux/display_connection.rs`: shared platform event loop.
- `crates/gpui_windows/src/{platform,window,vsync}.rs`: shared UI-thread draw
  coordinator, per-HWND renderer, compositor timing.
- Framework/platform `Cargo.toml` and `crates/ui/Cargo.toml`: license boundaries.

GPUI/platform backends declare Apache-2.0; Zed's application `ui` crate declares
GPL-3.0-or-later. Sela uses framework APIs and original UI code, not Zed application
components or assets. Transitive license review remains a distribution gate.
Cargo.lock fixes transitive versions; feature defaults on transitive edges mean
`default-features = false` does not guarantee every upstream default is disabled.

One package is sufficient for bootstrap. `src/main.rs` owns the operator window;
future domain modules must remain independent of GPUI. The minimal window is
explicitly a technical preview, not a service editor or parity claim. Its quit
action is a development control, not an observed EasyWorship shortcut.

## Build and run

Linux: run `.agents/setup`, then `cargo build --locked` and `cargo run --locked`
inside a graphical X11/Wayland session with Vulkan support and installed fonts.
Use `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`
and `cargo test --locked` for available automated checks. `--version` requires no
display. A native launch without display variables may choose upstream's headless
platform; this is not a rendered-window check.

Windows: install Git, Rust through rustup, Visual Studio 2022 Build Tools with
Desktop development with C++, the Windows SDK, and CMake; use an x64 native tools
PowerShell. Run `rustup toolchain install`, `cargo build --locked`,
`cargo test --locked`, then `cargo run --locked`. Native launch requires a supported
DirectX adapter/driver. Verify window opening, resizing, Ctrl+Q and titlebar close.
Windows CI is only build/headless coverage; it cannot qualify actual displays.

Verified 2026-10-06 on Windows 11 Home (i7-14650HX, Intel UHD + RTX 4060 Laptop,
three displays at mixed scale). Prerequisites installed without an admin shell:

```powershell
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100 --add Microsoft.VisualStudio.Component.VC.CMake.Project --includeRecommended"
winget install --id Python.Python.3.12 -e --scope user   # native drivers only
```

An ordinary PowerShell (not a developer prompt) suffices: rustup's MSVC target
locates the Build Tools. The repository's `rust-toolchain.toml` installs 1.98.1
on first `cargo` use. A clean debug `cargo build --locked --all-targets` took
about five minutes. Run `python scripts/native-smoke-windows.py` for the native
launch/resize/quit check described in [native testing](native-testing.md).

GPUI's Windows backend selected the RTX 4060 with a Direct3D 11.1 device for the
operator window. In debug builds it logs `0x887A002D` when the optional Windows
"Graphics Tools" feature is absent; that only disables the DXGI debug layer.
This is operator-window evidence, not audience-renderer or DX12 qualification.

### Orb native rendering caveat

On this Debian 12 orb, Mesa 22.3.6 llvmpipe Vulkan selected/configured successfully
but Xvfb captures were black. Software OpenGL rendered the same build correctly
with `VK_DRIVER_FILES=/dev/null`, intentionally disabling Vulkan for this test.
Do not export that override globally or infer physical GPU support from this run.
The inspected path used Xvfb `:99`, Openbox, xcompmgr and `DISPLAY=:99`, with a
private `XDG_RUNTIME_DIR` (mode 700). Root-window captures worked; direct-window
captures returned black or hung, so capture the root then crop to window geometry.
`RUST_LOG=info` exposes upstream adapter/backend initialization on stderr; no file
logger or network telemetry is enabled. M0-03 will own reproducible native checks.

## Independent audience gate

A second GPUI window is **not** an independent audience compositor: event/render
work shares UI execution. M0-04 must test a distinct paced renderer boundary,
including UI stalls and process failure. The M0-10 Windows gate remains mandatory
before investing in the full workspace UI. Linux software Vulkan evidence cannot
replace it; no provisional renderer choice is described as production-qualified.
