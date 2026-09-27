# Backlog

Ideas outside the current scope (PROMPT.md §2.5). One line of rationale each. Not to be implemented without a new decision.

- **Borrow a still-valid reset time for Desktop-only values** — when the status line's `resets_at` is in the future and the Desktop sample was taken inside that same window, countdown and pace could still be shown; the prompt asks to show them as unavailable in that mode.
- **Offer `refreshInterval` in the install script** — would keep the status line (and so the widget) ticking while Claude Code sits idle, at the cost of a process launch every N seconds; values only change after API calls anyway.
- **Chart of Desktop's 30-day history** — `plan-usage-history.json` holds a month of 15-minute samples; a sparkline would show usage patterns, but it is not in the spec.
- **Weekly warning on the tray icon** — the ring follows the session only (spec); a small weekly marker (dot or inner arc) would flag a weekly limit close to exhaustion while the session is fine.
- **Mica backdrop for the popup** — would match Windows 11 flyouts more closely; needs a transparent WebView and a manual visual check (D-20).
- **Popup polish nits** (budget used): the hero ring area could be more compact when there is one window; settings window title does not switch language until reopened; the reset row's centre could show the next reset time.
- **French screenshots in the README** — both languages are captured in `docs/screenshots/`; the README (Phase 6) will likely show English only.
- **Unhook the bridge from the Windows uninstaller** — would save a manual step, but means the installer touching `~/.claude/settings.json`, which the prompt keeps in Olivier's hands (D-25).
- **Code-sign the installer** — removes the SmartScreen warning; needs a paid certificate.
- **Verify the settings round-trip before writing** (D-26) — PowerShell 5.1's parser alters rare values (a `"\/Date(…)\/"` string, integers above ~1e28, lone surrogate escapes); re-parsing the output and comparing with the input would refuse instead. Unlikely in `settings.json`.
- **Resolve a relative `-SettingsPath` once** (D-26) — `Test-Path` and `[IO.File]` resolve relative paths against different folders; only affects a custom argument, the default is absolute.
- **Job-object assignment race** — the chained command's children spawned in the first microseconds before job assignment escape the kill-on-timeout; would need `CREATE_SUSPENDED` + resume via FFI. Low risk for status-line scripts.

