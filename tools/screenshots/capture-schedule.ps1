param([string]$Output = 'assets/screenshots/schedule.png')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$outputPath = [IO.Path]::GetFullPath($Output, $repository)
if ([IO.Path]::GetExtension($outputPath) -ne '.png') {
    throw 'The screenshot output must have a .png extension.'
}
if (Get-Process circadia -ErrorAction SilentlyContinue) {
    throw 'Quit Circadia before capturing the README screenshot.'
}

Add-Type -AssemblyName System.Drawing, System.Windows.Forms, UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ScreenshotWindow {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect bounds);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr window, int attribute, out Rect bounds, int size);
}
'@
if ([ScreenshotWindow]::SetThreadDpiAwarenessContext([IntPtr](-4)) -eq [IntPtr]::Zero) {
    throw 'Cannot enable per-monitor DPI awareness for the capture thread.'
}

function Wait-For([string]$Description, [scriptblock]$Read) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $value = & $Read
        if ($value) { return $value }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out waiting for $Description."
}

function Control([string]$Id) {
    $root.FindFirst(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Id))
}

function Select-Page([string]$Id, [string]$ReadyId) {
    $control = Wait-For $Id { Control $Id }
    $control.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
    Wait-For $ReadyId { Control $ReadyId } | Out-Null
}

function Ready-To-Capture {
    $application.Refresh()
    if ($application.HasExited) { throw 'Screenshot instance exited before capture completed.' }
    $manage = Control 'manage'
    $preview = Control 'preview'
    if (-not $manage -or -not $preview -or -not $manage.Current.IsEnabled -or -not $preview.Current.IsEnabled) {
        return $false
    }
    $toggle = $manage.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    if ($toggle.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::On) { return $false }
    foreach ($id in 'manage-change', 'banner-configuration-error', 'banner-paused', 'banner-write-failed', 'banner-restored', 'banner-unknown-keys', 'banner-backup-failed', 'banner-crash', 'banner-schedule', 'banner-native', 'banner-invalid-fields') {
        $banner = Control $id
        if ($banner -and -not $banner.Current.IsOffscreen) { return $false }
    }
    return $true
}

function Visible-Bounds {
    $bounds = [ScreenshotWindow+Rect]::new()
    $result = [ScreenshotWindow]::DwmGetWindowAttribute($window, 9, [ref]$bounds, 16)
    if ($result -ne 0) { throw "Cannot read visible window bounds: $result" }
    return $bounds
}

$oldChannel = $env:CIRCADIA_CHANNEL
Push-Location $repository
try {
    $env:CIRCADIA_CHANNEL = 'preview'
    & just build-self-contained
    if ($LASTEXITCODE -ne 0) { throw 'Preview build failed.' }
} finally {
    $env:CIRCADIA_CHANNEL = $oldChannel
    Pop-Location
}

