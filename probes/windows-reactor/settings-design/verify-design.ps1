param(
    [string]$Executable = "$PSScriptRoot\target\debug\circadia-settings-design-probe.exe",
    [string]$Results = "$PSScriptRoot\results",
    [ValidateSet('screens', 'pickers', 'renderers', 'localization', 'tray', 'icons')]
    [string[]]$Part = @('screens', 'pickers', 'renderers', 'localization', 'tray', 'icons')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class DesignProbeWindows {
    public delegate bool EnumCallback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumCallback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
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

# Per-monitor awareness keeps UI Automation rectangles and screen captures in the same pixels.
[DesignProbeWindows]::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null

$Automation = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]
$Isolation = "[$([char]0x2068)$([char]0x2069)]"
$shell = New-Object -ComObject WScript.Shell
$exe = (Resolve-Path -LiteralPath $Executable).Path
$log = [IO.Path]::ChangeExtension($exe, 'log')
New-Item -ItemType Directory -Force -Path $Results | Out-Null
if (Test-Path -LiteralPath $log) { Remove-Item -LiteralPath $log }
$report = [ordered]@{ executable = $exe; started = (Get-Date).ToString('o') }

function Clean([string]$Text) { return $Text -replace $Isolation, '' }

function New-Condition($Property, $Value) {
    return [System.Windows.Automation.PropertyCondition]::new($Property, $Value)
}

function Start-Probe([string[]]$Arguments) {
    return Start-Process -FilePath $exe -ArgumentList $Arguments -PassThru
}

function Get-ProcessWindows($Process) {
    return $Automation::RootElement.FindAll($Scope::Children, (New-Condition $Automation::ProcessIdProperty $Process.Id))
}

function Wait-Window($Process) {
    $limit = [DateTime]::UtcNow.AddSeconds(15)
    do {
        foreach ($candidate in Get-ProcessWindows $Process) {
            if ($candidate.Current.BoundingRectangle.Width -gt 500) { return $candidate }
        }
        if ($Process.HasExited) { throw "Probe exited: $($Process.ExitCode)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limit)
    throw 'Settings did not open within 15 seconds'
}

function Find-Id($Root, [string]$Id) {
    return $Root.FindFirst($Scope::Descendants, (New-Condition $Automation::AutomationIdProperty $Id))
}

# Flyouts and the tray menu can be hosted in their own top-level windows.
function Find-Anywhere($Process, [string]$Id) {
    foreach ($window in Get-ProcessWindows $Process) {
        $found = Find-Id $window $Id
        if ($found) { return $found }
    }
    return $null
}

function Get-Id($Root, [string]$Id) {
    $found = Find-Id $Root $Id
    if (-not $found) { throw "Missing control: $Id" }
    return $found
}

function Wait-Id($Process, [string]$Id, [int]$Milliseconds = 3000) {
    $limit = [DateTime]::UtcNow.AddMilliseconds($Milliseconds)
    do {
        $found = Find-Anywhere $Process $Id
        if ($found) { return $found }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limit)
    return $null
}

function Wait-Gone($Process, [string]$Id, [int]$Milliseconds = 2000) {
    $limit = [DateTime]::UtcNow.AddMilliseconds($Milliseconds)
    do {
        if (-not (Find-Anywhere $Process $Id)) { return $true }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limit)
    return $false
}

function Get-Pattern($Element, $Pattern) {
    $found = $null
    if ($Element.TryGetCurrentPattern($Pattern, [ref]$found)) { return $found }
    return $null
}

function Invoke-Element($Element) {
    $Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

function Select-Element($Element) {
    $selection = Get-Pattern $Element ([System.Windows.Automation.SelectionItemPattern]::Pattern)
    if ($selection) { $selection.Select() } else { Invoke-Element $Element }
}

function Focus-Window($Window) {
    [DesignProbeWindows]::SetForegroundWindow([IntPtr]$Window.Current.NativeWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 300
}

function Send-Keys($Window, [string]$Keys) {
    Focus-Window $Window
    $shell.SendKeys($Keys)
    Start-Sleep -Milliseconds 400
}

function Save-Screen($Element, [string]$Name) {
    $handle = [IntPtr]$Element.Current.NativeWindowHandle
    if ($handle -ne [IntPtr]::Zero) { [DesignProbeWindows]::SetForegroundWindow($handle) | Out-Null }
    Start-Sleep -Milliseconds 500
    $rect = $Element.Current.BoundingRectangle
    $bitmap = New-Object System.Drawing.Bitmap ([int][Math]::Ceiling($rect.Width)), ([int][Math]::Ceiling($rect.Height))
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int][Math]::Floor($rect.X), [int][Math]::Floor($rect.Y), 0, 0, $bitmap.Size)
        $bitmap.Save((Join-Path $Results "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    return "$Name.png"
}

function Read-Sample($Process, [string]$State) {
    $Process.Refresh()
    return [ordered]@{
        state = $State
        workingSetBytes = $Process.WorkingSet64
        privateBytes = $Process.PrivateMemorySize64
        cpuSeconds = $Process.TotalProcessorTime.TotalSeconds
        threads = $Process.Threads.Count
        handles = $Process.HandleCount
    }
}

function Wait-Tray($Process) {
    $limit = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $tray = [DesignProbeWindows]::TrayWindow($Process.Id)
        if ($tray -ne [IntPtr]::Zero) { return $tray }
        if ($Process.HasExited) { throw "Probe exited before creating its tray window: $($Process.ExitCode)" }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limit)
    throw 'Tray callback window did not appear'
}

function Send-TrayEvent([IntPtr]$Tray, [int]$Event, [int]$X = 0, [int]$Y = 0) {
    $position = [UIntPtr][uint32](($Y -band 0xFFFF) * 65536 + ($X -band 0xFFFF))
    if (-not [DesignProbeWindows]::PostMessage($Tray, 1025, $position, [IntPtr]$Event)) {
        throw "Posting tray event $Event failed"
    }
}

function Stop-Probe($Process) {
    try {
        if (-not $Process.HasExited) {
            $window = $null
            foreach ($candidate in Get-ProcessWindows $Process) {
                if ($candidate.Current.BoundingRectangle.Width -gt 500) { $window = $candidate }
            }
            if ($window) {
                $close = Find-Id $window 'CloseButton'
                if ($close) { Invoke-Element $close; Start-Sleep -Milliseconds 500 }
                Select-Element (Get-Id $window 'nav-general')
                $quit = Wait-Id $Process 'quit'
                if ($quit) { Invoke-Element $quit }
                $Process.WaitForExit(5000) | Out-Null
            }
        }
    } catch {
        Write-Warning "Graceful quit failed: $_"
    } finally {
        if (-not $Process.HasExited) { $Process.Kill(); $Process.WaitForExit() }
        $Process.Dispose()
    }
}

function Get-Scroller($Window) {
    $best = $null
    $all = $Window.FindAll($Scope::Descendants, (New-Condition $Automation::IsScrollPatternAvailableProperty $true))
    foreach ($candidate in $all) {
        $scroll = Get-Pattern $candidate ([System.Windows.Automation.ScrollPattern]::Pattern)
        if (-not $scroll -or -not $scroll.Current.VerticallyScrollable) { continue }
        if (-not $best -or $candidate.Current.BoundingRectangle.Width -gt $best.Current.BoundingRectangle.Width) {
            $best = $candidate
        }
    }
    return $best
}

function Save-Page($Window, [string]$Name) {
    $files = @(Save-Screen $Window "$Name-1")
    $scroller = Get-Scroller $Window
    if (-not $scroller) { return $files }
    $scroll = $scroller.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    $step = [Math]::Max(10, $scroll.Current.VerticalViewSize * 0.9)
    $index = 2
    for ($position = $step; $position -lt 100 + $step; $position += $step) {
        $scroll.SetScrollPercent(-1, [Math]::Min(100, $position))
        Start-Sleep -Milliseconds 600
        $files += Save-Screen $Window "$Name-$index"
        $index++
        if ($position -ge 100) { break }
    }
    $scroll.SetScrollPercent(-1, 0)
    return $files
}

function Capture([string]$Name, [string[]]$Arguments) {
    $process = Start-Probe (@('--open') + $Arguments)
    try {
        $window = Wait-Window $process
        Start-Sleep -Milliseconds 1500
        return Save-Page $window $Name
    } finally {
        Stop-Probe $process
    }
}

function Measure-TitleBar($Window) {
    $titleBar = Get-Id $Window 'title-bar'
    $system = $Window.FindFirst($Scope::Descendants, (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::TitleBar)))
    $buttons = @()
    if ($system) {
        foreach ($button in $system.FindAll($Scope::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
            $buttons += [ordered]@{ name = $button.Current.Name; id = $button.Current.AutomationId; height = $button.Current.BoundingRectangle.Height; width = $button.Current.BoundingRectangle.Width }
        }
    }
    $titleText = Find-Id $Window 'PART_TitleText'
    $titleCenter = $null
    if ($titleText) { $titleCenter = $titleText.Current.BoundingRectangle.Y + $titleText.Current.BoundingRectangle.Height / 2 - $Window.Current.BoundingRectangle.Y }
    $pageTitle = Find-Id $Window 'page-title'
    $systemHeight = $null
    if ($system) { $systemHeight = $system.Current.BoundingRectangle.Height }
    return [ordered]@{
        dpi = [DesignProbeWindows]::GetDpiForWindow([IntPtr]$Window.Current.NativeWindowHandle)
        reactorTitleBarHeight = $titleBar.Current.BoundingRectangle.Height
        titleTextCenterBelowWindowTop = $titleCenter
        pageTitleTopBelowWindowTop = $pageTitle.Current.BoundingRectangle.Y - $Window.Current.BoundingRectangle.Y
        systemTitleBarHeight = $systemHeight
        captionButtons = $buttons
    }
}

function Test-Screens {
    $screens = [ordered]@{}
    foreach ($theme in 'dark', 'light') {
        foreach ($page in 'schedule', 'apps', 'transitions', 'general', 'lab') {
            $screens["$theme-$page"] = Capture "$theme-$page" @('--theme', $theme, '--page', $page)
        }
        $states = [ordered]@{
            'steady-day' = @('--now', '13:00')
            'manual' = @('--status', 'manual')
            'automatic' = @('--status', 'automatic')
            'not-managing' = @('--status', 'not-managing')
            'preview' = @('--status', 'preview')
            'invalid' = @('--invalid')
            'banners' = @('--page', 'apps', '--banners', '--invalid')
            'consent' = @('--dialog', 'consent', '--status', 'not-managing')
            'add-app' = @('--page', 'apps', '--dialog', 'add')
        }
        foreach ($state in $states.Keys) {
            $process = Start-Probe (@('--open', '--theme', $theme) + $states[$state])
            try {
                $window = Wait-Window $process
                Start-Sleep -Milliseconds 1500
                $screens["$theme-$state"] = @(Save-Screen $window "$theme-$state")
                if ($state -eq 'steady-day') { $report["titleBar-$theme"] = Measure-TitleBar $window }
            } finally {
                Stop-Probe $process
            }
        }
    }
    $report.screens = $screens
}

function Get-ItemName($Item) {
    $name = Clean $Item.Current.Name
    if ($name) { return $name }
    $text = $Item.FindFirst($Scope::Descendants, (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::Text)))
    if ($text) { return '(text) ' + (Clean $text.Current.Name) }
    return ''
}

function Describe-Focus {
    $focus = $Automation::FocusedElement
    if (-not $focus) { return $null }
    return '{0} {1} id={2} name={3}' -f $focus.Current.ControlType.ProgrammaticName, $focus.Current.ClassName, $focus.Current.AutomationId, (Get-ItemName $focus)
}

function Test-Popup($Process) {
    foreach ($window in Get-ProcessWindows $Process) {
        if ($window.FindFirst($Scope::Descendants, (New-Condition $Automation::ClassNameProperty 'FlyoutPresenter'))) { return $true }
    }
    return $false
}

function Read-List($Process, [string]$Id) {
    $list = Find-Anywhere $Process $Id
    if (-not $list) { return $null }
    $selection = Get-Pattern $list ([System.Windows.Automation.SelectionPattern]::Pattern)
    $selected = if ($selection) { $selection.Current.GetSelection() | Select-Object -First 1 } else { $null }
    $listRect = $list.Current.BoundingRectangle
    $record = [ordered]@{ id = $Id; selected = $null; selectedVisible = $false; firstVisible = $null }
    if ($selected) {
        $rect = $selected.Current.BoundingRectangle
        $record.selected = Get-ItemName $selected
        $record.selectedVisible = (-not $rect.IsEmpty) -and $rect.Top -ge $listRect.Top - 1 -and $rect.Bottom -le $listRect.Bottom + 1
    }
    foreach ($item in $list.FindAll($Scope::Children, (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::ListItem)))) {
        $rect = $item.Current.BoundingRectangle
        if (-not $rect.IsEmpty -and $rect.Top -ge $listRect.Top - 1) { $record.firstVisible = Get-ItemName $item; break }
    }
    return $record
}

function Test-Picker($Process, $Window, $record) {
    $Id = $record.id
    $record.step = 'focus button'
    $button = Get-Id $Window $Id
    $button.SetFocus()
    Start-Sleep -Milliseconds 300
    $record.before = Clean $button.Current.Name
    $record.step = 'open'
    Invoke-Element $button
    $accept = Wait-Id $Process "$Id-accept"
    $record.opened = [bool]$accept
    if (-not $accept) { return }
    $record.popupWhenOpen = Test-Popup $Process
    $record.focusAfterOpen = Describe-Focus
    $record.step = 'read lists'
    $record.lists = @(foreach ($segment in 'hour', 'minute', 'period') { Read-List $Process "$Id-$segment" })
    $hour = Find-Anywhere $Process "$Id-hour"
    $selected = ($hour.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern)).Current.GetSelection() | Select-Object -First 1
    $next = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetNextSibling($selected)
    $record.step = 'select hour'
    if ($next) { Select-Element $next; Start-Sleep -Milliseconds 300 }
    $record.step = 'accept'
    Invoke-Element $accept
    $record.closedAfterAccept = Wait-Gone $Process "$Id-accept"
    $record.popupAfterAccept = Test-Popup $Process
    $record.focusAfterAccept = Describe-Focus
    if ($Id -in 'lab-detach', 'lab-replace', 'lab-replace-focus') { Save-Screen $Window "after-accept-$Id" | Out-Null }
    Start-Sleep -Milliseconds 300
    $record.after = Clean (Get-Id $Window $Id).Current.Name
    if (-not $record.closedAfterAccept) {
        Invoke-Element (Find-Anywhere $Process "$Id-cancel")
        $record.closedAfterCancel = Wait-Gone $Process "$Id-accept"
        if (-not $record.closedAfterCancel) {
            Send-Keys $Window '{ESC}'
            $record.closedAfterEscape = Wait-Gone $Process "$Id-accept"
        }
    }
    $record.step = 'reopen'
    Start-Sleep -Milliseconds 300
    Invoke-Element (Get-Id $Window $Id)
    $record.reopened = [bool](Wait-Id $Process "$Id-accept")
    if ($record.reopened) {
        Invoke-Element (Find-Anywhere $Process "$Id-cancel")
        $record.closedAfterReopenCancel = Wait-Gone $Process "$Id-accept"
        $record.popupAfterReopenCancel = Test-Popup $Process
        if (-not $record.closedAfterReopenCancel) {
            Send-Keys $Window '{ESC}'
            $record.closedAfterReopenEscape = Wait-Gone $Process "$Id-accept"
        }
    }
    $record.step = 'done'
}

function Test-PickerKeyboard($Process, $Window, [string]$Id) {
    $record = [ordered]@{ id = $Id }
    (Get-Id $Window $Id).SetFocus()
    Send-Keys $Window ' '
    $record.opened = [bool](Wait-Id $Process "$Id-accept")
    if (-not $record.opened) { return $record }
    $record.focusAfterOpen = Describe-Focus
    $record.hourBefore = (Read-List $Process "$Id-hour").selected
    Send-Keys $Window '{DOWN}'
    $record.focusAfterDown = Describe-Focus
    $record.hourAfterDown = (Read-List $Process "$Id-hour").selected
    Send-Keys $Window '{TAB}'
    $record.focusAfterTab = Describe-Focus
    Send-Keys $Window '{ESC}'
    $record.closedAfterEscape = Wait-Gone $Process "$Id-accept"
    return $record
}

function Test-Pickers {
    $pickers = @()
    foreach ($order in 'centered', 'natural') {
        foreach ($region in 'en-US', 'de-DE') {
            $process = Start-Probe @('--open', '--theme', 'dark', '--page', 'lab', '--order', $order, '--region', $region)
            try {
                $window = Wait-Window $process
                Start-Sleep -Milliseconds 1500
                foreach ($strategy in 'detach', 'replace', 'replace-focus', 'focus', 'disable', 'nothing') {
                    $record = [ordered]@{ id = "lab-$strategy" }
                    try {
                        Test-Picker $process $window $record
                    } catch {
                        $record.error = "$_ at $($_.InvocationInfo.ScriptLineNumber)"
                        Send-Keys $window '{ESC}'
                    }
                    $record.order = $order
                    $record.region = $region
                    $pickers += $record
                }
                try {
                    $keyboard = Test-PickerKeyboard $process $window 'lab-nothing'
                } catch {
                    $keyboard = [ordered]@{ id = 'lab-nothing'; keyboardError = "$_ at $($_.InvocationInfo.ScriptLineNumber)" }
                    Send-Keys $window '{ESC}'
                }
                $keyboard.order = $order
                $keyboard.region = $region
                $pickers += $keyboard
                (Get-Id $window 'lab-replace').SetFocus()
                Send-Keys $window ' '
                if (Wait-Id $process 'lab-replace-accept') {
                    Save-Screen $window "picker-$order-$region" | Out-Null
                    Send-Keys $window '{ESC}'
                }
            } finally {
                Stop-Probe $process
            }
        }
    }
    $report.pickers = $pickers
}

function Test-Renderers {
    $renderers = @()
    foreach ($renderer in 'canvas', 'composition', 'xaml') {
        $process = Start-Probe @('--renderer', $renderer, '--theme', 'dark')
        $record = [ordered]@{ renderer = $renderer; samples = @(); latency = @() }
        try {
            $tray = Wait-Tray $process
            Start-Sleep -Seconds 2
            $record.samples += Read-Sample $process 'tray only'
            Send-TrayEvent $tray 1024
            $window = Wait-Window $process
            Start-Sleep -Seconds 3
            $record.samples += Read-Sample $process 'schedule open'
            Start-Sleep -Seconds 10
            $record.samples += Read-Sample $process 'schedule after 10 s idle'
            Save-Screen $window "renderer-$renderer" | Out-Null
            $transform = $window.GetCurrentPattern([System.Windows.Automation.TransformPattern]::Pattern)
            $transform.Resize(900, 760)
            Start-Sleep -Milliseconds 1200
            Save-Screen $window "renderer-$renderer-narrow" | Out-Null
            $transform.Resize(1500, 1000)
            Start-Sleep -Milliseconds 1200
            Save-Screen $window "renderer-$renderer-wide" | Out-Null
            Select-Element (Get-Id $window 'nav-lab')
            Start-Sleep -Milliseconds 1500
            foreach ($step in 1..6) {
                $before = (Get-Id $window 'graph-latency').Current.Name
                Invoke-Element (Get-Id $window 'shift-later')
                $limit = [DateTime]::UtcNow.AddSeconds(2)
                do {
                    $text = (Get-Id $window 'graph-latency').Current.Name
                    if ($text -ne $before) { break }
                    Start-Sleep -Milliseconds 50
                } while ([DateTime]::UtcNow -lt $limit)
                $record.latency += $text
            }
            Invoke-Element (Get-Id $window 'toggle-theme')
            Start-Sleep -Milliseconds 1200
            Save-Screen $window "renderer-$renderer-toggled" | Out-Null
            Invoke-Element (Get-Id $window 'toggle-theme')
            $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern).Close()
            Start-Sleep -Seconds 3
            $record.samples += Read-Sample $process 'closed'
            Send-TrayEvent $tray 1024
            Wait-Window $process | Out-Null
            Start-Sleep -Milliseconds 1000
        } catch {
            $record.error = $_.ToString()
        } finally {
            Stop-Probe $process
        }
        $renderers += $record
    }
    $report.renderers = $renderers
}

