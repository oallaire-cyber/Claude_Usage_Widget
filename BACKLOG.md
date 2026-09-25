# Backlog

Ideas outside the current scope (PROMPT.md §2.5). One line of rationale each. Not to be implemented without a new decision.

- **Borrow a still-valid reset time for Desktop-only values** — when the status line's `resets_at` is in the future and the Desktop sample was taken inside that same window, countdown and pace could still be shown; the prompt asks to show them as unavailable in that mode.
- **Offer `refreshInterval` in the install script** — would keep the status line (and so the widget) ticking while Claude Code sits idle, at the cost of a process launch every N seconds; values only change after API calls anyway.
- **Chart of Desktop's 30-day history** — `plan-usage-history.json` holds a month of 15-minute samples; a sparkline would show usage patterns, but it is not in the spec.
- **Job-object assignment race** — the chained command's children spawned in the first microseconds before job assignment escape the kill-on-timeout; would need `CREATE_SUSPENDED` + resume via FFI. Low risk for status-line scripts.

