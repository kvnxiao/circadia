Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

function Get-NativeSettingsElement {
    $process = Get-Process -Name SystemSettings -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $process) { return $null }
    return [Windows.Automation.AutomationElement]::RootElement.FindFirst(
        [Windows.Automation.TreeScope]::Descendants,
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,$process.Id))
}

function Close-NativeSettings {
    $ancestor = Get-NativeSettingsElement
    if (-not $ancestor) { return }
    $pattern = $null
    while ($ancestor -and -not $ancestor.TryGetCurrentPattern([Windows.Automation.WindowPattern]::Pattern,[ref]$pattern)) {
        $ancestor = [Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($ancestor)
    }
    if (-not $ancestor) { throw 'Settings window close pattern unavailable' }
    $pattern.Close()
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (Get-NativeSettingsElement) {
        if ($timer.Elapsed.TotalSeconds -ge 5) { throw 'Settings window did not close' }
        Start-Sleep -Milliseconds 100
    }
}