$directory = Join-Path $repository ('target/readme-capture/' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory -Force | Out-Null
@'
schema_version = 3
wake = "05:00"
bedtime = "23:00"
daytime_kelvin = 6500
middle_kelvin = 3400
sleep_kelvin = 2400
scheduled_transition_minutes = 60
ordinary_fade_ms = 250
focus_delay_ms = 250
output_rate_hz = 60
preview_seconds = 10
solar_timing = true
sunrise = true
sunset = true
use_windows_location = false
suppression = []
exclusions = []
start_at_sign_in = false
start_minimized = false
check_updates_automatically = false
manage_night_light = true
interface_language = "en-US"

[coordinates]
latitude = 40.7128
longitude = -74.0060
'@ | Set-Content -LiteralPath (Join-Path $directory 'config.toml') -Encoding utf8

$application = $null
$backdrop = $null
$root = $null
$pointer = [System.Windows.Forms.Cursor]::Position
$capturePath = Join-Path $directory 'schedule.png'
try {
    if (Get-Process circadia -ErrorAction SilentlyContinue) {
        throw 'Circadia started during the build. Quit it and run the capture again.'
    }
    $application = Start-Process -FilePath (Join-Path $repository 'target/debug/circadia.exe') -ArgumentList @('--data-dir', ('"' + $directory + '"')) -PassThru
    $window = Wait-For 'Circadia settings window' {
        $application.Refresh()
        if ($application.HasExited) { throw 'Screenshot instance exited before opening settings.' }
        if ($application.MainWindowHandle -ne [IntPtr]::Zero) { $application.MainWindowHandle }
    }
    if ([ScreenshotWindow]::GetDpiForWindow($window) -ne 96) {
        throw 'README screenshots require 100% display scaling.'
    }
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($window)
    Select-Page 'nav-about' 'about-version'
    $version = (Control 'about-version').Current.Name
    if ($version -notmatch '-preview') { throw "Expected a preview build, got: $version" }
    Select-Page 'nav-schedule' 'manage'
    Wait-For 'ready Schedule page with active management' { Ready-To-Capture } | Out-Null
    foreach ($id in 'summary-sunrise-time', 'summary-sunset-time') {
        Wait-For "$id solar time" {
            $control = Control $id
            $control -and $control.Current.Name -match '\d'
        } | Out-Null
    }

    $width = 1280
    $height = 800
    $work = [System.Windows.Forms.Screen]::FromHandle($window).WorkingArea
    if ($work.Width -lt $width + 40 -or $work.Height -lt $height + 40) {
        throw 'The desktop work area must fit a 1280x800 screenshot plus 40 pixels of clearance.'
    }
    $x = $work.Left + [int](($work.Width - $width) / 2)
    $y = $work.Top + [int](($work.Height - $height) / 2)
    $backdrop = [System.Windows.Forms.Form]::new()
    $backdrop.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
    $backdrop.ShowInTaskbar = $false
    $backdrop.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
    $backdrop.BackColor = [System.Drawing.Color]::FromArgb(32, 33, 39)
    $backdrop.Bounds = [System.Drawing.Rectangle]::new($x - 20, $y - 20, $width + 40, $height + 40)
    $backdrop.TopMost = $true
    $backdrop.Show()
    [System.Windows.Forms.Application]::DoEvents()
    if (-not [ScreenshotWindow]::SetWindowPos($window, [IntPtr](-1), $x, $y, $width, $height, 0x0040)) {
        throw 'Cannot position the screenshot window.'
    }
    $visible = Visible-Bounds
    $outer = [ScreenshotWindow+Rect]::new()
    if (-not [ScreenshotWindow]::GetWindowRect($window, [ref]$outer)) { throw 'Cannot read window size.' }
    $outerWidth = $width + ($outer.Right - $outer.Left) - ($visible.Right - $visible.Left)
    $outerHeight = $height + ($outer.Bottom - $outer.Top) - ($visible.Bottom - $visible.Top)
    if (-not [ScreenshotWindow]::SetWindowPos($window, [IntPtr](-1), $x, $y, $outerWidth, $outerHeight, 0x0040)) {
        throw 'Cannot size the screenshot window.'
    }
    Start-Sleep -Milliseconds 500
    $visible = Visible-Bounds
    if ($visible.Right - $visible.Left -ne $width -or $visible.Bottom - $visible.Top -ne $height) {
        throw 'The visible window is not 1280x800.'
    }
    if (-not [ScreenshotWindow]::SetCursorPos($visible.Right - 50, $visible.Top + 80)) {
        throw 'Cannot position the pointer inside Circadia; no click was sent.'
    }
    [ScreenshotWindow]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    [ScreenshotWindow]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    if (-not [ScreenshotWindow]::SetCursorPos($work.Right - 5, $work.Bottom - 5)) {
        throw 'Cannot move the pointer outside the capture.'
    }
    Start-Sleep -Milliseconds 1000
    Wait-For 'capture-ready Schedule page' { Ready-To-Capture } | Out-Null
    $bitmap = [System.Drawing.Bitmap]::new($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($visible.Left, $visible.Top, 0, 0, $bitmap.Size)
        $bitmap.Save($capturePath, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    Write-Host "Captured $version at 1280x800."
} finally {
    if ($backdrop) { $backdrop.Dispose() }
    [ScreenshotWindow]::SetCursorPos($pointer.X, $pointer.Y) | Out-Null
    if ($application) {
        try {
            $application.Refresh()
            if ($application.HasExited) { throw 'Screenshot instance exited before Quit was requested.' }
            if (-not $root) { throw 'No settings window for graceful cleanup.' }
            Select-Page 'nav-general' 'about-quit'
            (Control 'about-quit').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            if (-not $application.WaitForExit(15000)) { throw 'Circadia did not quit within 15 seconds.' }
            if ($application.ExitCode -ne 0) { throw "Circadia exited with code $($application.ExitCode)." }
        } catch {
            if (-not $application.HasExited) { $application.Kill() }
            throw "Screenshot cleanup failed; Night light may still be active. Logs: $directory. $_"
        }
    }
    Write-Host "Capture configuration and logs: $directory"
}
New-Item -ItemType Directory -Path (Split-Path $outputPath) -Force | Out-Null
Copy-Item -LiteralPath $capturePath -Destination $outputPath -Force
Write-Host "Saved $outputPath. Inspect the image for clipping and focus rings before committing it."
