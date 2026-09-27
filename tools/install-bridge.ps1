<#
.SYNOPSIS
    Hooks cuw-bridge.exe into Claude Code as its status-line command.

.DESCRIPTION
    Windows PowerShell 5.1. Run it yourself; nothing runs it automatically.
    - Copies cuw-bridge.exe to -InstallDir (a path without spaces, so the command works whether Claude
      Code runs it through Git Bash or PowerShell).
    - Backs up the settings file (timestamped copy next to it), then sets "statusLine" to the bridge,
      preserving every other key and their order. Writes UTF-8 without BOM.
    - Records any existing statusLine in <DataDir>\install.json: the bridge keeps running that command
      and prints its output unchanged, and uninstall-bridge.ps1 restores it exactly.
    - Idempotent: running it again changes nothing.

.PARAMETER SettingsPath
    Claude Code settings file. Default: %USERPROFILE%\.claude\settings.json

.PARAMETER BridgeSource
    cuw-bridge.exe to install. Default: next to this script, else target\release\cuw-bridge.exe.

.PARAMETER InstallDir
    Where the bridge is copied. Default: %LOCALAPPDATA%\ClaudeUsageWidget\bin

.PARAMETER DataDir
    Where install.json is written. Default: %APPDATA%\ClaudeUsageWidget (the folder the bridge reads).
    Change it only for testing.
#>
[CmdletBinding()]
param(
    [string]$SettingsPath,
    [string]$BridgeSource,
    [string]$InstallDir,
    [string]$DataDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
Import-Module (Join-Path $PSScriptRoot 'CuwJson.psm1') -Force

if (-not $SettingsPath) { $SettingsPath = Get-CuwDefaultSettingsPath }
if (-not $DataDir) { $DataDir = Get-CuwDefaultDataDir }
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA 'ClaudeUsageWidget\bin' }
if (-not $BridgeSource) {
    foreach ($candidate in @((Join-Path $PSScriptRoot 'cuw-bridge.exe'),
                             (Join-Path $PSScriptRoot '..\cuw-bridge.exe'),
                             (Join-Path $PSScriptRoot '..\target\release\cuw-bridge.exe'))) {
        if (Test-Path -LiteralPath $candidate) { $BridgeSource = $candidate; break }
    }
}
if (-not $BridgeSource -or -not (Test-Path -LiteralPath $BridgeSource)) {
    Write-Error "cuw-bridge.exe not found. Pass -BridgeSource <path>."
    exit 2
}

# The command string as Claude Code will run it: forward slashes, no spaces (see DECISIONS.md).
function Get-BridgeCommand([string]$ExePath) {
    $full = [System.IO.Path]::GetFullPath($ExePath)
    $home_ = [System.IO.Path]::GetFullPath($env:USERPROFILE).TrimEnd('\')
    $cmd = $full.Replace('\', '/')
    if ($cmd.Contains(' ') -and $full.StartsWith($home_ + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        $cmd = '~/' + $full.Substring($home_.Length + 1).Replace('\', '/')
    }
    if ($cmd.Contains(' ')) {
        throw "The bridge path contains a space ($full). Pass -InstallDir with a path without spaces."
    }
    return $cmd
}

try {
    # 1. Read and validate the settings before changing anything.
    $settingsExisted = Test-Path -LiteralPath $SettingsPath
    if ($settingsExisted) { $settings = Read-CuwJsonFile $SettingsPath }
    else { $settings = New-Object PSObject }

    $installedExe = Join-Path $InstallDir 'cuw-bridge.exe'
    $bridgeCommand = Get-BridgeCommand $installedExe

    # 2. Install (or refresh) the binary.
    if (-not (Test-Path -LiteralPath $InstallDir)) { New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null }
    $srcFull = [System.IO.Path]::GetFullPath($BridgeSource)
    if ($srcFull -ne [System.IO.Path]::GetFullPath($installedExe)) {
        Copy-Item -LiteralPath $srcFull -Destination $installedExe -Force
    }

    # 3. Already installed? Then there is nothing to change.
    $current = Get-CuwProperty $settings 'statusLine'
    if ((Get-CuwProperty $current 'command') -eq $bridgeCommand) {
        Write-Host "Already installed: statusLine runs $bridgeCommand. Nothing changed."
        exit 0
    }

    # 4. Record the previous statusLine (exactly as it was) for chaining and uninstall.
    $recordPath = Join-Path $DataDir 'install.json'
    $recSettingsExisted = $settingsExisted
    $recHadStatusLine = (Test-CuwProperty $settings 'statusLine')
    $recPrevious = $current
    # The current statusLine may itself be an older bridge (another -InstallDir, or a copy run from the
    # repo). Then the existing record holds the user's real previous statusLine: keep it. Recording the
    # old bridge instead would make uninstall restore a command pointing at a deleted exe.
    if ([string](Get-CuwProperty $current 'command') -match '(^|[\\/"])cuw-bridge\.exe"?\s*$') {
        if (Test-Path -LiteralPath $recordPath) {
            $oldRecord = Read-CuwJsonFile $recordPath
            if (Test-CuwProperty $oldRecord 'settings_existed') { $recSettingsExisted = [bool](Get-CuwProperty $oldRecord 'settings_existed') }
            $recHadStatusLine = [bool](Get-CuwProperty $oldRecord 'had_status_line')
            $recPrevious = Get-CuwProperty $oldRecord 'previous_status_line'
        } else {
            # No record left: the user's original statusLine is unknown, so uninstall will remove it.
            $recHadStatusLine = $false
            $recPrevious = $null
        }
    }
    $record = [ordered]@{
        schema               = 1
        settings_path        = [System.IO.Path]::GetFullPath($SettingsPath)
        settings_existed     = $recSettingsExisted
        had_status_line      = $recHadStatusLine
        previous_status_line = $recPrevious
        bridge_command       = $bridgeCommand
        installed_at         = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    }
    $recordText = (ConvertTo-CuwJson $record) + "`n"

    # 5. New statusLine: keep the previous object's other options (padding, refreshInterval, ...).
    $newStatusLine = New-Object PSObject
    if ($current -is [System.Management.Automation.PSCustomObject]) {
        foreach ($p in $current.PSObject.Properties) { Set-CuwProperty $newStatusLine $p.Name $p.Value }
    }
    Set-CuwProperty $newStatusLine 'type' 'command'
    Set-CuwProperty $newStatusLine 'command' $bridgeCommand
    Set-CuwProperty $settings 'statusLine' $newStatusLine
    $settingsText = (ConvertTo-CuwJson $settings) + "`n"

    # 6. Backup, record, then write settings (the destination is replaced last).
    if ($settingsExisted) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $backup = "$SettingsPath.bak-$stamp"
        $n = 1
        while (Test-Path -LiteralPath $backup) { $backup = "$SettingsPath.bak-$stamp-$n"; $n++ }
        Copy-Item -LiteralPath $SettingsPath -Destination $backup
        Write-Host "Backup: $backup"
    }
    Write-CuwFileAtomic $recordPath $recordText
    Write-CuwFileAtomic $SettingsPath $settingsText

    if ($null -ne $recPrevious) { Write-Host "Previous statusLine recorded; the bridge will keep showing its output." }
    Write-Host "Installed: statusLine now runs $bridgeCommand"
    Write-Host "Restart any running Claude Code session (or send a message) to start feeding the widget."
    exit 0
}
catch {
    Write-Error "Install failed, settings left unchanged: $($_.Exception.Message)"
    exit 1
}