function Read-BuiltIn($Window) {
    $toggle = Get-Id $Window 'lab-builtin-toggle'
    $texts = @(foreach ($text in $toggle.FindAll($Scope::Descendants, (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::Text)))) { $text.Current.Name })
    return [ordered]@{ name = $toggle.Current.Name; texts = $texts }
}

function Test-Localization {
    $localization = [ordered]@{}
    foreach ($variant in @(
            @{ name = 'english'; arguments = @() },
            @{ name = 'ui-language-de'; arguments = @('--ui-language', 'de-DE') },
            @{ name = 'german'; arguments = @('--language', 'de', '--region', 'de-DE') })) {
        $process = Start-Probe (@('--open', '--theme', 'dark', '--page', 'lab') + $variant.arguments)
        try {
            $window = Wait-Window $process
            Start-Sleep -Milliseconds 1500
            $localization[$variant.name] = [ordered]@{
                region = Clean (Get-Id $window 'lab-region').Current.Name
                pseudoRatio = (Get-Id $window 'lab-pseudo').Current.Name
                builtIn = Read-BuiltIn $window
            }
            Save-Screen $window "localization-$($variant.name)" | Out-Null
        } finally {
            Stop-Probe $process
        }
    }

    $process = Start-Probe @('--open', '--theme', 'dark', '--language', 'de', '--region', 'de-DE', '--invalid')
    try {
        $window = Wait-Window $process
        Start-Sleep -Milliseconds 1500
        $sentence = (Get-Id $window 'status-sentence').Current.Name
        $german = [ordered]@{
            orderError = Clean (Get-Id $window 'middle-error').Current.Name
            statusSentence = Clean $sentence
            isolationMarksInAutomationName = $sentence -match $Isolation
            files = Save-Page $window 'german-schedule'
        }
        $latitude = Get-Id $window 'latitude'
        $value = $latitude.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $german.latitudeShown = $value.Current.Value
        $value.SetValue('40.5')
        $german.periodRejected = [bool](Wait-Id $process 'latitude-error' 2000)
        $value.SetValue('40,5')
        $german.commaAccepted = Wait-Gone $process 'latitude-error'
        $localization.germanSchedule = $german
    } finally {
        Stop-Probe $process
    }

    $process = Start-Probe @('--open', '--theme', 'dark', '--page', 'apps')
    try {
        $window = Wait-Window $process
        Start-Sleep -Milliseconds 1500
        $plural = [ordered]@{ before = Clean (Get-Id $window 'list-heading').Current.Name }
        $remove = $window.FindFirst($Scope::Descendants, [System.Windows.Automation.AndCondition]::new(
                (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::Button)),
                (New-Condition $Automation::NameProperty "Remove $([char]0x2068)Counter-Strike 2$([char]0x2069)")))
        if ($remove) {
            Invoke-Element $remove
            Start-Sleep -Milliseconds 500
            $plural.after = Clean (Get-Id $window 'list-heading').Current.Name
        }
        $localization.plural = $plural
    } finally {
        Stop-Probe $process
    }

    foreach ($page in 'schedule', 'apps', 'general') {
        $localization["pseudo-$page"] = Capture "pseudo-$page" @('--theme', 'dark', '--language', 'qps-Ploc', '--page', $page)
    }
    $report.localization = $localization
}

