# Claude Usage Widget — working notes for Claude

Windows tray app showing Claude subscription usage (5-hour session + 7-day weekly windows).
The mission and phase plan live in `PROMPT.md`; state of play in `PROGRESS.md`.

## Hard rules (summary — PROMPT.md §2 is authoritative)

- Never read `~/.claude/.credentials.json`. Never call any Anthropic endpoint. Never read browser cookies.
- `~/.claude.json` and `%APPDATA%\Claude\` hold MCP configs that may contain API keys: never load them
  whole. Inspect only via `tools/extract-*.mjs`, which print whitelisted usage fields only.
- The shipped app and bridge make **zero network requests**.
- Never modify anything under `%USERPROFILE%\.claude\`. Test install scripts against fake files only.
- Do not modify `.claude/settings.json` in this repo.
- Scope freeze: new ideas go to `BACKLOG.md`.

## Layout

```
Cargo.toml              workspace: crates/core, crates/bridge, app/src-tauri
crates/core             cuw-core: pure logic (parsing, state model, merge, pace, notifications). No I/O.
crates/bridge           cuw-bridge.exe: Claude Code statusLine command → state.json + history.jsonl
app/                    Tauri 2 app; frontend = Vite + TypeScript, no UI framework
app/src-tauri           cuw-app crate (tray, file watching, windows)
tools/                  install/uninstall PowerShell 5.1 scripts, source-discovery scripts
tools/tests             PowerShell tests for the install scripts (run against temp fake settings)
docs/                   FINDINGS.md (source discovery), screenshots/
```

## Commands

```
cd app && npm ci && npm run build          # frontend (must exist before building cuw-app)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
powershell -NoProfile -ExecutionPolicy Bypass -File tools/tests/Test-InstallBridge.ps1
cd app && npm run tauri dev                # run the app
cd app && npx tauri build --debug --no-bundle   # standalone debug exe (frontend embedded) → target/debug/claude-usage-widget.exe
cargo build --release -p cuw-bridge        # bridge binary
node tools/gen-preview-views.mjs           # regenerate app/src/preview/views.json from the core (after logic changes)
cd app && npm run build && node scripts/screenshots.mjs [--lang fr]   # preview screenshots → docs/screenshots (uses Edge)
```

Run the app against fake data: set `CUW_DATA_DIR` to a temp folder containing a `state.json`. It still
reads the real `~/.claude.json` and Claude Desktop history (read-only).

## App architecture (Phases 3–5)

- `crates/core`: `tray.rs` (tone rules + ring rasteriser, pixel tests), `view.rs` (merged windows + pace
  + tone, sent to the frontend), `text.rs` (tooltip and toast wording, EN/FR, local time injected).
- `app/src-tauri/src`: `engine.rs` (the one thread that loads, merges, notifies, updates the tray —
  never hold a lock across a Tauri call, see D-18), `ui.rs` (tray, menu, popup/pinned/settings windows),
  `data.rs` (source loading by mtime), `placement.rs` (popup geometry), `settings.rs`, `sys.rs` (Win32
  FFI: accent, UI language, rounded corners), `paths.rs`.
- `app/src`: one bundle for the three windows, chosen by window label; `?view=…` preview mode in a
  plain browser. Text for the card lives in `i18n.ts`.

## Conventions

- Conventional Commits, small. Push at the end of every phase.
- Check `git diff --cached --stat` before committing (no secrets, no unexpected deletions).
- Times are Unix seconds (`i64`) throughout the core; the core never reads the clock — `now` is a parameter.
- PowerShell is 5.1: no `??`, no ternary, no `-AsHashtable`; write UTF-8 **without** BOM explicitly.
- Record decisions in `DECISIONS.md`, things only Olivier can verify in `MANUAL_CHECKS.md`.
