param([string]$Executable = "$PSScriptRoot\target\debug\circadia-reactor-probe.exe")

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CircadiaPointerInput {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct Mouse {
        public int X, Y;
        public uint Data, Flags, Time;
        public UIntPtr Extra;
    }
    [StructLayout(LayoutKind.Explicit)] struct InputData {
        [FieldOffset(0)] public Mouse Mouse;
    }
    [StructLayout(LayoutKind.Sequential)] struct Input { public uint Type; public InputData Data; }
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, Input[] input, int size);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int key);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    static void Send(Input input) {
        if (SendInput(1, new[] { input }, Marshal.SizeOf(typeof(Input))) != 1)
            throw new InvalidOperationException("SendInput failed: " + Marshal.GetLastWin32Error());
    }
    public static void Move(double x, double y) {
        int left = GetSystemMetrics(76), top = GetSystemMetrics(77);
        int width = GetSystemMetrics(78), height = GetSystemMetrics(79);
        Send(new Input { Data = new InputData { Mouse = new Mouse {
            X = (int)Math.Round((x - left) * 65535.0 / (width - 1)),
            Y = (int)Math.Round((y - top) * 65535.0 / (height - 1)),
            Flags = 0xC001
        } } });
    }
    public static void Button(bool down) {
        Send(new Input { Data = new InputData { Mouse = new Mouse { Flags = down ? 2u : 4u } } });
    }
}
'@
foreach ($key in @(1, 2, 4, 16, 17, 18, 91, 92)) {
    if ([CircadiaPointerInput]::GetAsyncKeyState($key) -lt 0) {
        throw 'Release mouse buttons and modifier keys before running the pointer probe.'
    }
}
$oldDpi = [CircadiaPointerInput]::SetThreadDpiAwarenessContext([IntPtr](-4))
if ($oldDpi -eq [IntPtr]::Zero) { throw 'Cannot set per-monitor DPI awareness for the pointer test' }
$cursor = [CircadiaPointerInput+Point]::new()
if (-not [CircadiaPointerInput]::GetCursorPos([ref]$cursor)) { throw 'Cannot read the cursor position' }
$probe = $null
$mouseDown = $false
try {
    $probe = Start-Process -FilePath (Resolve-Path -LiteralPath $Executable).Path -PassThru -WindowStyle Hidden
    $condition = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $probe.Id)
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $window = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
            [System.Windows.Automation.TreeScope]::Children, $condition)
        if ($window) { break }
        if ($probe.HasExited) { throw "Probe exited before creating its window: $($probe.ExitCode)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $window) { throw 'Probe window did not appear' }
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        [void][CircadiaPointerInput]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle)
        Start-Sleep -Milliseconds 100
        $owner = [uint32]0
        [void][CircadiaPointerInput]::GetWindowThreadProcessId(
            [CircadiaPointerInput]::GetForegroundWindow(), [ref]$owner)
        if ($owner -eq $probe.Id) { break }
    } while ([DateTime]::UtcNow -lt $deadline)

    function Assert-ProbeForeground {
        $owner = [uint32]0
        [void][CircadiaPointerInput]::GetWindowThreadProcessId(
            [CircadiaPointerInput]::GetForegroundWindow(), [ref]$owner)
        if ($owner -ne $probe.Id) { throw 'Pointer test stopped because the probe lost foreground focus' }
    }
    function Get-Control([string]$Id) {
        $limit = [DateTime]::UtcNow.AddSeconds(5)
        do {
            $control = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
                [System.Windows.Automation.PropertyCondition]::new(
                    [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Id))
            if ($control) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        if (-not $control) { throw "Missing control: $Id" }
        return $control
    }
    function Move-Pointer([double]$X, [double]$Y) {
        Assert-ProbeForeground
        $bounds = (Get-Control 'plot-area').Current.BoundingRectangle
        Start-Sleep -Milliseconds 150
        $bounds = (Get-Control 'plot-area').Current.BoundingRectangle
        [CircadiaPointerInput]::Move($bounds.Left + $X * $bounds.Width / 640, $bounds.Top + $Y * $bounds.Height / 220)
        Start-Sleep -Milliseconds 120
        $after = (Get-Control 'plot-area').Current.BoundingRectangle
        Write-Verbose "Pointer ($X,$Y); graph before=$bounds after=$after"
    }
    function Set-MouseButton([bool]$Down) {
        Assert-ProbeForeground
        [CircadiaPointerInput]::Button($Down)
        $script:mouseDown = $Down
        Start-Sleep -Milliseconds 120
    }
    function Set-Anchor([string]$Id, [int]$Index) {
        Assert-ProbeForeground
        $control = Get-Control $Id
        $control.SetFocus()
        $expand = $control.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        $expand.Expand()
        $name = @('Sunrise', 'Sunset', 'Wake')[$Index]
        $itemCondition = [System.Windows.Automation.AndCondition]::new([System.Windows.Automation.Condition[]]@(
            $condition,
            [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem),
            [System.Windows.Automation.PropertyCondition]::new(
                [System.Windows.Automation.AutomationElement]::NameProperty, $name)))
        $limit = [DateTime]::UtcNow.AddSeconds(5)
        do {
            $item = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
                [System.Windows.Automation.TreeScope]::Descendants, $itemCondition)
            if ($item) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        if (-not $item) { throw "Missing anchor option: $name" }
        $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
        $expand.Collapse()
    }
    function Assert-State([int]$Count, [int]$Selected, [string]$Anchor, [double]$Offset,
        [double]$Kelvin, [string]$Order, [bool]$Dragging, [switch]$PointerTolerance) {
        $offsetTolerance = if ($PointerTolerance) { 2.5 } else { 0 }
        $kelvinTolerance = if ($PointerTolerance) { 25 } else { 0 }
        $limit = [DateTime]::UtcNow.AddSeconds(5)
        do {
            $text = (Get-Control 'probe-state').Current.Name
            $pattern = '^Count=(\d+); Selected=(\d+); Anchor=Some\((\w+)\); Offset=Some\(([^)]+)\); Kelvin=([^;]+); Order=([\d,]*); Dragging=(true|false)$'
            if ($text -match $pattern) {
                $actualOffset = [double]::Parse($Matches[4], [Globalization.CultureInfo]::InvariantCulture)
                $actualKelvin = [double]::Parse($Matches[5], [Globalization.CultureInfo]::InvariantCulture)
                if ([int]$Matches[1] -eq $Count -and [int]$Matches[2] -eq $Selected -and
                    $Matches[3] -eq $Anchor -and [Math]::Abs($actualOffset - $Offset) -le $offsetTolerance -and $actualOffset -eq [Math]::Truncate($actualOffset) -and
                    [Math]::Abs($actualKelvin - $Kelvin) -le $kelvinTolerance -and [Math]::Abs($actualKelvin % 10) -lt 0.000001 -and $Matches[6] -eq $Order -and
                    [bool]::Parse($Matches[7]) -eq $Dragging) { return }
            }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $limit)
        throw "Unexpected state: $text; expected count=$Count selected=$Selected anchor=$Anchor offset=$Offset kelvin=$Kelvin order=$Order dragging=$Dragging"
    }

    Assert-ProbeForeground
    $bounds = (Get-Control 'plot-area').Current.BoundingRectangle
    (Get-Control 'kelvin-input').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('3855')
    Start-Sleep -Milliseconds 150
    (Get-Control 'apply-values').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Assert-State 2 2 Sunset 30 3860 '1,2' $false
    (Get-Control 'kelvin-input').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('3400')
    Start-Sleep -Milliseconds 150
    (Get-Control 'apply-values').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Assert-State 2 2 Sunset 30 3400 '1,2' $false
    Set-Anchor 'point-anchor' 0
    Assert-State 2 2 Sunrise 30 3400 '2,1' $false
    Set-Anchor 'point-anchor' 1
    Assert-State 2 2 Sunset 30 3400 '1,2' $false

    Move-Pointer 503.3333333333 128.6792452830
    Set-MouseButton $true
    Assert-State 2 2 Sunset 30 3400 '1,2' $true
    Move-Pointer 505.3333333333 128.6792452830
    Assert-State 2 2 Sunset 30 3400 '1,2' $true
    Move-Pointer 330 110
    Assert-State 2 2 Sunset -360 3850 '1,2' $true -PointerTolerance
    Set-MouseButton $false
    Move-Pointer 400 80
    Assert-State 2 2 Sunset -360 3850 '1,2' $false -PointerTolerance

    Move-Pointer 400 66
    Set-MouseButton $true
    Assert-State 3 3 Sunset -180 4910 '1,2,3' $true -PointerTolerance
    Move-Pointer 480 88
    Assert-State 3 3 Sunset 0 4380 '1,2,3' $true -PointerTolerance
    Set-MouseButton $false
    Set-Anchor 'new-point-anchor' 0
    Assert-State 3 3 Sunset 0 4380 '1,2,3' $false -PointerTolerance

    Move-Pointer 494 102
    Set-MouseButton $true
    Assert-State 4 4 Sunrise 752 4040 '1,2,3,4' $true -PointerTolerance
    Move-Pointer -20 240
    Assert-State 4 4 Sunrise -360 1200 '4,1,2,3' $true -PointerTolerance
    Set-MouseButton $false
    Move-Pointer 500 100
    Assert-State 4 4 Sunrise -360 1200 '4,1,2,3' $false -PointerTolerance

    $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
    if (-not $probe.WaitForExit(5000)) { throw 'Probe did not exit after closing settings' }
    if ($probe.ExitCode -ne 0) { throw "Probe exited with code $($probe.ExitCode)" }
    [ordered]@{
        result = 'passed'
        processId = $probe.Id
        input = 'Win32 SendInput mouse; UI Automation selection and assertions'
        scaleX = $bounds.Width / 640
        scaleY = $bounds.Height / 220
        offsetToleranceMinutes = 2.5
        temperatureToleranceKelvin = 25
        checks = @('numeric Apply uses integer temperature steps', 'reanchor preserves offset and temperature', 'chronological reconnection',
            'off-center grab', 'movement threshold', 'click insertion and immediate drag',
            'separate new-point anchor', 'diagonal radius miss inserts', 'captured movement outside plot',
            'release outside plot stops drag', 'no movement after release', 'successful exit')
    } | ConvertTo-Json
} finally {
    if ($mouseDown) { [CircadiaPointerInput]::Button($false) }
    if ($probe) {
        if (-not $probe.HasExited) { $probe.Kill(); $probe.WaitForExit() }
        $probe.Dispose()
    }
    [void][CircadiaPointerInput]::SetCursorPos($cursor.X, $cursor.Y)
    [void][CircadiaPointerInput]::SetThreadDpiAwarenessContext($oldDpi)
}
