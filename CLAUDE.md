# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

haneng is a **Windows Korean/English IME state indicator**: hover the mouse over a text input (I-beam cursor) and a badge next to the cursor shows the current mode (blue 한 / gray A). That is the whole product as of v0.4.0.

**History**: the project originally shipped a full wrong-mode typo auto-corrector (dubeolsik/sebeolsik conversion engine, detection with dictionaries+bigrams, hotkey conversion, daemons for macOS/Windows/Linux, C FFI). After real-device testing the user cut all conversion features — too error-prone in practice — keeping only the hover badge. The complete implementation is preserved at git tag `v0.3.2`; PLAN.md describes the original (now historical) design. Do not resurrect conversion features without an explicit request.

## Commands

- `cargo test` — all tests
- `cargo clippy --all-targets` — lint (keep warning-free)
- `cargo check -p haneng-windows --target x86_64-pc-windows-msvc` — cross-check the daemon (build/run needs real Windows; CI's windows runner validates natively)
- `cargo run -p haneng-settings` — settings window (runs on any OS)
- Release: `scripts/release.sh <ver>` (version bump + CHANGELOG + test + commit + tag), then `git push origin main --tags` → release.yml builds MSI/zip on windows-latest into a draft release; publish with `gh release edit <tag> --draft=false --latest`.

## Architecture

- **haneng-windows** (binary `hanengw`): tray-resident indicator. `WH_MOUSE_LL` hook (mouse move only — **no keyboard hook, no text manipulation, no network**); `indicator.rs` renders a layered/click-through/no-activate topmost badge window (I-beam cursor comparison detects text areas; 50ms throttle; GDI paint); `ime.rs::query_korean_mode` reads the focused control's IME open status via `WM_IME_CONTROL`/`IMC_GETOPENSTATUS` with `SendMessageTimeoutW` (50ms — a hung target must not stall us; None = unsupported, fall back to last known value); `tray.rs` (tray-icon crate; muda items are not Send → thread_local + same-thread handler pattern). Release builds use `windows_subsystem = "windows"`. Config extras: `hover_indicator`, `initial_mode`, `ime_query`.
- **haneng-settings** (eframe/egui, glow backend — the default wgpu backend breaks Windows builds via gpu-allocator/windows version conflicts): badge on/off, initial mode, and the **update button** (`update.rs`: GitHub releases API check + one-click MSI upgrade on Windows; ureq with explicit TLS provider — Windows must select native-tls or it fails at runtime; 15s timeout; catch_unwind so the spinner can never hang forever). egui has no CJK glyphs — system Korean font loaded at runtime. eframe 0.35 renamed `App::update` to `App::ui`.
- **haneng-core**: mostly a legacy library from the conversion era (hangul automaton, detector, lexicon — unused by the product but kept compiling with tests). The parts still used: `config.rs` (plain `key = value` config with `extras` for adapter-specific keys, `Config::extra`/`set_extra`) and nothing else.
- **haneng-datagen**: dev tool regenerating core's embedded data (only relevant to legacy modules).

## Hard-won environment facts

- BSD sed (macOS) silently no-ops GNU `0,/re/` addresses — release.sh uses python for the version substitution and verifies the result.
- PowerShell 5.1 reads BOM-less scripts as ANSI — `scripts/package-windows.ps1` must stay ASCII-only; CI invokes it with `pwsh`.
- Win11's new Korean IME does not answer `IMC_GETCONVERSIONMODE`; `IMC_GETOPENSTATUS` is the query that works. Never trust an unanswered query as "english" — 0 is ambiguous.
- Cross-checking crates that pull `ring` (ureq/rustls) fails locally without a C cross-toolchain; the CI runners build them natively.

## Versioning

Single version source: `[workspace.package] version` in the root Cargo.toml (all crates use `version.workspace = true`). Keep CHANGELOG.md (Keep a Changelog) updated under `[Unreleased]` as you work.
