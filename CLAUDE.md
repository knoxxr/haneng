# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

haneng is a **cross-platform Korean/English IME state indicator**: hover the mouse over a text input and a badge appears **just above the text caret** showing the current mode — blue **한** (Korean) / gray **a** (English lowercase) / orange **A** (English + Caps Lock). Windows, macOS, and Linux (X11). That is the whole product as of v0.4.0. (The mouse hover is the *trigger*; the badge is *positioned* at the caret. Linux keeps mouse-relative positioning — X11 has no caret-position API.)

**History**: the project originally shipped a full wrong-mode typo auto-corrector (dubeolsik/sebeolsik conversion engine, detection with dictionaries+bigrams, hotkey conversion, C FFI). After real-device testing the user cut all conversion features — too error-prone in practice — keeping only the hover badge. The complete implementation is preserved at git tag `v0.3.2`; PLAN.md describes the original (now historical) design. Do not resurrect conversion features without an explicit request.

## Commands

- `cargo test` — all tests
- `cargo clippy --all-targets` — lint (keep warning-free)
- `cargo check -p haneng-windows --target x86_64-pc-windows-msvc` / `cargo check -p haneng-linux --target x86_64-unknown-linux-gnu` — cross-check those daemons from macOS
- `cargo run -p haneng-macos` — the macOS daemon (`hanengd`) builds and runs natively on this dev machine (darwin) — useful for smoke tests; needs Accessibility permission to actually detect text areas
- `cargo run -p haneng-settings` — settings window (runs on any OS)
- Release: `scripts/release.sh <ver>` (version bump + CHANGELOG + test + commit + tag), then `git push origin main --tags` → release.yml builds Windows MSI/zip, macOS .app zip, and Linux tar.gz into a draft release; publish with `gh release edit <tag> --draft=false --latest`.

## Architecture

All three daemons are the same product per OS: detect "mouse is over a text input", read Korean/English + Caps Lock, and show/hide a small following badge. Each shares a 3-state `Mode` enum (한 / a / A). No daemon manipulates text or sends network traffic; only the settings app touches the network (update check, opt-in).

- **haneng-windows** (binary `hanengw`): `WH_MOUSE_LL` hook (mouse move only — **no keyboard hook**); `indicator.rs` renders a layered/click-through/no-activate topmost GDI badge window (I-beam cursor comparison detects text areas → trigger; 50ms throttle; 300ms refresh timer so 한/영·Caps changes and caret movement show without mouse movement). Badge is **positioned at the caret**, not the mouse: `ime.rs::caret_screen_rect` reads the focused control's `GUITHREADINFO.rcCaret` (+ `ClientToScreen`); apps with no real Win32 caret (Chrome/Electron) report none → badge hidden. `ime.rs::query_korean_mode` reads focused-control IME open status via `WM_IME_CONTROL`/`IMC_GETOPENSTATUS` + `SendMessageTimeoutW`; Caps via `GetKeyState(VK_CAPITAL)` toggle bit. `tray.rs` (tray-icon; muda items not Send → thread_local pattern). `windows_subsystem = "windows"`.
- **haneng-macos** (binary `hanengd`, **buildable/runnable on the dev machine**): tao event loop + `WaitUntil` poll timer (~120ms); `badge.rs` an AppKit `NSWindow` overlay via **objc2/objc2-app-kit** (borderless, `NSStatusWindowLevel`, `ignoresMouseEvents`, all-spaces; an `NSTextField` renders the glyph — AppKit draws Korean natively, no rasterizer); `ax.rs` detects text inputs via Accessibility (`AXUIElementCopyElementAtPosition` role check → trigger — needs Accessibility permission, prompted via `AXIsProcessTrustedWithOptions`) and reads the **caret rect** for positioning via `caret_bounds_at` (`AXSelectedTextRange` → `AXBoundsForRange` parameterized attribute; none → badge hidden); `tis.rs` Korean via `TISCopyCurrentKeyboardInputSource`; `mac_input.rs` cursor location + Caps via CoreGraphics FFI. `ActivationPolicy::Accessory` (no dock icon).
- **haneng-linux** (binary `hanengl`, X11, **experimental — never run on real hardware, only cross-checked**): poll loop; `render.rs` rasterizes the badge to a BGRX buffer with **fontdue** (system Korean font) — X can't easily do transparent/rounded, so it's an opaque square pushed via `put_image` to an override-redirect window; text-input detection via **XFixes cursor name** (`get_cursor_image_and_name` → "xterm"/"text"); Caps via pointer-query Lock mask; **Korean mode has no X11 query API** so it observes the Hangul toggle key via **XRecord** in a background thread (the one place a daemon observes keys — toggle only, documented). `linux_toggle_keycodes` config.
- **haneng-settings** (eframe/egui, glow backend — default wgpu breaks Windows via gpu-allocator/windows conflict): badge on/off, initial mode, and the **update button** (`update.rs`: GitHub releases API + one-click MSI upgrade on Windows, browser open elsewhere; ureq with explicit TLS provider — Windows must select native-tls; 15s timeout; catch_unwind so the spinner can't hang). egui has no CJK glyphs — system Korean font loaded at runtime. eframe 0.35 renamed `App::update` to `App::ui`.
- **haneng-core**: legacy conversion-era library kept compiling with tests. Still used: `config.rs` (`key = value` config with `extras`, `Config::extra`/`set_extra`). `Mode` is duplicated per-daemon (tiny enum) rather than shared, to avoid coupling.
- **haneng-datagen**: dev tool regenerating core's embedded data (legacy modules only).

## Hard-won environment facts

- BSD sed (macOS) silently no-ops GNU `0,/re/` addresses — release.sh uses python for the version substitution and verifies the result.
- PowerShell 5.1 reads BOM-less scripts as ANSI — `scripts/package-windows.ps1` must stay ASCII-only; CI invokes it with `pwsh`.
- Win11's new Korean IME does not answer `IMC_GETCONVERSIONMODE`; `IMC_GETOPENSTATUS` is the query that works. Never trust an unanswered query as "english" — 0 is ambiguous.
- Cross-checking crates that pull `ring` (ureq/rustls) fails locally without a C cross-toolchain; the CI runners build them natively. That's why `haneng-settings` ureq uses native-tls on Windows.
- objc2-app-kit main-thread-only types (`NSWindow`, `NSTextField`) need `use objc2::MainThreadOnly` for `alloc(mtm)`; most AppKit setters are safe wrappers (no `unsafe` block needed) — the compiler's unused-unsafe warning tells you which.
- `render`/`fontdue` in haneng-linux is `#[cfg(target_os = "linux")]`-gated because fontdue is a linux-only dependency; otherwise `cargo test --workspace` on other hosts fails to resolve it. Same pattern keeps each daemon's platform deps off other hosts (every real module is cfg-gated; only the shared `Mode` enum compiles everywhere).
- The Linux daemon builds with zero system C deps (x11rb + fontdue are pure Rust); only haneng-settings needs the runner's GUI libs, which ubuntu-latest already provides (proven by the v0.1.x Linux release builds — don't add a speculative apt step).

## Versioning

Single version source: `[workspace.package] version` in the root Cargo.toml (all crates use `version.workspace = true`). Keep CHANGELOG.md (Keep a Changelog) updated under `[Unreleased]` as you work.
