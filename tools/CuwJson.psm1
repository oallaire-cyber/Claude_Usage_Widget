# Shared helpers for install-bridge.ps1 / uninstall-bridge.ps1 (Windows PowerShell 5.1).
#
# Windows PowerShell 5.1's ConvertTo-Json indents oddly, truncates at -Depth, and escapes < > ' &.
# ConvertTo-CuwJson writes 2-space JSON (the layout Claude Code itself uses) and keeps property order,
# so a settings file round-trips without gratuitous changes.

Set-StrictMode -Version 2.0

function ConvertTo-CuwJsonString([string]$s) {
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.Append('"')
    foreach ($ch in $s.ToCharArray()) {
        $code = [int]$ch
        switch ($code) {
            0x22 { [void]$sb.Append('\"'); continue }
            0x5C { [void]$sb.Append('\\'); continue }
            0x08 { [void]$sb.Append('\b'); continue }
            0x0C { [void]$sb.Append('\f'); continue }
            0x0A { [void]$sb.Append('\n'); continue }
            0x0D { [void]$sb.Append('\r'); continue }
            0x09 { [void]$sb.Append('\t'); continue }
            default {
                if ($code -lt 0x20) { [void]$sb.Append(('\u{0:x4}' -f $code)) }
                else { [void]$sb.Append($ch) }
            }
        }
    }
    [void]$sb.Append('"')
    return $sb.ToString()
}

function ConvertTo-CuwJson {
    param($Value, [int]$Level = 0)
    $inv = [System.Globalization.CultureInfo]::InvariantCulture
    $pad = '  ' * ($Level + 1)
    $end = '  ' * $Level
    if ($null -eq $Value) { return 'null' }
    if ($Value -is [bool]) { if ($Value) { return 'true' } else { return 'false' } }
    if ($Value -is [string] -or $Value -is [char]) { return (ConvertTo-CuwJsonString ([string]$Value)) }
    if ($Value -is [double] -or $Value -is [single]) {
        if ([double]::IsNaN($Value) -or [double]::IsInfinity($Value)) { return 'null' }
        return ([double]$Value).ToString('R', $inv)
    }
    if ($Value -is [int] -or $Value -is [long] -or $Value -is [decimal] -or $Value -is [int16] -or
        $Value -is [byte] -or $Value -is [uint32] -or $Value -is [uint64] -or $Value -is [sbyte] -or $Value -is [uint16]) {
        return $Value.ToString($inv)
    }
    if ($Value -is [System.Collections.IDictionary]) {
        $keys = @($Value.Keys)
        if ($keys.Count -eq 0) { return '{}' }
        $parts = foreach ($k in $keys) { $pad + (ConvertTo-CuwJsonString ([string]$k)) + ': ' + (ConvertTo-CuwJson $Value[$k] ($Level + 1)) }
        return "{`n" + ($parts -join ",`n") + "`n$end}"
    }
    if ($Value -is [System.Collections.IList]) {
        if ($Value.Count -eq 0) { return '[]' }
        $parts = foreach ($item in $Value) { $pad + (ConvertTo-CuwJson $item ($Level + 1)) }
        return "[`n" + ($parts -join ",`n") + "`n$end]"
    }
    if ($Value -is [System.Management.Automation.PSCustomObject]) {
        $props = @($Value.PSObject.Properties)
        if ($props.Count -eq 0) { return '{}' }
        $parts = foreach ($p in $props) { $pad + (ConvertTo-CuwJsonString $p.Name) + ': ' + (ConvertTo-CuwJson $p.Value ($Level + 1)) }
        return "{`n" + ($parts -join ",`n") + "`n$end}"
    }
    return (ConvertTo-CuwJsonString ([string]$Value))
}

# Parse JSON text. Empty/whitespace text is an empty object. Throws on invalid JSON.
function Read-CuwJsonText([string]$Text) {
    if ($Text.Length -gt 0 -and [int]$Text[0] -eq 0xFEFF) { $Text = $Text.Substring(1) }
    if ([string]::IsNullOrWhiteSpace($Text)) { return (New-Object PSObject) }
    $obj = ConvertFrom-Json -InputObject $Text -ErrorAction Stop
    if (-not ($obj -is [System.Management.Automation.PSCustomObject])) {
        throw 'Settings file does not contain a JSON object.'
    }
    return $obj
}

function Read-CuwJsonFile([string]$Path) {
    $text = [System.IO.File]::ReadAllText($Path, (New-Object System.Text.UTF8Encoding($false)))
    return (Read-CuwJsonText $text)
}

# Write UTF-8 without BOM. The complete bytes are built first, written to a temp file next to the
# destination, then moved over it: the destination is only replaced once the new file exists.
function Write-CuwFileAtomic([string]$Path, [string]$Text) {
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($Text)
    $dir = Split-Path -Parent $Path
    if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $tmp = "$Path.cuw-tmp-$PID"
    [System.IO.File]::WriteAllBytes($tmp, $bytes)
    if (Test-Path -LiteralPath $Path) {
        [System.IO.File]::Replace($tmp, $Path, [NullString]::Value)
    } else {
        [System.IO.File]::Move($tmp, $Path)
    }
}

function Get-CuwProperty($Object, [string]$Name) {
    if ($null -eq $Object) { return $null }
    $p = $Object.PSObject.Properties[$Name]
    if ($null -eq $p) { return $null }
    return $p.Value
}

function Test-CuwProperty($Object, [string]$Name) {
    return ($null -ne $Object -and $null -ne $Object.PSObject.Properties[$Name])
}

# Set a property, keeping its position if it already exists (so key order is preserved).
function Set-CuwProperty($Object, [string]$Name, $Value) {
    if (Test-CuwProperty $Object $Name) { $Object.PSObject.Properties[$Name].Value = $Value }
    else { $Object | Add-Member -NotePropertyName $Name -NotePropertyValue $Value }
}

function Get-CuwDefaultDataDir { return (Join-Path $env:APPDATA 'ClaudeUsageWidget') }
function Get-CuwDefaultSettingsPath { return (Join-Path $env:USERPROFILE '.claude\settings.json') }

Export-ModuleMember -Function ConvertTo-CuwJson, Read-CuwJsonText, Read-CuwJsonFile, Write-CuwFileAtomic,
    Get-CuwProperty, Test-CuwProperty, Set-CuwProperty, Get-CuwDefaultDataDir, Get-CuwDefaultSettingsPath
