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

## D-06 · 2026-09-25 · Where the bridge and its files live
- Data: `%APPDATA%\ClaudeUsageWidget\` → `state.json`, `history.jsonl`, `state.lock`, `install.json`.
  `CUW_DATA_DIR` overrides it (tests only).
- Binary: the install script **copies** `cuw-bridge.exe` to `%LOCALAPPDATA%\ClaudeUsageWidget\bin\`.
  The app's install folder has spaces ("Claude Usage Widget"), and a quoted path is a plain string, not a
  command, when Claude Code falls back to PowerShell. A path without spaces works in both Git Bash and
  PowerShell. The command is written with forward slashes (docs requirement for Git Bash). If the path
  still has a space, the script uses the `~/…` form or refuses. Consequence: after an app update, re-run
  the install script to refresh the copy (README will say so).
- The previous `statusLine` is recorded in `install.json`, not in `settings.json` (an unknown key in
  Claude Code's settings could be flagged by its settings validation).

## D-07 · 2026-09-25 · Multi-session merge details (bridge)
- Same cycle = `resets_at` within **60 s**: keep the max. Later `resets_at` = new cycle: replace. Earlier
  `resets_at` = a lagging session: ignore. A reading without `resets_at` replaces the stored value.
- A window missing from a payload keeps its stored value (see D-04).
- Read-modify-write of `state.json` is serialised by a lock file (`create_new`), best effort: after
  ~300 ms the bridge proceeds without it rather than delay Claude Code; locks older than 5 s are broken.
  Tested with 8 concurrent bridge processes (result = max).
- History: ≤ 1 sample/min/window, only for windows present in the payload; pruning (> 14 days) rewrites
  the file atomically at most once a day, so the hot path is an append.

## D-08 · 2026-09-25 · Chaining behaviour
- The chained command runs through Git Bash (found like Claude Code does: `CLAUDE_CODE_GIT_BASH_PATH`,
  next to `git.exe` on PATH, default install folders — never WSL's `bash.exe`), else PowerShell. Same
  stdin, 2 s timeout, stderr discarded, no console window.
- State is written **before** the chained command runs (Claude Code cancels an in-flight status line
  when a new update arrives).
- Output is printed unchanged when it has any non-whitespace content; if the chained command prints
  nothing, fails to start or times out, the bridge prints its default line ("always print something").
- Found while testing: on Windows, the chained shell inherited the bridge's own stdout, so a slow
  grandchild kept Claude Code's pipe open after the timeout. Fix: the bridge marks its std handles
  non-inheritable and runs the chained command inside a **job object** that kills the whole process tree
  on timeout or bridge exit (small hand-written Win32 FFI, no extra crate).
- `CUW_BRIDGE_CHAINED` is set on the child so a bridge chained to itself does not recurse.

## D-09 · 2026-09-25 · Bridge never fails: `panic = "abort"` removed
The release profile no longer uses `panic = "abort"` (a scaffold default). The bridge wraps its work in
`catch_unwind`, silences the panic hook, prints a fallback line and exits 0 — impossible under abort.

## D-10 · 2026-09-25 · Bridge runtime (measured)
`node tools/bench-bridge.mjs target/release/cuw-bridge.exe 60`, release build, this machine, process
spawn included (baseline: spawning `whoami.exe` = 23 ms median):

| Case | Median | p90 |
|---|---|---|
| payload with `rate_limits` (state write + fsync + history) | **29.5 ms** | 56.6 ms |
| payload without `rate_limits` | 15.6 ms | 23.8 ms |
| empty stdin | 15.2 ms | 23.8 ms |
| chaining `echo hi` through Git Bash | 100.8 ms | 130.6 ms |

Target (< 50 ms median) met. Chaining cost is Git Bash start-up, which Claude Code would pay anyway to
run the previous command itself. Binary size 451 KB.

## D-11 · 2026-09-25 · Install scripts: own JSON writer, exact restore
Windows PowerShell 5.1's `ConvertTo-Json` re-indents oddly, truncates at `-Depth` and escapes `< > ' &`.
The scripts parse with `ConvertFrom-Json` (keeps property order) and write with a small serializer
(`tools/CuwJson.psm1`) producing 2-space JSON like Claude Code's own, UTF-8 without BOM, via temp file +
`File.Replace`. Result, tested: for a file in Claude Code's layout, install → uninstall is
**byte-identical**; for other layouts it is JSON-identical. `statusLine` keeps its position; other
`statusLine` options (`padding`, `refreshInterval`) are kept when the bridge replaces the command.
Uninstall leaves settings alone if `statusLine` was changed after install (unless `-Force`), and deletes
the settings file only if install created it and it is empty again. Invalid JSON aborts with no change.

