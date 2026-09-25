<#
    Tests for install-bridge.ps1 / uninstall-bridge.ps1 (Windows PowerShell 5.1).
    Everything runs against FAKE settings files in a temp folder; the real %USERPROFILE%\.claude is
    never touched. Exit code 0 = all passed.
#>
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$tools = Split-Path -Parent $PSScriptRoot
$install = Join-Path $tools 'install-bridge.ps1'
$uninstall = Join-Path $tools 'uninstall-bridge.ps1'
$root = Join-Path ([System.IO.Path]::GetTempPath()) ("cuw-install-tests-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $root | Out-Null

$script:failures = 0
$script:passes = 0
function Assert([bool]$Condition, [string]$Message) {
    if ($Condition) { $script:passes++ } else { $script:failures++; Write-Host "  FAIL: $Message" -ForegroundColor Red }
}

# Canonical form using PowerShell's own serializer (independent of the scripts' writer).
function Canon([string]$Text) {
    if ([string]::IsNullOrWhiteSpace($Text)) { return '' }
    return (ConvertFrom-Json $Text | ConvertTo-Json -Depth 100 -Compress)
}
function ReadText([string]$Path) { return [System.IO.File]::ReadAllText($Path, (New-Object System.Text.UTF8Encoding($false))) }
function WriteText([string]$Path, [string]$Text) {
    [System.IO.File]::WriteAllBytes($Path, (New-Object System.Text.UTF8Encoding($false)).GetBytes($Text))
}
function HasBom([string]$Path) {
    $b = [System.IO.File]::ReadAllBytes($Path)
    return ($b.Length -ge 3 -and $b[0] -eq 0xEF -and $b[1] -eq 0xBB -and $b[2] -eq 0xBF)
}

# Fresh sandbox per case: fake settings dir, data dir, install dir and a fake bridge binary.
function New-Case([string]$Name) {
    $d = Join-Path $root $Name
    New-Item -ItemType Directory -Path $d | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $d 'claude') | Out-Null
    $src = Join-Path $d 'cuw-bridge-src.exe'
    WriteText $src 'fake bridge binary'
    return [pscustomobject]@{
        Dir      = $d
        Settings = (Join-Path $d 'claude\settings.json')
        Data     = (Join-Path $d 'data')
        Bin      = (Join-Path $d 'bin')
        Src      = $src
    }
}
# The child's error stream must not become a terminating error here (PS 5.1 wraps native stderr).
function Invoke-Install($c) {
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -ExecutionPolicy Bypass -File $install -SettingsPath $c.Settings -BridgeSource $c.Src -InstallDir $c.Bin -DataDir $c.Data *>&1 | Out-Null
    return $LASTEXITCODE
}
function Invoke-Uninstall($c, [switch]$Force) {
    $ErrorActionPreference = 'Continue'
    if ($Force) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $uninstall -SettingsPath $c.Settings -InstallDir $c.Bin -DataDir $c.Data -Force *>&1 | Out-Null
    } else {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $uninstall -SettingsPath $c.Settings -InstallDir $c.Bin -DataDir $c.Data *>&1 | Out-Null
    }
    return $LASTEXITCODE
}
function Get-StatusLine($c) { return (ConvertFrom-Json (ReadText $c.Settings)).statusLine }
function Get-Backups($c) { return ,@(Get-ChildItem -LiteralPath (Split-Path $c.Settings) -Filter 'settings.json.bak-*') }
function ExpectedCommand($c) { return ([System.IO.Path]::GetFullPath((Join-Path $c.Bin 'cuw-bridge.exe'))).Replace('\', '/') }

# A settings file in Claude Code's own layout (2-space JSON), with awkward content.
$eAcute = [string][char]0x00E9
$emoji = [char]::ConvertFromUtf32(0x1F600)
$richSettings = @"
{
  "permissions": {
    "allow": [
      "Bash(npm test:*)",
      "Read(C:\\Users\\x\\file.txt)"
    ],
    "deny": [],
    "defaultMode": "acceptEdits"
  },
  "model": "opus",
  "env": {
    "GREETING": "caf$eAcute $emoji <tag> & 'quote' \"dq\" tab\there"
  },
  "numbers": {
    "int": 42,
    "neg": -7,
    "float": 0.1,
    "big": 9007199254740993,
    "exp": 1.5E-07
  },
  "flags": [
    true,
    false,
    null
  ],
  "emptyObj": {},
  "single": [
    "only"
  ],
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit",
        "hooks": [
          {
            "type": "command",
            "command": "echo done"
          }
        ]
      }
    ]
  }
}
"@.Replace("`r`n", "`n") + "`n"

