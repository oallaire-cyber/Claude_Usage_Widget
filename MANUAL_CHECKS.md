# Manual checks (only Olivier can do these)

Each check lists exact steps and what you should see. Nothing here has been run on your real
Claude Code setup: every automated test used fake settings files.

## 1. Install the bridge into Claude Code (after Phase 1)

Needs a built bridge: `cargo build --release -p cuw-bridge` in the repo (or, after Phase 6, the copy
shipped with the installer).

1. Open **Windows PowerShell** (5.1) in `C:\Users\olive\Documents\Claude_Usage_Widget`.
2. Dry look first — what it will change: your `~/.claude/settings.json` currently has **no** `statusLine`.
3. Run:
   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File tools\install-bridge.ps1
   ```
   Expected output: `Backup: ...settings.json.bak-YYYYMMDD-HHMMSS` then
   `Installed: statusLine now runs C:/Users/olive/AppData/Local/ClaudeUsageWidget/bin/cuw-bridge.exe`.
4. Check `%USERPROFILE%\.claude\settings.json`: every other key unchanged, plus
   `"statusLine": { "type": "command", "command": "C:/Users/olive/AppData/Local/ClaudeUsageWidget/bin/cuw-bridge.exe" }`.
5. Start (or continue) a Claude Code session and send one message. The status line under the prompt
   should read like `5h 14% · 7d 8%` (before the first reply it may show `5h -- · 7d --`).
6. Check `%APPDATA%\ClaudeUsageWidget\state.json` exists and shows `five_hour` / `seven_day`.

## 2. Compare with claude.ai (after install)

1. Open claude.ai → **Settings → Usage**.
2. Compare "Current session" with the status line's `5h` value and "Weekly — all models" with `7d`.
   They should match to within a percent or two (Claude Code only refreshes after its own API calls, so
   usage from other devices shows up after your next Claude Code message).
3. Note whether claude.ai shows a separate weekly limit for a specific model (e.g. Fable). If it does,
   it will **not** appear in the widget — no local source provides it today (docs/FINDINGS.md).

## 3. Undo (any time)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools\uninstall-bridge.ps1
```
Expected: `Backup: ...`, `statusLine removed.`, and the copied `cuw-bridge.exe` deleted. Your
settings file returns exactly to what it was before install. If a Claude Code session is running the
bridge at that moment, deleting the exe may fail with a warning — delete it later.
