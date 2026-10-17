param([Parameter(Mandatory=$true)][ValidateRange(1200,6500)][int]$Temperature)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/native-settings.ps1"
$reader = "$PSScriptRoot/target/debug/circadia-nightlight-runtime-probe.exe"
function Settings-Slider {
    $root = [Windows.Automation.AutomationElement]::RootElement
    $process = Get-Process -Name SystemSettings -ErrorAction Stop | Select-Object -First 1
    $element = $root.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id))
    if (-not $element) { throw 'Settings automation subtree unavailable' }
    $slider = $element.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.AndCondition]::new(
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Slider),
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Strength')))
    if (-not $slider) { throw 'Strength slider unavailable' }
    return $slider
}
$opened = $false
try {
    Start-Process 'ms-settings:nightlight' -WindowStyle Hidden
    $opened = $true
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 200
        try { $slider = Settings-Slider; break } catch { $slider = $null }
    } while ($timer.Elapsed.TotalSeconds -lt 15)
    if (-not $slider) { throw 'Settings did not open with its Strength slider' }
    $before = $slider.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern).Current.Value
    $result = & $reader --restore-temperature $Temperature | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Temperature restoration failed' }
    Start-Sleep -Milliseconds 300
    $whileOpen = $slider.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern).Current.Value
    Close-NativeSettings
    Start-Sleep -Milliseconds 500
    Start-Process 'ms-settings:nightlight' -WindowStyle Hidden
    $opened = $true
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 200
        try { $slider = Settings-Slider; break } catch { $slider = $null }
    } while ($timer.Elapsed.TotalSeconds -lt 15)
    if (-not $slider) { throw 'Settings did not reopen with its Strength slider' }
    $after = $slider.GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern).Current.Value
    $readback = & $reader | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $readback.kelvin -ne $Temperature -or $readback.preview) { throw 'Readback changed during settings reopening' }
    [ordered]@{requestedKelvin=$Temperature;strengthBefore=$before;strengthAfterWriteWhileOpen=$whileOpen;strengthAfterReopening=$after;readback=$readback} | ConvertTo-Json -Depth 3
} finally {
    if ($opened) { Close-NativeSettings }
}
