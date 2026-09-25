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

- [ ] `cargo test`, clippy, fmt and frontend build pass locally and in CI on `main`
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

Phase 2 (core logic) in progress.

## Known issues

None yet.
