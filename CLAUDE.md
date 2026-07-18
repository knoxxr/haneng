# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

haneng is a cross-platform (Windows/macOS/Linux) desktop utility that detects and corrects Korean/English input-mode mistakes — text typed in the wrong IME mode, e.g. `gksrmf` intended as `한글`, or `ㅗ디ㅣㅐ` intended as `hello` (the same problem space as Punto Switcher for Russian). The full development plan, requirements, and roadmap live in **PLAN.md** — read it before making design decisions, and keep it updated when decisions change.

## Commands

Rust cargo workspace (`crates/haneng-core` engine, `crates/haneng-cli` demo/dev tool, `crates/haneng-macos` daemon `hanengd`, `crates/haneng-windows` daemon `hanengw`):

- `cargo test` — build + run all tests (unit, integration `tests/accuracy.rs`, doctests)
- `cargo test -p haneng-core detect` — run tests matching a name filter
- `cargo clippy --all-targets` — lint (keep it warning-free)
- `cargo run -q -p haneng-cli -- "gksrmf dlqfur"` — run the engine on text (auto-correct per word; `--to-hangul`/`--to-english` force direction, `-v` shows per-word verdicts, stdin lines when no text args)
- `cargo run -p haneng-macos` — start `hanengd`, the macOS daemon (needs Accessibility + Input Monitoring permission for the invoking terminal; ⌘⇧Space converts the last word and toggles the input source; `--no-tray` for headless runs)
- `cargo check -p haneng-windows --target x86_64-pc-windows-msvc` — cross-check the Windows daemon from macOS (build/run requires a Windows machine; hotkey is Ctrl+Shift+Space)
- `cargo check -p haneng-linux --target x86_64-unknown-linux-gnu` — cross-check the Linux X11 daemon (`hanengl`; hotkey Ctrl+Shift+Space)
- `cargo build --release` — distributable binaries (lto + strip configured in the workspace profile)

`tests/accuracy.rs` gates every detector change: the hand-labeled seed corpus (MUST_KEEP must stay at zero conversions across all sensitivities) plus corpus-driven gates that replay the embedded dictionaries as simulated typos (`corpus_recall_*` ≥ 90%, `corpus_zero_false_positives`). Run `cargo test -p haneng-core --test accuracy metrics_survey -- --ignored --nocapture` for a recall report per sensitivity (Balanced measured ~99%/98% with 0 FP).

Generated data: `crates/haneng-core/src/lexicon_data.rs` (FrequencyWords, CC-BY-SA 4.0) and `crates/haneng-core/src/sebeolsik_data.rs` (libhangul keyboard XMLs) are **generated** — never edit by hand. Regenerate with `cargo run -p haneng-datagen` after downloading sources into `data/` (curl commands in `crates/haneng-datagen/src/main.rs`). haneng-datagen must NOT depend on haneng-core (bootstrap: core needs the generated files to compile).

Layouts: `Layout::{Dubeolsik, Sebeolsik390, SebeolsikFinal}` (`layout = sebeolsik-390` etc. in config.txt) threads through `Composer::with_layout`/`feed_key`, `*_with` conversion functions, `Detector::with_layout`, and `build_replace_plan`. Sebeolsik jamo are position-explicit (`KeyJamo::Cho/Jung/Jong` vs dubeolsik `Dual`): no 도깨비불 (`jong_explicit` flag), doubled choseong combine by repeating the key (`combine_cho` — 390 has no direct ㄲ/ㅃ cho keys), and digits/punctuation can be word keys (daemons reclassify `Boundary`→`Letter` via `is_word_key`). Sebeolsik correctness is gated by corpus-wide roundtrip tests in accuracy.rs.

## Architecture

