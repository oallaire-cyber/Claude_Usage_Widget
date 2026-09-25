# Data-source findings

Discovery run on 2026-09-25 on Olivier's machine: Windows 11, Claude Code **2.1.282**, Claude Desktop
installed (classic, non-MSIX data folder in use).

All inspection went through `tools/extract-claude-code-cache.mjs` and
`tools/extract-claude-desktop-history.mjs`, which print only whitelisted usage fields, mask any string
longer than 40 characters or shaped like a token/UUID, and never print the rest of those files (they
can contain MCP server API keys). `~/.claude/.credentials.json` was not read.

## Is there a Fable / model-scoped weekly window anywhere?

**No — not in any local source on this machine today.**

| Source | Model-scoped window? |
|---|---|
| A. Status-line payload (`rate_limits`) | Not documented. Official docs list `five_hour`, `seven_day` and `spend_limit` only; the Fable weekly window is known to be missing (anthropics/claude-code#91920). The parser keeps any additional window automatically, so it will appear if Claude Code adds one. |
| B. `~/.claude.json` → `cachedUsageUtilization` | **Block absent** in Claude Code 2.1.282. |
| C. Claude Desktop `plan-usage-history.json` | Only `fh`, `sd`, `xu` keys across all 724 samples — no model-scoped key. |

Consequence: the Fable ring cannot be restored from local data for now. Recorded as a known limitation
(README, Phase 6).

## A. Status-line payload (primary, documented)

Source: https://code.claude.com/docs/en/statusline (read 2026-09-25). Differences from the prompt's
summary are in `DECISIONS.md` (D-04). Shape used by the parser:

```json
"rate_limits": {
  "five_hour":   { "used_percentage": 23.5,  "resets_at": 1738425600 },
  "seven_day":   { "used_percentage": 41.2,  "resets_at": 1738857600 },
  "spend_limit": { "used_percentage": 104.5, "resets_at": 1740787200 }
}
```

- `resets_at`: documented as Unix seconds; ISO 8601 also accepted.
- `rate_limits` appears only for Pro/Max (or a gateway spend limit), only after the first API response;
  each window may be independently absent; a window is dropped once its `resets_at` passes.
- Olivier currently has **no `statusLine`** configured (checked by printing only that key of
  `~/.claude/settings.json`), so after install the bridge prints its own default line.

Fixtures: `fixtures/statusline/*.json` (documented, ISO `resets_at`, no `rate_limits`, no `seven_day`,
extra window, truncated JSON, empty).

## B. Claude Code cache `~/.claude.json` (secondary, undocumented)

- File present (≈ 90 KB). **No `cachedUsageUtilization` key.**
- Other top-level key *names* matching usage/limit/rate (values not printed):
  `cachedExtraUsageDisabledReason`, `skillUsage`, `pluginUsage`. None carries rate-limit percentages
  (`skillUsage`/`pluginUsage` are feature-usage counters by name; `cachedExtraUsageDisabledReason` is a
  reason string for "extra usage" being disabled).
- Consequence: no real fixture can be made. The app still watches the file and parses the block
  defensively if a future version writes it (shape assumed in `DECISIONS.md`); absent block = source B
  absent, never an error.

## C. Claude Desktop `plan-usage-history.json` (tertiary, undocumented)

Locations checked:

| Location | Present |
|---|---|
| `%APPDATA%\Claude\plan-usage-history.json` | **yes** (≈ 63 KB, last written 2026-09-25 06:54 UTC) |
| `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\plan-usage-history.json` | no (package folder exists, file absent) |

Structure (all 724 samples, 2026-08-26 → 2026-09-25):

```json
{
  "version": 2,
  "samples": [
    { "t": 1790319250345, "org": "<uuid>", "u": { "fh": 0, "sd": 9, "xu": 100 } }
  ]
}
```

| Field | Meaning (inferred) | Observed |
|---|---|---|
| `t` | sample time, **Unix milliseconds** | median gap 15 min (min 4.5 min) — written while Desktop runs |
| `org` | organisation UUID | 1 distinct value (masked) |
| `u.fh` | five-hour window, % used | integers 0–100, in every sample |
| `u.sd` | seven-day window, % used | integers 0–67, in every sample |
| `u.xu` | "extra usage" (paid overage) % used — inferred from the name and the `cachedExtraUsageDisabledReason` key in source B | 21 of 724 samples, fractional (8.46–100) |

- **No reset times** — countdown and pace are unavailable when this is the only source.
- Latest sample is the newest `t` (the array is chronological, but the parser does not rely on order).
- Several `org` values could appear for multi-organisation accounts; the parser takes the newest sample
  overall.

Redacted fixture: `fixtures/desktop/plan-usage-history.redacted.json` — real structure, org zeroed,
values synthetic.