function Read-Menu($Process) {
    $items = @()
    foreach ($window in Get-ProcessWindows $Process) {
        foreach ($item in $window.FindAll($Scope::Descendants, (New-Condition $Automation::ControlTypeProperty ([System.Windows.Automation.ControlType]::MenuItem)))) {
            $items += [ordered]@{ name = Clean $item.Current.Name; enabled = $item.Current.IsEnabled; host = $window.Current.ClassName }
        }
    }
    return $items
}

function Wait-Menu($Process) {
    $limit = [DateTime]::UtcNow.AddSeconds(3)
    do {
        $items = Read-Menu $Process
        if ($items.Count -gt 0) { return $items }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $limit)
    return @()
}

function Test-Tray {
    $process = Start-Probe @('--open', '--theme', 'dark')
    try {
        $tray = Wait-Tray $process
        $window = Wait-Window $process
        Start-Sleep -Milliseconds 1500
        $area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        $x = $area.Right - 80
        $y = $area.Bottom - 8
        Send-TrayEvent $tray 123 $x $y
        $menu = [ordered]@{ first = @(Wait-Menu $process) }
        $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bitmap = New-Object System.Drawing.Bitmap 480, 520
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen([Math]::Max(0, $screen.Right - 480), [Math]::Max(0, $area.Bottom - 520), 0, 0, $bitmap.Size)
            $bitmap.Save((Join-Path $Results 'tray-menu.png'), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
        $suppress = $null
        foreach ($menuWindow in Get-ProcessWindows $process) {
            $suppress = $menuWindow.FindFirst($Scope::Descendants, (New-Condition $Automation::NameProperty 'Suppress for 1 hour'))
            if ($suppress) { break }
        }
        if ($suppress) {
            Invoke-Element $suppress
            Start-Sleep -Milliseconds 800
            Send-TrayEvent $tray 123 $x $y
            $menu.afterSuppress = @(Wait-Menu $process)
            $shell.SendKeys('{ESC}')
            Start-Sleep -Milliseconds 500
        } else {
            $shell.SendKeys('{ESC}')
        }
        $menu.manualStatus = Clean (Get-Id $window 'status-sentence').Current.Name
        $report.tray = $menu
    } finally {
        Stop-Probe $process
    }
}

function Read-Png([string]$Path) {
    $stream = [IO.MemoryStream]::new([IO.File]::ReadAllBytes($Path))
    return [System.Drawing.Bitmap]::new($stream)
}

function Save-Rect([int]$X, [int]$Y, [int]$Width, [int]$Height, [string]$Name) {
    $bitmap = New-Object System.Drawing.Bitmap $Width, $Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($X, $Y, 0, 0, $bitmap.Size)
    } finally {
        $graphics.Dispose()
    }
    $bitmap.Save((Join-Path $Results "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    return $bitmap
}

# The lab draws each icon on opaque black, so the expected screen color is the
# PNG color multiplied by its alpha.
function Compare-Icon($Captured, $Expected) {
    $record = [ordered]@{ pixels = 0; exact = 0; withinOne = 0; maxDifference = 0; nonBlack = 0 }
    if ($Captured.Width -ne $Expected.Width -or $Captured.Height -ne $Expected.Height) {
        $record.sizeMismatch = "$($Captured.Width)x$($Captured.Height) against $($Expected.Width)x$($Expected.Height)"
        return $record
    }
    for ($y = 0; $y -lt $Expected.Height; $y++) {
        for ($x = 0; $x -lt $Expected.Width; $x++) {
            $source = $Expected.GetPixel($x, $y)
            $shown = $Captured.GetPixel($x, $y)
            $difference = 0
            foreach ($channel in 'R', 'G', 'B') {
                $wanted = [Math]::Round($source.$channel * $source.A / 255.0)
                $difference = [Math]::Max($difference, [Math]::Abs($shown.$channel - $wanted))
            }
            $record.pixels++
            if ($difference -eq 0) { $record.exact++ }
            if ($difference -le 1) { $record.withinOne++ }
            if ($shown.R + $shown.G + $shown.B -gt 0) { $record.nonBlack++ }
            $record.maxDifference = [Math]::Max($record.maxDifference, $difference)
        }
    }
    return $record
}

function Measure-Icons($Window, [string]$Label, $Expected) {
    $records = [ordered]@{}
    foreach ($id in 'lab-icon-data-32', 'lab-icon-file-32', 'lab-icon-data-64', 'lab-icon-file-64') {
        $element = Find-Id $Window $id
        if (-not $element) { $records[$id] = 'missing'; continue }
        $rect = $element.Current.BoundingRectangle
        $bitmap = Save-Rect ([int][Math]::Floor($rect.X)) ([int][Math]::Floor($rect.Y)) $Expected.Width $Expected.Height "icon-$Label-$id"
        try {
            $record = Compare-Icon $bitmap $Expected
        } finally {
            $bitmap.Dispose()
        }
        $record.bounds = "$($rect.X),$($rect.Y) $($rect.Width)x$($rect.Height)"
        $records[$id] = $record
    }
    return $records
}

function Test-Icons {
    $folder = Join-Path (Split-Path -Parent $exe) 'lab-icons'
    $process = Start-Probe @('--open', '--theme', 'dark', '--page', 'lab')
    try {
        $window = Wait-Window $process
        Start-Sleep -Milliseconds 2000
        Focus-Window $window
        $expected = Read-Png (Join-Path $folder 'notepad-32.png')
        $icons = [ordered]@{ dpi = [DesignProbeWindows]::GetDpiForWindow([IntPtr]$Window.Current.NativeWindowHandle) }
        try {
            $icons.initial = Measure-Icons $window 'initial' $expected
            foreach ($action in 'recreate', 'delete', 'swap') {
                Invoke-Element (Get-Id $window "lab-icon-$action")
                Start-Sleep -Milliseconds 1500
                Focus-Window $window
                $icons[$action] = Measure-Icons $window $action $expected
            }
        } finally {
            $expected.Dispose()
        }
        $report.icons = $icons
    } finally {
        Stop-Probe $process
    }
}

foreach ($name in $Part) {
    try {
        switch ($name) {
            'screens' { Test-Screens }
            'pickers' { Test-Pickers }
            'renderers' { Test-Renderers }
            'localization' { Test-Localization }
            'tray' { Test-Tray }
            'icons' { Test-Icons }
        }
    } catch {
        $report["$name-error"] = "$_ at $($_.InvocationInfo.ScriptLineNumber)"
    }
}
$report.finished = (Get-Date).ToString('o')
if (Test-Path -LiteralPath $log) { Copy-Item -LiteralPath $log -Destination (Join-Path $Results 'probe.log') }
$json = $report | ConvertTo-Json -Depth 8
Set-Content -LiteralPath (Join-Path $Results 'design-results.json') -Value $json -Encoding UTF8
$json