- **haneng-core** (implemented, platform-independent, zero deps): `hangul.rs` jamo tables and syllable compose/decompose; `layout.rs` dubeolsik QWERTY↔jamo mapping; `compose.rs` the dubeolsik input automaton (`Composer`, incl. 도깨비불 jongseong migration) for eng→han; `decompose.rs` han→eng key recovery; `detect.rs` per-word wrong-mode `Detector` v2 (structure gate → dictionary-first evidence comparison with per-sensitivity margins + `TARGET_EVIDENCE_FLOOR`; bigrams act only as negative filters because well-formed fakes score high — see `lexicon.rs` `score_survey`); `lexicon.rs` + generated `lexicon_data.rs` (10k EN / 20k KO dictionaries, char/jamo bigram log-prob tables); `tracker.rs` the shared `WordBuffer`/`KeyClass` last-word tracker used by every adapter; `plan.rs` `build_replace_plan` computing backspace count (IME preedit = jamo-per-key vs committed = char-per-key) and replacement text; `auto.rs` `AutoCorrector::on_word_committed` turning a boundary-committed word into an `AutoDecision` (replacement + revert text + mode switch + exception word); `config.rs` dependency-free `key = value` config and exception-dictionary persistence (`config.txt`/`exceptions.txt` under the per-OS config dir). Detection deliberately keeps fully-composed hangul that might be English (ambiguous without dictionaries/n-grams — future work). All conversion and detection logic must stay pure and unit-testable with no platform dependencies.
- **Platform adapters** (one crate per OS): key-event observation and text injection.
  - macOS (**implemented**, `crates/haneng-macos`, binary `hanengd`): listen-only `CGEventTap` classifies physical keycodes (`keymap.rs`) into the core `WordBuffer`; ⌘⇧Space builds a `ReplacePlan` and injects marked events (`inject.rs`, `EVENT_SOURCE_USER_DATA` marker so the tap skips its own events); `tis.rs` wraps TIS FFI for input-source query/switch and `IsSecureEventInputEnabled` (hard stop when secure input is active); `tray.rs` menubar toggle/quit via tray-icon + tao (tray must be created after the tao loop starts, and menu items are not Send — menu events are proxied back to the main thread). Pressing the hotkey again on the same word converts back.
  - Windows (**implemented, not yet run on real Windows**, `crates/haneng-windows`, binary `hanengw`): `WH_KEYBOARD_LL`/`WH_MOUSE_LL` hooks feed the same core `WordBuffer`; Ctrl+Shift+Space converts; `inject.rs` uses `SendInput` `KEYEVENTF_UNICODE` with `dwExtraInfo` marker (injection waits until Ctrl/Shift are physically released — otherwise apps see Ctrl+Backspace); `ime.rs` reads/sets `IME_CMODE_NATIVE` via `WM_IME_CONTROL` to the default IME window; `secure.rs` checks `ES_PASSWORD` on the focused Edit control (UIA coverage pending). Verify with `cargo check --target x86_64-pc-windows-msvc` — the crate compiles as a stub elsewhere.
  - Linux X11 (**implemented, not yet run on real Linux**, `crates/haneng-linux`, binary `hanengl`): XRecord observation (raw 32-byte xEvent parsing) + XTest injection via x11rb (pure Rust). X11-specific constraints: no IME-mode query API exists, so the daemon *tracks* mode by observing 한/영 toggle keycodes (default 130/108, `linux_toggle_keycodes` config override) and assumes English at startup; no event marker exists, so an `INJECTING` flag suppresses self-observation; unicode typing remaps a spare keycode per char (xdotool technique); **no password-field detection yet** (needs AT-SPI — known gap). Wayland is out of scope for this adapter (Fcitx5/IBus plugin track).
  - **Adapter-wide rule**: on Windows/Linux, modifier keys arrive as their own KeyDown/KeyPress events — `classify` must return `None` (ignore) for them, never `Clear`, and the modifier-shortcut clear must run only for non-modifier keys; otherwise Shift typing and the Ctrl+Shift+Space hotkey break. macOS is exempt (modifiers are FlagsChanged, not KeyDown).