Write-Host 'Case 1: no statusLine, other keys preserved, idempotent, exact uninstall'
$c = New-Case 'rich'
WriteText $c.Settings $richSettings
Assert ((Invoke-Install $c) -eq 0) 'install exits 0'
$sl = Get-StatusLine $c
Assert ($sl.type -eq 'command') 'statusLine.type = command'
Assert ($sl.command -eq (ExpectedCommand $c)) "statusLine.command = bridge path with forward slashes ($($sl.command))"
Assert (-not $sl.command.Contains('\')) 'no backslashes in command'
Assert (Test-Path (Join-Path $c.Bin 'cuw-bridge.exe')) 'bridge copied to install dir'
Assert (-not (HasBom $c.Settings)) 'settings written without BOM'
$after = ConvertFrom-Json (ReadText $c.Settings)
$after.PSObject.Properties.Remove('statusLine')
Assert (($after | ConvertTo-Json -Depth 100 -Compress) -eq (Canon $richSettings)) 'every other key preserved'
$names = @((ConvertFrom-Json (ReadText $c.Settings)).PSObject.Properties | ForEach-Object { $_.Name })
Assert (($names -join ',') -eq 'permissions,model,env,numbers,flags,emptyObj,single,hooks,statusLine') "key order preserved ($($names -join ','))"
Assert ((Get-Backups $c).Count -eq 1) 'one timestamped backup'
Assert ((ReadText (Get-Backups $c)[0].FullName) -eq $richSettings) 'backup is byte-identical to the original'
$rec = ConvertFrom-Json (ReadText (Join-Path $c.Data 'install.json'))
Assert ($rec.had_status_line -eq $false) 'record: had no statusLine'
Assert ($null -eq $rec.previous_status_line) 'record: previous_status_line is null'

$snapshot = ReadText $c.Settings
$recSnapshot = ReadText (Join-Path $c.Data 'install.json')
Assert ((Invoke-Install $c) -eq 0) 'second install exits 0'
Assert ((ReadText $c.Settings) -eq $snapshot) 'second install leaves settings byte-identical'
Assert ((ReadText (Join-Path $c.Data 'install.json')) -eq $recSnapshot) 'second install leaves record unchanged'
Assert ((Get-Backups $c).Count -eq 1) 'second install makes no new backup'

Assert ((Invoke-Uninstall $c) -eq 0) 'uninstall exits 0'
Assert ((ReadText $c.Settings) -eq $richSettings) 'uninstall restores the original byte-for-byte'
Assert (-not (Test-Path (Join-Path $c.Data 'install.json'))) 'install record removed'
Assert (-not (Test-Path (Join-Path $c.Bin 'cuw-bridge.exe'))) 'bridge binary removed'

Write-Host 'Case 2: existing statusLine is recorded, chained, and restored exactly'
$c = New-Case 'existing'
$withSl = @"
{
  "model": "sonnet",
  "statusLine": {
    "type": "command",
    "command": "bash ~/.claude/statusline.sh --flag \"quoted arg\"",
    "padding": 2,
    "refreshInterval": 5
  },
  "theme": "dark"
}
"@.Replace("`r`n", "`n") + "`n"
WriteText $c.Settings $withSl
Assert ((Invoke-Install $c) -eq 0) 'install exits 0'
$sl = Get-StatusLine $c
Assert ($sl.command -eq (ExpectedCommand $c)) 'statusLine points at the bridge'
Assert ($sl.padding -eq 2 -and $sl.refreshInterval -eq 5) 'other statusLine options kept'
$rec = ConvertFrom-Json (ReadText (Join-Path $c.Data 'install.json'))
Assert ($rec.had_status_line -eq $true) 'record: had statusLine'
Assert ($rec.previous_status_line.command -eq 'bash ~/.claude/statusline.sh --flag "quoted arg"') 'record: previous command verbatim'
$names = @((ConvertFrom-Json (ReadText $c.Settings)).PSObject.Properties | ForEach-Object { $_.Name })
Assert (($names -join ',') -eq 'model,statusLine,theme') 'statusLine keeps its position'
Assert ((Invoke-Install $c) -eq 0) 'second install exits 0'
$rec2 = ConvertFrom-Json (ReadText (Join-Path $c.Data 'install.json'))
Assert ($rec2.previous_status_line.command -eq $rec.previous_status_line.command) 'second install does not record the bridge as "previous"'
Assert ((Invoke-Uninstall $c) -eq 0) 'uninstall exits 0'
Assert ((ReadText $c.Settings) -eq $withSl) 'previous statusLine restored byte-for-byte'

Write-Host 'Case 3: no settings file at all'
$c = New-Case 'nofile'
Assert ((Invoke-Install $c) -eq 0) 'install exits 0'
Assert (Test-Path $c.Settings) 'settings file created'
Assert ((Get-StatusLine $c).command -eq (ExpectedCommand $c)) 'statusLine set'
Assert ((Get-Backups $c).Count -eq 0) 'no backup when there was no file'
Assert ((Invoke-Uninstall $c) -eq 0) 'uninstall exits 0'
Assert (-not (Test-Path $c.Settings)) 'settings file removed again (restores "no file")'

Write-Host 'Case 4: invalid JSON is refused without touching anything'
$c = New-Case 'invalid'
$bad = "{ `"model`": `"opus`", oops }`n"
WriteText $c.Settings $bad
Assert ((Invoke-Install $c) -ne 0) 'install fails'
Assert ((ReadText $c.Settings) -eq $bad) 'settings untouched'
Assert (-not (Test-Path (Join-Path $c.Data 'install.json'))) 'no install record'

Write-Host 'Case 5: user changed statusLine after install: uninstall leaves it alone'
$c = New-Case 'changed'
WriteText $c.Settings "{`n  `"model`": `"opus`"`n}`n"
Assert ((Invoke-Install $c) -eq 0) 'install exits 0'
$mine = "{`n  `"model`": `"opus`",`n  `"statusLine`": {`n    `"type`": `"command`",`n    `"command`": `"my-new-line`"`n  }`n}`n"
WriteText $c.Settings $mine
Assert ((Invoke-Uninstall $c) -eq 0) 'uninstall exits 0'
Assert ((ReadText $c.Settings) -eq $mine) 'settings untouched'

