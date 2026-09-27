# Progress

## Phase checklist

- [x] Phase 0 — Bootstrap (CI green on first push, run 36167329000)
- [x] Phase 1 — Bridge, sources, install scripts
- [x] Phase 2 — Core logic
- [x] Phase 3 — Tray icon
- [x] Phase 4 — Popup card
- [x] Phase 5 — Alerts, settings, pinned mode
- [x] Phase 6 — Release (tag `v0.1.0`, release run 36229934350, draft release with installer)

## Definition of Done

- [x] `cargo test`, clippy, fmt and frontend build pass locally and in CI on `main` — true at end of run 1 (CI run 36169256569) and run 2 (CI run 36185017692) and run 3 (CI run 36229761335)
- [x] Bridge: all parser fixtures pass; exits 0 on malformed input; median runtime recorded and < 50 ms (or deviation explained in DECISIONS.md) — 29.5 ms, D-10
- [x] Install/uninstall scripts pass tests against fake settings files; uninstall restores previous `statusLine` exactly — 50 assertions, byte-identical restore
- [x] Core: pace, staleness and notification-once logic unit-tested — `merge.rs`, `pace.rs`, `notify.rs`
- [x] Tray icon colour thresholds verified by pixel test — `crates/core/src/tray.rs` (every size and tone, threshold boundaries)
- [x] Preview screenshots exist in `docs/screenshots/` for every state in §5, light and dark — plus extra-window, Desktop-only, pinned, settings; English and French
- [x] Installer built by CI from tag `v0.1.0` and attached to a draft release — `Claude.Usage.Widget_0.1.0_x64-setup.exe` (2.2 MB), release run 36229934350
- [x] README, MANUAL_CHECKS, DECISIONS, BACKLOG complete; PROGRESS shows every phase done

## Prerequisites (checked 2026-09-25)

git 2.50.1 · gh authenticated as `oallaire-cyber` (scopes repo, workflow) · node 22.18.0 / npm 10.9.3 ·
rustc/cargo 1.98.1 (stable-x86_64-pc-windows-msvc) · VS 2019 Build Tools with VC x64 toolset ·
WebView2 153.0.4234.48 · Windows PowerShell 5.1.26100 · Playwright 1.63.0 available via npx.

## Next step

None for the builder: every phase is done and every Definition-of-Done box is ticked (PROMPT.md §7 —
stop). 2026-09-27: Olivier installed and ran the first draft installer on his laptop without problems.
Before publication, an audit led to the fixes in D-26, the history was rewritten to GitHub's private
commit address, and `v0.1.0` was re-tagged on the final commit. What remains is Olivier's: a quick
reinstall from the new draft release, then make the repository public and publish the release.

## Known issues

- No Fable / model-scoped weekly window exists in any local source today (docs/FINDINGS.md) — the
  extra ring can only appear if Claude Code starts sending it.
- Source B (`cachedUsageUtilization`) is absent in Claude Code 2.1.282; its parser works on an assumed
  shape (D-13).