- **haneng-ffi** (`cdylib`/`staticlib` named `libhaneng`, header at `crates/haneng-ffi/include/haneng.h` — keep the two in sync): C ABI over conversion + `Detector` for the Fcitx5/IBus (Wayland) plugin track. Pointer-taking exports are `unsafe extern "C"` with `# Safety` docs (clippy `not_unsafe_ptr_arg_deref` denies otherwise); returned strings are freed with `haneng_free`.
- **Per-app disable** (`disabled_apps = terminal, slack` in config.txt; case-insensitive substring): checked at conversion-trigger time, not per keystroke. App identity per OS: macOS = executable basename from the key event's `EVENT_TARGET_UNIX_PROCESS_ID` via `proc_pidpath`; Windows = foreground exe name via `QueryFullProcessImageNameW`; Linux = active window `WM_CLASS` via `_NET_ACTIVE_WINDOW`.
- **haneng-settings** (eframe/egui, binary `haneng-settings`): standalone settings window editing `config.txt` (auto/sensitivity/layout/disabled_apps, preserving unknown extras via `Config::serialize`) and the exception dictionary. Daemons read config only at startup — the UI says so; runtime toggles stay in the tray. Launched from the tray "설정..." item (spawns the binary next to the daemon exe). egui has no CJK glyphs by default — `install_korean_font` loads an OS system font at runtime. Note eframe 0.35 renamed `App::update` to `App::ui(&mut self, ui, frame)`.
  - Linux X11: `XRecord` + `XTest`
  - Linux Wayland: no global hooking possible — implemented as a separate Fcitx5/IBus plugin track calling haneng-core over C FFI
- **UI**: Tauri 2 tray icon + settings window, talking to a headless daemon over local IPC. The daemon must work without the UI.

## Non-negotiable design constraints

These come from PLAN.md and should not be violated by any change:

- **Never replace the user's IME.** The app observes committed input and corrects via backspace + re-injection; it must never touch text still in IME composition (preedit) state. Only convert at word boundaries (space, enter, punctuation).
- **Never process secure input.** Stop entirely when a password field has focus (macOS `IsSecureEventInputEnabled()`, Windows `ES_PASSWORD`/UIA `IsPassword`).
- **Local-only, no keystroke persistence.** Keep at most the last 1–2 words in memory, discard at word boundaries, never write keystrokes to disk, no network code (opt-in update check is the only exception).
- **Injected events must be tagged** with a self-marker so the hook can distinguish its own injections from user input (race prevention).
- **False positives kill the product.** Auto-conversion ships behind a conservative threshold; accuracy targets are false positive < 0.5%, recall > 90%, enforced by a corpus-based regression harness in CI. When a user undoes an auto-conversion, that word is learned into the exception dictionary.

## Daemon behavior shared by both adapters

Both daemons implement the same event pipeline (mirrored code in each `main.rs` — keep them in sync when changing one): every user keydown bumps a `GENERATION` counter; on a `Boundary` key with auto mode on, a task sleeps ~60ms, re-checks the generation (abort if the user kept typing), checks secure input, asks `AutoCorrector`, injects, switches IME mode, and arms `PENDING_UNDO`. A user Backspace immediately after an auto-correction consumes `PENDING_UNDO`: the user's backspace already deleted the boundary char, so the undo task deletes `replacement-1` chars, re-injects the original, restores the mode, and persists the word via `config::append_exception`. Any other key/click/hotkey clears `PENDING_UNDO`. Auto/enabled toggles: tray menu on macOS, `config.txt` (`auto = off`, `sensitivity = conservative|balanced|aggressive`) on both.

## Roadmap position

Phased delivery (details in PLAN.md): Phase 0 core engine + accuracy harness (done) → Phase 1 manual conversion MVP (done) → Phase 2 automatic detection + undo learning + dictionary/bigram scoring (done) → Phase 3 done at code level — Linux X11 adapter, C FFI (haneng-ffi), per-app exceptions, Windows tray, sebeolsik 390/final layouts, CI (`.github/workflows/ci.yml`: fmt + clippy -D warnings + tests on all 3 OSes); remaining — Fcitx5/IBus plugin itself (C++ against libhaneng, needs a Linux box) and AT-SPI password detection on Linux. Settings UI: done (haneng-settings). Real-device verification is pending on all three OSes; code signing/packaging (Phase 4) needs certificates.
