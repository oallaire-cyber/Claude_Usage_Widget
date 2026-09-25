# Progress

## Phase checklist

- [x] Phase 0 — Bootstrap (CI green on first push, run 36167329000)
- [x] Phase 1 — Bridge, sources, install scripts
- [x] Phase 2 — Core logic
- [ ] Phase 3 — Tray icon
- [ ] Phase 4 — Popup card
- [ ] Phase 5 — Alerts, settings, pinned mode
- [ ] Phase 6 — Release

## Definition of Done

- [x] `cargo test`, clippy, fmt and frontend build pass locally and in CI on `main` — true at end of run 1 (CI run 36169256569); re-check every run
- [x] Bridge: all parser fixtures pass; exits 0 on malformed input; median runtime recorded and < 50 ms (or deviation explained in DECISIONS.md) — 29.5 ms, D-10
- [x] Install/uninstall scripts pass tests against fake settings files; uninstall restores previous `statusLine` exactly — 50 assertions, byte-identical restore
- [x] Core: pace, staleness and notification-once logic unit-tested — `merge.rs`, `pace.rs`, `notify.rs`
- [ ] Tray icon colour thresholds verified by pixel test
- [ ] Preview screenshots exist in `docs/screenshots/` for every state in §5, light and dark
- [ ] Installer built by CI from tag `v0.1.0` and attached to a draft release
- [ ] README, MANUAL_CHECKS, DECISIONS, BACKLOG complete; PROGRESS shows every phase done

## Prerequisites (checked 2026-09-25)

git 2.50.1 · gh authenticated as `oallaire-cyber` (scopes repo, workflow) · node 22.18.0 / npm 10.9.3 ·
rustc/cargo 1.98.1 (stable-x86_64-pc-windows-msvc) · VS 2019 Build Tools with VC x64 toolset ·
WebView2 153.0.4234.48 · Windows PowerShell 5.1.26100 · Playwright 1.63.0 available via npx.

## Next step

Run 2 starts **Phase 3 — Tray icon**: runtime-rendered ring icon (16/20/24/32 px) coloured with
`cuw_core::pace` (blue / amber ≥ 1.1× / red ≥ 1.75× or ≥ 90 %, grey for stale / no data), pixel test,
tooltip, right-click menu. The app will need a file watcher on `state.json`, `~/.claude.json` (source B,
parse via `sources::from_claude_code_cache` only) and Desktop's `plan-usage-history.json`, feeding
`merge::merge`. The tray colour function is not written yet — it belongs to Phase 3.

## Known issues

- No Fable / model-scoped weekly window exists in any local source today (docs/FINDINGS.md) — the
  extra ring can only appear if Claude Code starts sending it.
- Source B (`cachedUsageUtilization`) is absent in Claude Code 2.1.282; its parser works on an assumed
  shape (D-13).
- The bridge has not yet run inside a real Claude Code session: that is manual check 1.

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
