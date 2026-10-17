param([string]$Executable = "$PSScriptRoot\target\debug\circadia-reactor-lifecycle-probe.exe")

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class CircadiaProbeWindows {
    public delegate bool EnumCallback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumCallback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    public static IntPtr TrayWindow(uint process) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner != process) return true;
            var title = new StringBuilder(256);
            GetWindowText(window, title, title.Capacity);
            if (title.ToString() != "windows-notifyicon") return true;
            result = window;
            return false;
        }, IntPtr.Zero);
        return result;
    }
}
'@
$probe = Start-Process -FilePath (Resolve-Path -LiteralPath $Executable).Path -PassThru -WindowStyle Hidden
try {
    $pidCondition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $probe.Id)
    function Get-Settings {
        return [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
            [System.Windows.Automation.TreeScope]::Children, $pidCondition)
    }
    function Wait-Settings {
        $limit = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $found = Get-Settings
            if ($found) { return $found }
            if ($probe.HasExited) { throw "Probe exited: $($probe.ExitCode)" }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        throw 'Settings did not open within ten seconds'
    }
    function Get-Control([string]$Id) {
        $control = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Id))
        if (-not $control) { throw "Missing control: $Id" }
        return $control
    }
    function Invoke-Control([string]$Id) {
        (Get-Control $Id).GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    }
    function Wait-Text([string]$Id, [string]$Expected) {
        $limit = [DateTime]::UtcNow.AddSeconds(5)
        do {
            if ((Get-Control $Id).Current.Name -eq $Expected) { return }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        throw "Control $Id did not display '$Expected'"
    }
    function Read-Memory([string]$State) {
        $probe.Refresh()
        return [ordered]@{ state = $State; workingSetBytes = $probe.WorkingSet64; privateBytes = $probe.PrivateMemorySize64; cpuSeconds = $probe.TotalProcessorTime.TotalSeconds }
    }
    function Open-FromCallback {
        if (-not [CircadiaProbeWindows]::PostMessage($tray, 1025, [UIntPtr]::Zero, [IntPtr]1024)) {
            throw 'Posting the synthetic NIN_SELECT callback failed'
        }
    }

    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $tray = [CircadiaProbeWindows]::TrayWindow($probe.Id)
        if ($tray -ne [IntPtr]::Zero) { break }
        if ($probe.HasExited) { throw "Probe exited before creating its tray window: $($probe.ExitCode)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($tray -eq [IntPtr]::Zero) { throw 'Tray callback window did not appear' }
    Start-Sleep -Milliseconds 1000
    if (Get-Settings) { throw 'Settings opened during tray-only startup' }
    $samples = @(Read-Memory 'initially closed')
    Open-FromCallback
    $window = Wait-Settings
    Invoke-Control 'increment'
    Wait-Text 'counter' 'Application counter: 1'
    Invoke-Control 'inspect-handle'
    Wait-Text 'handle-status' 'Native window handle available: true'
    $samples += Read-Memory 'open'
    $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
    Start-Sleep -Milliseconds 1000
    if ($probe.HasExited) { throw 'Closing settings terminated the tray application' }
    if (Get-Settings) { throw 'Settings did not close' }
    $samples += Read-Memory 'closed again'
    Open-FromCallback
    $window = Wait-Settings
    Wait-Text 'counter' 'Application counter: 1'
    Open-FromCallback
    Start-Sleep -Milliseconds 200
    $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
        [System.Windows.Automation.TreeScope]::Children, $pidCondition)
    if ($windows.Count -ne 1) { throw "Repeated activation created $($windows.Count) windows" }
    Invoke-Control 'exit'
    if (-not $probe.WaitForExit(5000)) { throw 'Exit action did not terminate the application' }
    if ($probe.ExitCode -ne 0) { throw "Probe failed on exit: $($probe.ExitCode)" }
    [ordered]@{
        result = 'passed'
        processId = $probe.Id
        activation = 'synthetic notification callback; actual Shell mouse and keyboard input not tested'
        checks = @('tray-only startup', 'native handle access', 'close without process exit', 'counter survives reopen', 'one settings window', 'explicit exit')
        samples = $samples
    } | ConvertTo-Json -Depth 4
} finally {
    if (-not $probe.HasExited) { $probe.Kill(); $probe.WaitForExit() }
    $probe.Dispose()
}