- The bridge has not yet run inside a real Claude Code session: that is manual check 1.
- Tray tooltip, right-click menu, blur-to-close, dragging the pinned widget, its transparency, toast
  appearance and the autostart entry could not be seen from this run (the screen was locked; captures
  used `PrintWindow` on the app's own windows). All are in MANUAL_CHECKS.md §3.
- Toasts from a `target\debug` build carry Windows PowerShell's name (D-23); the installer sets the
  app's own identity on its Start-menu shortcut (D-25) — confirming it needs an install (MANUAL_CHECKS §0).
- The installer was not run by the builder (it would register an app on Olivier's profile); Olivier ran
  it himself on 2026-09-27 (MANUAL_CHECKS §0).

## Run reports

### Run 1 — 2026-09-25 — Phases 0–2

**Completed**
- Phase 0: prerequisites all present (VS 2019 Build Tools accepted, D-05); private repo
  `oallaire-cyber/Claude_Usage_Widget` created and pushed; Cargo workspace (`crates/core`,
  `crates/bridge`, `app/src-tauri`); Tauri 2 app scaffold builds; CI (fmt, clippy, tests, frontend
  build, PowerShell install tests) and tag-triggered release workflow; five tracking files.
- Phase 1: `cuw-bridge.exe` (451 KB) — parses the status-line payload, keeps the max per window cycle
  across concurrent sessions, writes `state.json` atomically under a lock, appends `history.jsonl`
  (1/min/window, 14-day pruning), chains a previous status-line command (2 s timeout, process-tree kill),
  always prints a line and exits 0. Median runtime 29.5 ms. Source discovery for B and C with masking
  scripts → `docs/FINDINGS.md` + redacted fixtures. Install/uninstall scripts with 50 passing checks.
- Phase 2: `cuw-core` merge (A/B/C, statuses ok / stale / window_reset / no_data), pace (ratio,
  projection by history fit or average, guards), notifications (once per cycle, optional reset notice).
  62 unit tests + 5 fixture tests.

**Definition of Done so far:** 4 of 8 ticked (build/CI, bridge, install scripts, core logic).

**Deviations from the prompt, and why**
- A third crate `cuw-core` holds all pure logic (D-01).
- The install script copies the bridge to `%LOCALAPPDATA%\ClaudeUsageWidget\bin` so the command has no
  spaces and works in both Git Bash and PowerShell (D-06).
- Scaffold `opener` plugin and `panic = "abort"` removed (D-03, D-09).
- Source B is absent, so there is no real fixture for it and no Fable ring (FINDINGS.md).

**Open issues:** see Known issues above. Nothing blocked; no anti-loop entries.

**Manual checks for Olivier now:** `MANUAL_CHECKS.md` §1 (install the bridge — optional before the app
exists, but it starts collecting history now) and §2 (compare with claude.ai → Settings → Usage).

### Run 2 — 2026-09-25 — Phases 3–5

**Completed**
- Phase 3 (tray icon): the ring is drawn at runtime at 16/20/24/32 px for the display scaling, filled to
  the session %, coloured blue / amber / red by pace with the guards, grey when stale, reset or empty
  (D-17). Pixel tests cover every size and tone and each threshold boundary. Tooltip in the prompt's
  exact shape plus "as of HH:MM", English and French. Right-click menu: Pin widget, Settings, Start with
  Windows, Quit.
- Phase 4 (popup card): opens next to the tray icon on whichever side the taskbar is, inside that
  monitor's work area (geometry unit-tested, including a monitor at negative coordinates); closes on
  focus loss or Esc. Concentric rings (session, weekly, extra windows inside) with even-spend markers,
  per-window reset countdown and one-line pace message, "Data as of …" footer and source. Light/dark
  follows Windows, accent colour on controls, French/English from the Windows language with an override.
  Solid Windows 11 surfaces rather than Mica (D-20).
- Phase 5: toasts wired to the core notification logic (defaults 80/95 % session, 75/90 % weekly,
  persisted so a restart does not repeat them); settings window (language, notifications on/off,
  thresholds, reset notice, start with Windows), applied immediately; pinned widget (always on top,
  semi-transparent, draggable, position remembered); single instance.
- Preview mode and 44 screenshots (8 card states × light/dark × English/French, plus pinned widget and
  settings), from fixtures computed by the real core logic, rendered in Edge (D-22). Two review-and-fix
  rounds used; leftover nits in BACKLOG.
- Real app checked with a fake data folder: renders, anchors, updates within 2 s of a file change,
  hands over to the running copy on second launch, pinned position survives a restart, one alert
  recorded for 80 % at 93 % usage.

**Definition of Done so far:** 6 of 8 ticked (added: tray pixel test, screenshots). Remaining: installer
and release (Phase 6), README and final paperwork.

**Deviations from the prompt, and why**
- Found on real data: Claude Desktop's rarely-updated "extra usage" value made the whole card look stale.
  The overall status now follows the session and weekly windows (D-21, amends run 1's D-14).
- `cuw-app` is built as a plain Rust library, dropping the mobile-only DLL output that made Windows
  linking fail intermittently (D-24).
- Screenshots use the installed Microsoft Edge rather than a downloaded browser (D-22).

**Open issues:** see Known issues — mainly things that need eyes and a mouse on the unlocked desktop.
Nothing blocked; no anti-loop entries.

**Manual checks for Olivier now:** `MANUAL_CHECKS.md` §3 (the tray app — nine short checks: icon,
tooltip, card placement and closing, menu, pinned widget drag, settings, start with Windows, toasts,
single instance). §1 and §2 still apply if not done yet.

### Run 3 — 2026-09-26 — Phase 6

**Completed**
- NSIS installer, per-user (no admin prompt), bundling the app, `cuw-bridge.exe` and the
  install/uninstall scripts; it does not run the install script. The bundled files come from a
  release-only config so ordinary builds do not need the release bridge (D-25). The Start-menu shortcut
  carries the app's notification identity. The bundled scripts were run from the installed layout
  (folder with spaces) against fake settings: install, then uninstall restores the file byte-for-byte.
- README: screenshots, install and uninstall steps, SmartScreen note, data flow, privacy statement,
  known limitations, build steps, credits (licences checked: two MIT, one without a licence — ideas
  only, no code reused from any).
- Tag `v0.1.0`; the release workflow built the installer on GitHub and attached it to a **draft**
  release (`Claude.Usage.Widget_0.1.0_x64-setup.exe`, 2.2 MB). CI green on `main` (run 36229761335);
  locally fmt, clippy, 110 Rust tests and 50 install-script checks pass.

**Definition of Done:** 8 of 8 ticked. The project is complete per PROMPT.md §7.

**Deviations from the prompt, and why**
- The Windows uninstaller does not unhook the bridge from Claude Code (it would have to edit
  `~/.claude/settings.json`); the README says to run `uninstall-bridge.ps1` first (D-25, BACKLOG).
- The installer was not run on this machine — only inspected — to avoid registering an app on
  Olivier's profile unasked (MANUAL_CHECKS §0).

**Open issues:** see Known issues. Nothing blocked; no anti-loop entries.

**Manual checks for Olivier now:** `MANUAL_CHECKS.md` §0 (run the installer, check the files, the
toast name and the autostart path, then publish the draft release), §1–§2 (hook the bridge using the
installed script, compare with claude.ai), §3 (the tray app).
