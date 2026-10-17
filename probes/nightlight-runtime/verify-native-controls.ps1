$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/native-settings.ps1"
$reader = "$PSScriptRoot/target/debug/circadia-nightlight-runtime-probe.exe"
function Read-NightLight {
    $result = & $reader | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Night Light read failed' }
    return $result
}
$baseline = Read-NightLight
if (-not $baseline.active -or $baseline.preview -or $baseline.schedule -ne 'Off') { throw 'This test requires active Night Light, preview off, and native schedule off' }
if ($null -eq $baseline.kelvin -or $baseline.kelvin -lt 1200 -or $baseline.kelvin -gt 6500) { throw 'Baseline temperature cannot be restored' }
$opened = $false
$changed = $false
try {
    Start-Process 'ms-settings:nightlight' -WindowStyle Hidden
    $opened = $true
    $root = [Windows.Automation.AutomationElement]::RootElement
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 300
        $process = Get-Process -Name SystemSettings -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $process) { continue }
        $window = $root.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id))
        if (-not $window) { continue }
        $slider = $window.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Slider))
    } while (-not $slider -and $timer.Elapsed.TotalSeconds -lt 15)
    if (-not $slider) { throw ('Native Night Light slider unavailable; automation root: ' + $window.Current.Name) }
    if ($slider.Current.Name -notlike '*Strength*') { throw ('Unexpected slider: ' + $slider.Current.Name) }
    $range = $slider.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern)
    $original = $range.Current.Value
    $target = if ($original -eq $range.Current.Minimum) { $range.Current.Maximum } else { $range.Current.Minimum }
    $changed = $true
    $range.SetValue($target)
    $timer.Restart()
    do { Start-Sleep -Milliseconds 100; $observed = Read-NightLight } while (($observed.kelvin -eq $baseline.kelvin -or $observed.preview) -and $timer.Elapsed.TotalSeconds -lt 5)
    if ($observed.kelvin -eq $baseline.kelvin -or $observed.preview) { throw 'Native slider did not complete a changed temperature' }
    [ordered]@{slider=$slider.Current.Name;oldStrength=$original;newStrength=$target;observed=$observed} | ConvertTo-Json -Depth 3
} finally {
    $cleanupErrors = @()
    if ($opened) {
        try { Close-NativeSettings }
        catch { $cleanupErrors += $_.Exception.Message }
    }
    if ($changed) {
        try {
            $restoration = & $reader --restore-temperature $baseline.kelvin | ConvertFrom-Json
            if (-not $restoration) { throw 'Restoration report missing' }
            if ($LASTEXITCODE -ne 0) { throw 'Temperature restoration failed' }
            Start-Sleep -Milliseconds 300
            $restored = Read-NightLight
            if ($restored.kelvin -ne $baseline.kelvin -or $restored.preview) { throw 'Restored temperature readback failed' }
        } catch { $cleanupErrors += $_.Exception.Message }
    }
    if ($cleanupErrors.Count) { throw ($cleanupErrors -join '; ') }
}
