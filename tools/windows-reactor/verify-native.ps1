param([string]$Executable = "$PSScriptRoot\target\debug\circadia-reactor-lifecycle-probe.exe")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$probe = Start-Process -FilePath $Executable -ArgumentList '--open' -WindowStyle Hidden -RedirectStandardError "$PSScriptRoot/target/native-stderr.log" -PassThru
try {
    $root = [Windows.Automation.AutomationElement]::RootElement
    function Window([string]$Name) {
        $all = $root.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $probe.Id))
        foreach ($item in $all) { if ($item.Current.Name -eq $Name -and $item.Current.ControlType -eq [Windows.Automation.ControlType]::Window) { return $item } }
        return $null
    }
    function Control($Window, [string]$Id) {
        $item = $Window.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty,$Id))
        if (-not $item) { throw "Missing control $Id" }
        return $item
    }
    function Invoke($Window, [string]$Id) {
        (Control $Window $Id).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    }
    function Wait-Window([string]$Name) {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        do { $item = Window $Name; if ($item) { return $item }; Start-Sleep -Milliseconds 100 } while ($timer.Elapsed.TotalSeconds -lt 10)
        $available = $root.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty, $probe.Id)) | Where-Object { $_.Current.ControlType -eq [Windows.Automation.ControlType]::Window } | ForEach-Object { $_.Current.Name }
        $state = (Control $settings 'handle-status').Current.Name
        throw "Window did not appear: $Name; available: $available; status: $state"
    }
    function Status([string]$Prefix) {
        $timer = [Diagnostics.Stopwatch]::StartNew()
        do { try { $current = Window 'Circadia development lifecycle probe'; $value = (Control $current 'handle-status').Current.Name; if ($value.StartsWith($Prefix)) { return $value } } catch { $value = $_.Exception.Message }; Start-Sleep -Milliseconds 100 } while ($timer.Elapsed.TotalSeconds -lt 15)
        throw "Status did not match $Prefix; got $value"
    }
    $settings = Wait-Window 'Circadia development lifecycle probe'
    Start-Sleep -Milliseconds 300
    (Control $settings 'foreground').SetFocus()
    Invoke $settings 'foreground'
    $foreground = Status 'Foreground:'
    if (-not $foreground.EndsWith('protected PID 4 access denied: true')) { throw $foreground }
    Invoke $settings 'picker'
    $dialog = Wait-Window 'Circadia executable selection'
    Start-Sleep -Milliseconds 800
    $dialog.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
    $cancel = Status 'Picker cancelled'
    Invoke $settings 'picker'
    $dialog = Wait-Window 'Circadia executable selection'
    Start-Sleep -Milliseconds 800
    $dialog.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).SetWindowVisualState([Windows.Automation.WindowVisualState]::Normal)
    $shell = New-Object -ComObject WScript.Shell
    if (-not $shell.AppActivate('Circadia executable selection')) { throw 'Cannot activate native dialog' }
    $shell.SendKeys('{ENTER}')
    $selection = Status 'Selected executable: circadia-reactor-lifecycle-probe.exe'
    Invoke $settings 'graph'
    $graph = Wait-Window 'Circadia pinned Reactor graph fixture'
    (Control $graph 'kelvin-input').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('3200')
    Start-Sleep -Milliseconds 300
    Invoke $graph 'apply-values'
    Start-Sleep -Milliseconds 300
    $curve = (Control $graph 'probe-state').Current.Name
    if (-not $curve.Contains('3200')) { throw "Graph edit failed: $curve" }
    $graph.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
    Invoke $settings 'increment'
    Start-Sleep -Milliseconds 200
    $counter = (Control $settings 'counter').Current.Name
    if ($counter -ne 'Application counter: 1') { throw $counter }
    [ordered]@{foreground=$foreground;cancel=$cancel;selection=$selection;graph=$curve;counter=$counter} | ConvertTo-Json
    Invoke $settings 'exit'
    if (-not $probe.WaitForExit(5000)) { throw 'Probe did not exit' }
} finally {
    if (-not $probe.HasExited) { $probe.Kill(); $probe.WaitForExit() }
    $probe.Dispose()
}
