# Decisions

Numbered, dated, one decision each: what, why, and consequence.

## D-01 · 2026-09-25 · Third crate `cuw-core` alongside `app` and `bridge`
Parsing of the status-line payload, the state model, merge, pace and notification logic live in a
pure `crates/core` library with no I/O. Both the bridge and the app depend on it, so the payload is
parsed by exactly one implementation, and Phase 2's "no I/O in tests" is enforced structurally.
Its only dependencies are `serde` + `serde_json`, which keeps the bridge small.

## D-02 · 2026-09-25 · Tauri 2 pinned, not 3
`cargo search tauri` now returns `3.0.0-alpha`; the prompt says Tauri 2, and an alpha is no base for a
tray app. All Tauri crates and npm packages are pinned to major version 2.

## D-03 · 2026-09-25 · Scaffold's `opener` plugin removed
`create-tauri-app` adds `tauri-plugin-opener` (opens URLs/files). The app must make no network
requests and has no need to open URLs, so it is removed; capabilities are `core:default` only.
`withGlobalTauri` is off and a strict CSP (`default-src 'self'`) is set.

## D-04 · 2026-09-25 · Status-line documentation vs. the prompt summary
Read https://code.claude.com/docs/en/statusline on 2026-09-25. Differences from PROMPT.md §3.A:
- `resets_at` is documented as **Unix epoch seconds** only. ISO 8601 is still accepted (public issue
  reports), as the prompt asks.
- A third documented window exists: **`spend_limit`** (Claude apps gateway only; `used_percentage`
  may exceed 100). It is treated as an "extra window" per the prompt's rule; percentages are not clamped
  at parse time.
- Claude Code **drops a window from the payload once its `resets_at` passes**, and re-runs the status
  line at that moment. So an absent window in a fresh payload is not proof of 0 % — the bridge keeps
  the last known value for windows missing from a payload and lets the app mark them `window_reset`.
- On Windows, Claude Code runs the `command` **through Git Bash when installed, else PowerShell**.
  Paths in the command must use forward slashes. The bridge runs a chained previous command the same
  way (Git Bash if found, else `powershell -NoProfile -Command`).
- Updates are debounced at 300 ms; an in-flight run is **cancelled** if a new update arrives. The
  bridge therefore writes state first and runs the chained command second.
- Optional `refreshInterval` (seconds) re-runs the command on a timer; the install script does not set
  it (it would add process launches for no new data — values only change after API responses).

## D-05 · 2026-09-25 · MSVC 2019 Build Tools accepted
Prereq check found Visual Studio 2019 Build Tools with the x64 C++ toolset (not 2022). Rust's
`x86_64-pc-windows-msvc` target builds the full Tauri workspace with it, so no upgrade is needed.