Write-Host 'Case 6: empty settings file and a BOM-prefixed file'
$c = New-Case 'empty'
WriteText $c.Settings ''
Assert ((Invoke-Install $c) -eq 0) 'install on empty file exits 0'
Assert ((Get-StatusLine $c).command -eq (ExpectedCommand $c)) 'statusLine set on empty file'
$c = New-Case 'bom'
$bomText = [string][char]0xFEFF + "{`n  `"model`": `"opus`"`n}`n"
WriteText $c.Settings $bomText
Assert ((Invoke-Install $c) -eq 0) 'install on BOM file exits 0'
Assert (-not (HasBom $c.Settings)) 'BOM not written back'
Assert ((ConvertFrom-Json (ReadText $c.Settings)).model -eq 'opus') 'content kept'

Write-Host 'Case 7: non-canonical formatting is preserved semantically'
$c = New-Case 'compact'
$compact = '{"a":1,"b":[1,2,{"c":null}],"s":"x\u00e9\ny","statusLine":{"type":"command","command":"old"}}'
WriteText $c.Settings $compact
Assert ((Invoke-Install $c) -eq 0) 'install exits 0'
Assert ((Invoke-Uninstall $c) -eq 0) 'uninstall exits 0'
Assert ((Canon (ReadText $c.Settings)) -eq (Canon $compact)) 'same JSON after install + uninstall'

Remove-Item -LiteralPath $root -Recurse -Force
Write-Host ''
Write-Host "Install-script tests: $script:passes passed, $script:failures failed"
if ($script:failures -gt 0) { exit 1 } else { exit 0 }
