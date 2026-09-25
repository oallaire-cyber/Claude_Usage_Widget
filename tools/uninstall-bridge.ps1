<#
.SYNOPSIS
    Removes cuw-bridge.exe from Claude Code's status line and restores the previous statusLine exactly.

.DESCRIPTION
    Windows PowerShell 5.1. Uses the record written by install-bridge.ps1 (<DataDir>\install.json).
    - If statusLine is no longer the bridge (you changed it since), the settings are left untouched
      unless -Force is given.
    - Backs up the settings file first; writes UTF-8 without BOM; preserves every other key and order.
    - If install created the settings file and it would now be empty, the file is removed again.

.PARAMETER SettingsPath
    Default: the path recorded at install time, else %USERPROFILE%\.claude\settings.json

.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\ClaudeUsageWidget\bin (the copied bridge is deleted from here).

.PARAMETER DataDir
    Default: %APPDATA%\ClaudeUsageWidget
#>
[CmdletBinding()]
param(
    [string]$SettingsPath,
    [string]$InstallDir,
    [string]$DataDir,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
Import-Module (Join-Path $PSScriptRoot 'CuwJson.psm1') -Force

if (-not $DataDir) { $DataDir = Get-CuwDefaultDataDir }
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA 'ClaudeUsageWidget\bin' }
$recordPath = Join-Path $DataDir 'install.json'

try {
    $record = $null
    if (Test-Path -LiteralPath $recordPath) { $record = Read-CuwJsonFile $recordPath }
    if (-not $SettingsPath) {
        $SettingsPath = Get-CuwProperty $record 'settings_path'
        if (-not $SettingsPath) { $SettingsPath = Get-CuwDefaultSettingsPath }
    }
    $bridgeCommand = Get-CuwProperty $record 'bridge_command'

    if (-not (Test-Path -LiteralPath $SettingsPath)) {
        Write-Host "No settings file at $SettingsPath. Nothing to restore."
    }
    else {
        $settings = Read-CuwJsonFile $SettingsPath
        $current = Get-CuwProperty $settings 'statusLine'
        $currentCmd = [string](Get-CuwProperty $current 'command')
        $isBridge = ($bridgeCommand -and $currentCmd -eq $bridgeCommand) -or ($currentCmd -match 'cuw-bridge\.exe')

        if (-not $isBridge -and -not $Force) {
            Write-Warning "statusLine is not the bridge (it is: '$currentCmd'). Settings left unchanged. Use -Force to restore anyway."
        }
        elseif ($null -eq $record -and -not $Force) {
            Write-Warning "No install record at $recordPath, so the previous statusLine is unknown. Settings left unchanged. Use -Force to just remove statusLine."
        }
        else {
            $hadStatusLine = [bool](Get-CuwProperty $record 'had_status_line')
            if ($hadStatusLine) {
                Set-CuwProperty $settings 'statusLine' (Get-CuwProperty $record 'previous_status_line')
            }
            else {
                $settings.PSObject.Properties.Remove('statusLine')
            }

            $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
            $backup = "$SettingsPath.bak-$stamp"
            $n = 1
            while (Test-Path -LiteralPath $backup) { $backup = "$SettingsPath.bak-$stamp-$n"; $n++ }
            Copy-Item -LiteralPath $SettingsPath -Destination $backup
            Write-Host "Backup: $backup"

            $settingsExisted = $true
            if (Test-CuwProperty $record 'settings_existed') { $settingsExisted = [bool](Get-CuwProperty $record 'settings_existed') }
            if (-not $settingsExisted -and @($settings.PSObject.Properties).Count -eq 0) {
                Remove-Item -LiteralPath $SettingsPath
                Write-Host "Removed $SettingsPath (install had created it; it is empty again)."
            }
            else {
                Write-CuwFileAtomic $SettingsPath ((ConvertTo-CuwJson $settings) + "`n")
                if ($hadStatusLine) { Write-Host 'Previous statusLine restored.' } else { Write-Host 'statusLine removed.' }
            }
            if (Test-Path -LiteralPath $recordPath) { Remove-Item -LiteralPath $recordPath }
        }
    }

    $exe = Join-Path $InstallDir 'cuw-bridge.exe'
    if (Test-Path -LiteralPath $exe) {
        try { Remove-Item -LiteralPath $exe; Write-Host "Removed $exe" }
        catch { Write-Warning "Could not delete $exe (a Claude Code session may be running it). Delete it later." }
    }
    exit 0
}
catch {
    Write-Error "Uninstall failed, settings left unchanged: $($_.Exception.Message)"
    exit 1
}
