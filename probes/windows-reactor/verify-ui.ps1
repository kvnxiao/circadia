param([string]$Executable = "$PSScriptRoot\target\debug\circadia-reactor-lifecycle-probe.exe")

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$probePath = (Resolve-Path -LiteralPath $Executable).Path
$probe = Start-Process -FilePath $probePath -ArgumentList '--graph' -PassThru -WindowStyle Hidden
try {
    $condition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $probe.Id)
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $window = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
            [System.Windows.Automation.TreeScope]::Children, $condition)
        if ($window) { break }
        if ($probe.HasExited) { throw "Probe exited with code $($probe.ExitCode) before creating a window" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $window) { throw 'Probe window did not appear within 15 seconds' }

    function Get-ProbeControl([string]$Id) {
        $control = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Id))
        if (-not $control) { throw "Missing UI Automation control: $Id" }
        return $control
    }
    function Invoke-ProbeControl([string]$Id) {
        $control = Get-ProbeControl $Id
        $pattern = $control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        Start-Sleep -Milliseconds 150
    }
    function Wait-ProbeStatus([string]$Expected) {
        $limit = [DateTime]::UtcNow.AddSeconds(5)
        do {
            $actual = (Get-ProbeControl 'status').Current.Name
            if ($actual -eq $Expected) { return }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        throw "Expected status '$Expected'; received '$actual'"
    }

    Invoke-ProbeControl 'add-point'
    Invoke-ProbeControl 'validate'
    Wait-ProbeStatus 'Point fields are valid. This probe does not save or evaluate a schedule.'
    1..3 | ForEach-Object { Invoke-ProbeControl 'remove-point' }
    Invoke-ProbeControl 'validate'
    Wait-ProbeStatus 'Add at least one point.'
    Invoke-ProbeControl 'add-point'
    Invoke-ProbeControl 'validate'
    Wait-ProbeStatus 'Point fields are valid. This probe does not save or evaluate a schedule.'
    $probe.Refresh()
    $report = [ordered]@{
        processId = $probe.Id
        title = $window.Current.Name
        result = 'passed'
        checks = @('native window', 'UI Automation button invocation', 'add point', 'reject empty curve', 'restore valid curve')
        workingSetBytes = $probe.WorkingSet64
        privateBytes = $probe.PrivateMemorySize64
        cpuSeconds = $probe.TotalProcessorTime.TotalSeconds
    }
    $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
    if ($probe.WaitForExit(500)) { throw 'Tray process exited after closing the graph' }
    $report['lastWindowClose'] = 'tray process survives graph closure'
    $report | ConvertTo-Json
} finally {
    if (-not $probe.HasExited) { $probe.Kill(); $probe.WaitForExit() }
    $probe.Dispose()
}
