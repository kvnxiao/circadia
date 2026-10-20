param([Parameter(Mandatory=$true)][string]$Reader)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Get-SettingsWindows {
    foreach ($process in @(Get-Process -Name SystemSettings -ErrorAction SilentlyContinue)) {
        $element = [Windows.Automation.AutomationElement]::RootElement.FindFirst(
            [Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id))
        if ($element) { $process }
    }
}
if (@(Get-SettingsWindows).Count) {
    [ordered]@{coverage_complete=$false;supported=$false;reason='Windows Settings was already open; native checks were skipped'} | ConvertTo-Json -Compress
    exit 0
}
function Read-NightLight {
    $text = & $Reader
    if ($LASTEXITCODE -ne 0) { throw 'Night Light inspection failed' }
    return $text | ConvertFrom-Json
}
function Get-OwnedWindow {
    return [Windows.Automation.AutomationElement]::RootElement.FindFirst(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$script:settingsProcessId))
}
function Find-Control([string]$NamePattern, [Windows.Automation.AutomationPattern]$RequiredPattern, [Windows.Automation.ControlType]$RequiredType = $null) {
    $window = Get-OwnedWindow
    if (-not $window) { return $null }
    $elements = $window.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
    foreach ($element in $elements) {
        if ($element.Current.Name -notmatch $NamePattern) { continue }
        if ($RequiredType -and $element.Current.ControlType -ne $RequiredType) { continue }
        $candidatePattern = $null
        if ($element.TryGetCurrentPattern($RequiredPattern,[ref]$candidatePattern)) { return $element }
    }
    return $null
}
function Wait-Control([string]$NamePattern, [Windows.Automation.AutomationPattern]$RequiredPattern) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        $element = Find-Control $NamePattern $RequiredPattern
        if ($element) { return $element }
        Start-Sleep -Milliseconds 100
    } while ($timer.Elapsed.TotalSeconds -lt 5)
    return $null
}
function Wait-Readback([scriptblock]$Predicate) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 100
        $observed = Read-NightLight
        if (& $Predicate $observed) { return $observed }
    } while ($timer.Elapsed.TotalSeconds -lt 5)
    throw ('Native control readback did not match within five seconds: ' + ($observed | ConvertTo-Json -Compress))
}
$script:settingsProcessId = 0
$records = [Collections.Generic.List[object]]::new()
try {
    Start-Process 'ms-settings:nightlight' -WindowStyle Hidden
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 200
        $processes = @(Get-SettingsWindows)
        if ($processes.Count -gt 1) { throw 'Cannot distinguish the Settings process opened for this probe' }
        if ($processes.Count -eq 1) { $script:settingsProcessId = $processes[0].Id }
        $slider = if ($script:settingsProcessId) { Find-Control '^Strength$' ([Windows.Automation.RangeValuePattern]::Pattern) ([Windows.Automation.ControlType]::Slider) } else { $null }
    } while (-not $slider -and $timer.Elapsed.TotalSeconds -lt 15)
    if (-not $slider) { throw 'Native Strength slider unavailable' }
    $before = Read-NightLight
    $range = $slider.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    $target = if ($before.kelvin -eq 6500) { 30 } else { 0 }
    if ($target -lt $range.Current.Minimum -or $target -gt $range.Current.Maximum) { throw 'Native slider range does not include the test strength' }
    $range.SetValue($target)
    $observed = Wait-Readback { param($value) $value.kelvin -ne $before.kelvin -and -not $value.preview }
    $records.Add([ordered]@{control='Strength';requestedStrength=$target;before=$before;after=$observed})
    foreach ($desired in @($false,$true)) {
        $button = Wait-Control '^Turn (on|off) now$' ([Windows.Automation.InvokePattern]::Pattern)
        $pattern = $null
        if (-not $button -or -not $button.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) {
            $records.Add([ordered]@{control='Night Light activation';supported=$false;reason='No matching actionable control within five seconds'})
            break
        }
        $before = Read-NightLight
        if ($before.active -ne $desired) { $pattern.Invoke() }
        $observed = Wait-Readback { param($value) $value.active -eq $desired }
        $records.Add([ordered]@{control='Night Light activation';requested=$desired;before=$before;after=$observed})
    }
    foreach ($desired in @($true,$false)) {
        $toggle = Wait-Control '^Schedule night light$' ([Windows.Automation.TogglePattern]::Pattern)
        $pattern = $null
        if (-not $toggle -or -not $toggle.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern,[ref]$pattern)) {
            $records.Add([ordered]@{control='Native schedule';supported=$false;reason='No matching actionable control within five seconds'})
            break
        }
        $before = Read-NightLight
        if ($before.schedule_enabled -ne $desired) { $pattern.Toggle() }
        $observed = Wait-Readback { param($value) $value.schedule_enabled -eq $desired }
        $records.Add([ordered]@{control='Native schedule';requested=$desired;before=$before;after=$observed})
    }
    $complete = $records.Count -eq 5 -and @($records | Where-Object { $_.supported -eq $false }).Count -eq 0
    [ordered]@{coverage_complete=$complete;checks=$records;utc=[DateTime]::UtcNow.ToString('O')} | ConvertTo-Json -Depth 8 -Compress
} finally {
    if ($script:settingsProcessId) {
        $ancestor = Get-OwnedWindow
        $pattern = $null
        while ($ancestor -and -not $ancestor.TryGetCurrentPattern([Windows.Automation.WindowPattern]::Pattern,[ref]$pattern)) {
            $ancestor = [Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($ancestor)
        }
        if ($ancestor) {
            $pattern.Close()
            $timer = [Diagnostics.Stopwatch]::StartNew()
            while (Get-OwnedWindow) {
                if ($timer.Elapsed.TotalSeconds -ge 5) { throw 'The probe Settings window did not close' }
                Start-Sleep -Milliseconds 100
            }
        }
    }
}
