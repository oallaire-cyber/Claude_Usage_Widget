# Mission: Claude Usage Widget for Windows (v3)

Build a small, polished Windows tray application that shows my Claude subscription usage at a glance:
current 5-hour session utilization and weekly (7-day) utilization, each with its reset countdown and a pace indicator.

You work autonomously. Do not ask me questions. Make reasonable decisions, record them in `DECISIONS.md`, keep going.
This prompt is executed over several runs. **The launch message tells you which phases this run covers. Do only those phases.**
At the start of every run, read `PROGRESS.md` (if it exists) and continue from there.

---

## 1. Context

- Owner: Olivier, GitHub account `oallaire-cyber`.
- Machine: Windows 11, user `olive`, default shell Windows PowerShell 5.1 (not PowerShell 7). Your Bash tool may run Git Bash; prefer Rust/Node tooling over shell-specific scripts, and write any PowerShell for 5.1.
- Working directory: `C:\Users\olive\Documents\Claude_Usage_Widget` (contains this file and `.claude/settings.json`; do not modify that settings file).
- Plan: Claude Max (5x). Olivier works mostly in Claude Code CLI.
- Other repos live under `C:\Users\olive\Documents`. Never touch them.

## 2. Hard rules (non-negotiable)

1. **Terms of use.** Anthropic's terms restrict subscription OAuth credentials to Claude Code and Anthropic's own apps. Therefore the app, the bridge and every script you write:
   - never read `~/.claude/.credentials.json` (you must not read it either),
   - never call api.anthropic.com, claude.ai, or any Anthropic endpoint,
   - never read browser cookies or sessions.
   Data comes only from local files that Claude Code / Claude Desktop write themselves (§3). Do not clone, port or read the code of projects that call Anthropic endpoints with the OAuth token (e.g. apexlocal-jz/claude-usage-tray, wus-technik/win_systray-claude-usage, the vscode-claude-status extension).
   **Sensitive neighbours:** `~/.claude.json` and `%APPDATA%\Claude\` also hold MCP server configs that can contain API keys. Never load those files whole into this conversation. Inspect them only through a small script that extracts and prints the whitelisted usage fields (§3) and nothing else. The app must parse only those fields too.
2. **No network in the shipped app.** The app and bridge make zero network requests. (CI and your dev tooling may use the network.)
3. **Do not modify anything under `%USERPROFILE%\.claude\`** during the run, including `settings.json`. Hooking the bridge into Claude Code is done by an install script that Olivier runs himself. Test that script only against temporary fake settings files.
4. **No secrets in git.** `.gitignore` covers `.env*`, local state/history files, build output. Check staged diffs before each commit.
5. **Scope freeze.** Implement only what this prompt lists. Any other idea goes to `BACKLOG.md` with one line of rationale — not into the code.
6. **Anti-loop.** If the same problem persists after 3 genuinely different fix attempts, record it in `PROGRESS.md` (what you tried, what you observed), then continue with other work if possible, otherwise STOP (§8).
7. **Polish budget.** For each UI state, at most 2 screenshot-review-and-fix rounds. Then move on and list remaining nits in `BACKLOG.md`.

## 3. Data sources (all local, all read-only)

**A. Status line (primary, documented).** Claude Code runs a user-configured command on each status line update and pipes it JSON on stdin. For Pro/Max subscribers this includes:

```json
"rate_limits": {
  "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
  "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
}
```

- Read the official doc first: https://code.claude.com/docs/en/statusline — follow it where it differs from this summary; record differences in `DECISIONS.md`.
- Known quirks (public issue reports): `resets_at` seen as Unix seconds and as ISO 8601; `rate_limits` absent until the first API response of a session and absent entirely in some versions; the model-specific weekly window (Fable) is NOT included (anthropics/claude-code#91920). Treat any extra object under `rate_limits` with `used_percentage` as an additional window.
- Several Claude Code sessions may run at once: for each window, keep the highest `used_percentage` among payloads sharing the same `resets_at`, so a quieter session never drags the value down.

**B. Claude Code cache (secondary, undocumented).** Claude Code writes a `cachedUsageUtilization` block into `%USERPROFILE%\.claude.json`. It may carry more windows (possibly the Fable weekly limit). In Phase 1, discover its structure with an extraction script that prints ONLY that block (see §2.1). If it holds a model-scoped weekly window, use it — that restores the Fable ring. Undocumented: parse defensively, never crash on shape changes.

**C. Claude Desktop history (tertiary, undocumented).** `plan-usage-history.json` in `%APPDATA%\Claude\` or, on newer versions, `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\` (newer file wins). Percentages only, no reset times. Use it only when A and B are absent or stale, and show that countdowns/pace are unavailable in that mode. Same extraction-script rule for inspection.

**Merge rule.** Per window, use the freshest valid value; A wins whenever current. Show the source and "data as of HH:MM" in the popup. A window's value becomes meaningless once its `resets_at` passes.

## 4. Architecture

- **`cuw-bridge.exe`** (Rust, tiny, minimal deps) — configured as Claude Code's `statusLine` command.
  - Reads stdin JSON. If `rate_limits` is present: writes `%APPDATA%\ClaudeUsageWidget\state.json` atomically (temp file + rename) with all windows + `updated_at`; appends a sample to `history.jsonl` (max 1 sample/minute per window, prune > 14 days).
  - Output (what Claude Code displays): if the install script recorded a previous `statusLine` command, run it with the same stdin (2 s timeout) and print its output unchanged; otherwise print a compact default line (`5h 14% · 7d 8%`).
  - Must never break Claude Code: always exit 0, always print something, tolerate malformed/missing input. Target median runtime < 50 ms (measure and record).
- **Tray app** (Tauri 2, Rust + TypeScript/Vite, no UI framework) — watches `state.json` and sources B and C (file notifications, no network), merges them (§3), computes staleness and pace, renders tray icon, popup, notifications.
- **`tools/install-bridge.ps1` / `tools/uninstall-bridge.ps1`** (PowerShell 5.1) — parameter `-SettingsPath` defaulting to `%USERPROFILE%\.claude\settings.json`. Install: timestamped backup, parse JSON, preserve every other key, record any existing `statusLine` for chaining, set `statusLine` to the bridge, write UTF-8 without BOM, idempotent. Uninstall restores the previous `statusLine` exactly.

## 4b. Reference projects (compliant — study for ideas; check each licence before reusing any code, credit in README)

- `pedroprates/claude-code-usage` (macOS menu bar) — same bridge architecture: status line → atomic `state.json`, chaining of an existing status line, max-per-`resets_at` across sessions.
- `michaelpeeters/claude-code-usage` (PyQt6, Windows supported) — `--statusline` bridge mode, installer that never overwrites an existing custom status line.
- `heavyc-dev/heavy-usage` — status line capture feeding a hook; threshold bands per window.
Design idea (not code) worth adopting: colour by **pace ratio** = % used ÷ % of window elapsed, with guards (below 20 % used or in the first 10 % of a window, fall back to plain thresholds; above 85–90 % used, red regardless), plus a marker on each bar showing where "even spend" would be now.

## 5. Operating mode

- **Prereqs (run 1):** check `git`, `gh auth status`, `node`, `npm`, `rustc`/`cargo`, MSVC C++ build tools, WebView2. Install Rust via `winget install Rustlang.Rustup` if missing. If MSVC build tools are missing, STOP.
- **Repo (run 1):** `git init`; `gh repo create oallaire-cyber/Claude_Usage_Widget --private --source . --remote origin`; branch `main`.
- **Files you maintain:** `CLAUDE.md` (conventions, commands, architecture), `PROGRESS.md` (phase checklist + Definition-of-Done checklist from §7 with ticks, next step, known issues), `DECISIONS.md`, `MANUAL_CHECKS.md` (what only Olivier can verify, with exact steps), `BACKLOG.md`.
- **Git:** small Conventional Commits; push at the end of every phase and before stopping for any reason.
- **CI:** GitHub Actions on `windows-latest`: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, frontend typecheck + build. Release workflow on tag push builds the installer and attaches it to a draft GitHub Release.
- **UI verification:** a preview mode renders the popup from fixture state in a plain browser page; screenshot it with Playwright in light and dark themes for each state (normal, ≥70%, ≥90%, stale, window reset, no data yet), save to `docs/screenshots/`, look at them yourself, fix within the polish budget.
- Use subagents for independent work when useful. Update `PROGRESS.md` before any context compaction.

## 6. Phases and acceptance criteria

**Phase 0 — Bootstrap.** Prereqs checked; repo created and pushed; Tauri app scaffolded and building; Cargo workspace with `app` and `bridge` crates; CI green; the five tracking files created.

**Phase 1 — Bridge, sources, install scripts.** Discover the structure of sources B and C with extraction scripts (§2.1) and document it in `docs/FINDINGS.md` with redacted fixtures — state explicitly whether a Fable/model-scoped window exists anywhere. Bridge parser tests cover: multi-session max-per-`resets_at`, Unix and ISO `resets_at`, missing `rate_limits`, missing `seven_day`, extra window, malformed JSON, empty stdin. Atomic write tested. Chaining tested with a dummy previous command (including timeout). Runtime measured and recorded. Install/uninstall scripts tested against fake settings files: other keys preserved, idempotent, uninstall restores the previous state exactly. `MANUAL_CHECKS.md` gets the install steps and "compare with claude.ai → Settings → Usage".

**Phase 2 — Core logic.** State model with statuses `ok`, `stale` (no update for > 30 min while a window is still active), `window_reset` (now > `resets_at`: show 0% as "reset — waiting for fresh data"), `no_data`. Merge of sources A/B/C per §3 (unit-tested). Pace per window: elapsed fraction = (now − (resets_at − window_length)) / window_length; pace ratio = used fraction ÷ elapsed fraction; status `on_track | ahead | will_exhaust` with projected exhaustion time (linear fit over recent history samples, fallback: average rate since window start). Notification logic: configurable thresholds per window, each fires once per window cycle; optional "limit reset" notification. All unit-tested, no I/O in tests.

**Phase 3 — Tray icon.** Rendered at runtime (16/20/24/32 px), ring filled to session %, colour by pace ratio with the guards in §4b (blue on/under pace, amber ≥ 1.1× pace, red ≥ 1.75× pace or ≥ 90 % used), grey for stale/no data. Pixel-level unit test of colour thresholds. Tooltip `Session 14% · resets 3h49 | Week 8% · resets Wed 06:00` plus "as of HH:MM". Right-click menu: Pin widget, Settings, Start with Windows, Quit.

**Phase 4 — Popup card.** Left-click opens a frameless card anchored to the tray (respect taskbar position, multi-monitor), closes on blur/Esc. Concentric rings (session outer, weekly inner, extra windows such as Fable further in), each with label, %, reset countdown, even-spend marker, one-line pace message. Footer "Data as of 14:32 (updates while Claude Code runs)". Follows Windows light/dark and accent colour; Mica/acrylic only if clean, else tasteful solid. French/English from the Windows display language, overridable. Target look: a first-party Windows 11 flyout — within the polish budget.

**Phase 5 — Alerts, settings, pinned mode.** Toast notifications wired to Phase 2 logic (defaults: session 80/95%, weekly 75/90%). Settings view: thresholds, language, notifications on/off, start with Windows. Pinned mode: small always-on-top, draggable, semi-transparent compact widget with persisted position. Single-instance enforced.

**Phase 6 — Release.** NSIS installer bundling app + bridge; the installer does NOT run the install script — README tells Olivier to run it. README: screenshots, install steps, SmartScreen note (unsigned), how data flows, privacy statement (reads only what Claude Code pipes to the status line; no network; nothing sent anywhere), known limitations (Fable window only if source B provides it; values update only while Claude Code or Claude Desktop runs; sources B and C are undocumented and may break). Tag `v0.1.0`; release workflow attaches installer to a draft release.

## 7. Definition of Done (whole project)

Copy this checklist into `PROGRESS.md` and tick items as they become true:

- [ ] `cargo test`, clippy, fmt and frontend build pass locally and in CI on `main`
- [ ] Bridge: all parser fixtures pass; exits 0 on malformed input; median runtime recorded and < 50 ms (or deviation explained in DECISIONS.md)
- [ ] Install/uninstall scripts pass tests against fake settings files; uninstall restores previous `statusLine` exactly
- [ ] Core: pace, staleness and notification-once logic unit-tested
- [ ] Tray icon colour thresholds verified by pixel test
- [ ] Preview screenshots exist in `docs/screenshots/` for every state in §5, light and dark
- [ ] Installer built by CI from tag `v0.1.0` and attached to a draft release
- [ ] README, MANUAL_CHECKS, DECISIONS, BACKLOG complete; PROGRESS shows every phase done

**When every box is ticked: write the final report and stop.** Do not add features, refactor working code, or polish further. Leftover ideas belong in `BACKLOG.md`.

**Per-run done:** the phases assigned in the launch message meet their acceptance criteria, are pushed, and `PROGRESS.md` is updated. Then write the run report and stop — do not start the next phase.

## 8. STOP conditions

Stop, update `PROGRESS.md`, commit and push what is safe, and write the run report when:
- MSVC build tools or WebView2 are missing (admin install needed);
- `gh` is not authenticated or repo creation fails;
- the only way forward would break a Hard rule in §2;
- the anti-loop rule (§2.6) blocks all remaining work in this run.

## 9. Run report (end of every run, printed as your final message and appended to PROGRESS.md)

What was completed, Definition-of-Done ticks so far, deviations from this prompt and why, open issues, what the next run should do, and the manual checks Olivier should do now.
