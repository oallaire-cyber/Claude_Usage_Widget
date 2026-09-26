# Manual checks (only Olivier can do these)

Each check lists exact steps and what you should see. Nothing here has been run on your real
Claude Code setup: every automated test used fake settings files.

## 0. Run the installer (after Phase 6)

1. Download `Claude.Usage.Widget_0.1.0_x64-setup.exe` from the **draft** release `v0.1.0` on GitHub
   (Releases page → the draft → Assets), or use `target\release\bundle\nsis\` from a local build.
2. Run it. Expected: SmartScreen "Windows protected your PC" (unsigned) → **More info → Run anyway**;
   **no** administrator prompt; it installs to `%LOCALAPPDATA%\Claude Usage Widget\`.
3. In that folder you should see `claude-usage-widget.exe`, `cuw-bridge.exe`, `uninstall.exe` and a
   `tools` folder with `install-bridge.ps1`, `uninstall-bridge.ps1`, `CuwJson.psm1`.
4. Check `%USERPROFILE%\.claude\settings.json` is **unchanged** (the installer must not hook the bridge).
5. Start the app from the Start menu. When a toast appears (check §3.8), it should now be labelled
   **Claude Usage Widget**, not Windows PowerShell.
6. Tick **Start with Windows**, then Task Manager → Startup apps → right-click the entry → Open file
   location: it must open the install folder above, not the repo's `target` folder.
7. If you are happy with the release: on GitHub, edit the draft release and **Publish** it (it stays a
   draft until you do).

## 1. Install the bridge into Claude Code (after Phase 1)

Needs a built bridge: after Phase 6, the copy shipped with the installer — run the script from the
install folder instead of the repo:
`powershell -NoProfile -ExecutionPolicy Bypass -File "$env:LOCALAPPDATA\Claude Usage Widget\tools\install-bridge.ps1"`
(or, from the repo, `cargo build --release -p cuw-bridge` first and use the steps below).

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

## 3. The tray app (after Phases 3–5)

Easiest: install it (§0) and start it from the Start menu. Or run it from the repo in **Windows PowerShell**:
```powershell
cd C:\Users\olive\Documents\Claude_Usage_Widget\app
npm ci
npx tauri build --debug --no-bundle
..\target\debug\claude-usage-widget.exe
```
It reads the real files (the bridge's `state.json`, `~/.claude.json`, Claude Desktop's history) and
never writes anywhere except `%APPDATA%\ClaudeUsageWidget\`. What was already checked automatically is in
DECISIONS.md D-22; what needs your eyes and mouse:

1. **Tray icon** — a ring in the notification area (you may need to drag it out of the `^` overflow
   onto the taskbar). Blue when on pace, amber when ahead, red near the limit, grey when stale or empty.
   It should look crisp, not blurry, at your display scaling.
2. **Tooltip** — hover the icon: `Session 14% · resets 3h49 | Week 8% · resets Wed 06:00` and a second
   line `as of HH:MM` (in French if Windows is in French).
3. **Card** — left-click: the card opens just above the icon (or next to it if your taskbar is on
   another side), fully on screen. Click elsewhere or press **Esc**: it closes. Click the icon again
   while it is open: it closes and does **not** immediately reopen. If you have a second monitor with a
   taskbar, try its tray too.
4. **Right-click menu** — Pin widget · Settings… · Start with Windows · Quit. The two tick marks must
   match reality after each click.
5. **Pinned widget** — "Pin widget": a small semi-transparent card appears bottom-right, stays on top,
   can be **dragged** anywhere (grab the text or ring). Quit and restart the app: it comes back at the same
   place. Hover shows two small buttons (open card, unpin).
6. **Settings** — change the language (the card and menu switch), switch notifications off and on,
   change thresholds (e.g. `50, 95`). Typing something invalid (e.g. `abc`) must underline the field in
   red and not save.
7. **Start with Windows** — tick it, then check Task Manager → Startup apps: "Claude Usage Widget" is
   listed. Sign out and in: only the tray icon (and the pinned widget, if pinned) appears, no card.
   Untick to remove the entry.
8. **Notifications** — when usage crosses 80 % (session) or 75 % (weekly), one toast appears, once per
   window. From a repo build the toast is labelled **Windows PowerShell**; the installed app shows its
   own name. If no toast ever appears, check Windows Settings → System →
   Notifications (and Focus / Do not disturb).
9. **Single instance** — start the exe a second time while it runs: no second icon; the card opens.

## 4. Undo the bridge install (any time)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:LOCALAPPDATA\Claude Usage Widget\tools\uninstall-bridge.ps1"
```
(or `tools\uninstall-bridge.ps1` from the repo). Do this **before** uninstalling the app from Windows
Settings → Apps, which removes the script.
Expected: `Backup: ...`, `statusLine removed.`, and the copied `cuw-bridge.exe` deleted. Your
settings file returns exactly to what it was before install. If a Claude Code session is running the
bridge at that moment, deleting the exe may fail with a warning — delete it later.
