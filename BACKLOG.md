# Backlog

Ideas outside the current scope (PROMPT.md §2.5). One line of rationale each. Not to be implemented without a new decision.

- **Borrow a still-valid reset time for Desktop-only values** — when the status line's `resets_at` is in the future and the Desktop sample was taken inside that same window, countdown and pace could still be shown; the prompt asks to show them as unavailable in that mode.
- **Offer `refreshInterval` in the install script** — would keep the status line (and so the widget) ticking while Claude Code sits idle, at the cost of a process launch every N seconds; values only change after API calls anyway.
- **Chart of Desktop's 30-day history** — `plan-usage-history.json` holds a month of 15-minute samples; a sparkline would show usage patterns, but it is not in the spec.
- **Weekly warning on the tray icon** — the ring follows the session only (spec); a small weekly marker (dot or inner arc) would flag a weekly limit close to exhaustion while the session is fine.
- **Mica backdrop for the popup** — would match Windows 11 flyouts more closely; needs a transparent WebView and a manual visual check (D-20).
- **Popup polish nits** (budget used): the hero ring area could be more compact when there is one window; settings window title does not switch language until reopened; the reset row's centre could show the next reset time.
- **French screenshots in the README** — both languages are captured in `docs/screenshots/`; the README (Phase 6) will likely show English only.
- **Job-object assignment race** — the chained command's children spawned in the first microseconds before job assignment escape the kill-on-timeout; would need `CREATE_SUSPENDED` + resume via FFI. Low risk for status-line scripts.