## D-12 · 2026-09-25 · Source discovery scripts mask identifiers
The extraction scripts mask strings > 40 chars, token-like prefixes and UUIDs. The first run printed the
Claude Desktop organisation UUID once (not a credential) before UUID masking was added; it appears
nowhere in the repository (fixtures use a zero UUID).

## D-13 · 2026-09-25 · Source B parsed on an assumed shape
`cachedUsageUtilization` does not exist in Claude Code 2.1.282 (FINDINGS.md), so its shape is a guess.
The parser deserialises only that key and accepts: window objects keyed by name (snake or camelCase,
normalised to snake_case) with `utilization` / `used_percentage` / `percent` and `resets_at` / `resetsAt`,
optionally wrapped in `data`; a block timestamp (`timestamp`, `updatedAt`, …) dates the values, else the
file's modification time (a timestamp in the future of the mtime is ignored). Anything else yields no
observations. If a future version writes the block with another shape, only this function changes.

## D-14 · 2026-09-25 · Merge and status rules
- Status line (A) is **current** when observed ≤ 30 min ago and its `resets_at` has not passed; a current
  A value always wins. Otherwise the freshest value wins; ties prefer A, then B, then C.
- If any source says a window reset at time R (R ≤ now), a value **without** a reset time observed before
  R belongs to the previous cycle and is discarded (prevents showing Desktop's pre-reset 90 % after reset).
- Per-window status: `window_reset` (now ≥ `resets_at`, display 0 %) takes precedence over `stale`
  (> 30 min without update), else `ok`. Overall: `no_data` if no window; `stale` if any window is stale;
  `window_reset` if every window has reset; else `ok`.
- "Data as of" = freshest `as_of` across windows; the "source" shown = the session window's source.
- Claude Desktop keys: `fh` → `five_hour`, `sd` → `seven_day`, `xu` → `extra_usage`; unknown keys are
  kept as `desktop_<key>` extra windows. No reset time is borrowed from another source for Desktop
  values (countdown/pace shown as unavailable, as the prompt says) — see BACKLOG.

## D-15 · 2026-09-25 · Pace rules
- Computed only for windows of known length (`five_hour*` = 5 h, `seven_day*` = 7 d) with a reset time
  in the future. `spend_limit` / `extra_usage` have none.
- Projection rate: least-squares slope over this cycle's history samples within the last 1/5 of the
  window (1 h for the session, ~34 h for the week), needing ≥ 3 samples spanning ≥ 1/4 of that period;
  otherwise the average rate since the window started.
- Status: `will_exhaust` if already at 100 % or the projection reaches 100 % before the reset;
  `ahead` if pace ratio ≥ 1.1; else `on_track`. The §4b guards (used < 20 % or elapsed < 10 %) force
  `on_track` and set `guarded` so the tray colours by plain thresholds.
- Consequence worth knowing: with the average-rate fallback, a ratio above 1.0 always projects
  exhaustion, so `ahead` only appears when recent history shows you have slowed down.

## D-16 · 2026-09-25 · Notification rules
- A threshold fires at most once per window cycle (cycle = `resets_at`, 60 s tolerance). If several
  thresholds are crossed at once, **one** notification names the highest.
- Already above a threshold when first seen → notifies (you should know), unless the app has persisted
  state for that cycle (the app stores `NotifyState`, Phase 5).
- "Limit reset" notification: on by default, but only for a window that produced a threshold warning in
  the cycle that just ended; sent once, either when `resets_at` passes or when a new cycle is seen.
- For Desktop-only data (no reset times) a new cycle is a drop of more than 5 points.
- With notifications disabled, state still advances silently, so re-enabling does not replay old warnings.
- Default thresholds: session 80/95 %, weekly 75/90 %; other windows get none unless configured.
