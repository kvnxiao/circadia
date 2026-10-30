use crate::automation::Automation;
use crate::automation::DialogButton;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::report;
use crate::lifecycle::require;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let (executable, activation_only) = match arguments {
        [_, executable] => (executable, false),
        [_, executable, option] if option == "--activation-only" => (executable, true),
        _ => {
            return Err(io::Error::other(
                "usage: native-lifecycle --identity CIRCADIA_EXE [--activation-only]",
            ));
        }
    };
    require(
        matches!(InstanceClaim::acquire()?, InstanceClaim::Primary(_)),
        "close Circadia before the identity check",
    )?;
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let automation = Automation::new()?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-identity-")
        .tempdir()?;
    crate::lifecycle::seed(
        directory.path(),
        &ConfigInput {
            start_minimized: false,
            ..ConfigInput::default()
        },
    )?;
    let mut application = Process::start(&fs_err::canonicalize(executable)?, directory.path())?;
    let process = application.id();
    let settings = wait("identity settings window", || {
        crate::lifecycle::settings_window(process)
    })?;
    crate::settings::editable(&automation, &settings, process)?;
    require(
        settings.GetDpiForWindow() == 96,
        "identity captures require 100% scaling",
    )?;
    let captures = tempfile::Builder::new()
        .prefix("identity-100-")
        .tempdir_in("target")?
        .keep();
    report(&format!("Identity screenshots: {}", captures.display()));
    crate::lifecycle::show_page(&automation, &settings, process, automation_id::NAV_APPS)?;
    let first = Fixture::new("identity-a-")?;
    let second = Fixture::new("identity-b-")?;
    arrange(&settings, &first.window, &second.window)?;
    activate(&second.window, second.process.id())?;
    observed(&automation, &settings, process, &second)?;
    activate(&first.window, first.process.id())?;
    observed(&automation, &settings, process, &first)?;
    activate(&settings, process)?;
    if activation_only {
        activate(&second.window, second.process.id())?;
        observed(&automation, &settings, process, &second)?;
        activate(&settings, process)?;
        crate::lifecycle::quit(&automation, &settings, process)?;
        application.wait_exit()?;
        report(
            "PASS: both visible fixture windows sustained foreground activation and entered recent history",
        );
        return Ok(());
    }
    crate::lifecycle::show_page(&automation, &settings, process, automation_id::NAV_APPS)?;
    let first_add = automation_id::recent_add(&first.name);
    let second_add = automation_id::recent_add(&second.name);
    order(&automation, &settings, process, &first_add, &second_add)?;
    let action = automation
        .find(&settings, process, &first_add)?
        .ok_or_else(|| io::Error::other("first app action is absent"))?;
    automation.focus_control(&settings, process, &first_add)?;
    activate(&second.window, second.process.id())?;
    order(&automation, &settings, process, &second_add, &first_add)?;
    return_to(&settings)?;
    require(
        automation.same_control(&settings, process, &first_add, &action)?,
        "recent app A lost its native action identity after history reordered",
    )?;
    focused(&automation, process, &first_add)?;
    crate::capture::Capture::window(&settings, process)?
        .snapshot(&captures.join("recent-after-reorder.png"))?;
    candidates(&automation, &settings, process, &first, &captures)?;
    automation.invoke(&settings, process, &first_add)?;
    wait("retained Add action writes app A", || {
        let input: ConfigInput = toml::from_str(&fs_err::read_to_string(
            ConfigStore::new(directory.path()).primary_path(),
        )?)
        .map_err(io::Error::other)?;
        Ok((input.suppression.len() == 1
            && input
                .suppression
                .first()
                .is_some_and(|entry| entry.executable == first.name))
        .then_some(()))
    })?;
    crate::lifecycle::quit(&automation, &settings, process)?;
    application.wait_exit()?;
    report(
        "PASS: recent actions and candidate checkboxes preserve native identity, focus and selection through history reorder; Add targets the retained executable; Night Light was not managed",
    );
    Ok(())
}

fn observed(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    fixture: &Fixture,
) -> io::Result<()> {
    let id = automation_id::recent_add(&fixture.name);
    wait("foreground fixture enters recent history", || {
        require_foreground(&fixture.window)?;
        automation.enabled(settings, process, &id)
    })
    .map(|_| ())
    .map_err(|error| {
        let controls = automation
            .controls_with_parents(settings, process)
            .map(|controls| {
                controls
                    .into_iter()
                    .map(|(control, _)| control.id)
                    .filter(|id| id.starts_with("recent-"))
                    .collect::<Vec<_>>()
            });
        io::Error::other(format!("{error}; expected {id}; actual {controls:?}"))
    })
}

fn candidates(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    first: &Fixture,
    captures: &Path,
) -> io::Result<()> {
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    let id = automation_id::dialog_choice(&first.name);
    wait("app A candidate checkbox", || {
        automation.enabled(settings, process, &id)
    })?;
    automation.toggle(settings, process, &id)?;
    automation.focus_control(settings, process, &id)?;

    let checkbox = automation
        .find(settings, process, &id)?
        .ok_or_else(|| io::Error::other("app A candidate is absent"))?;
    activate(&first.window, first.process.id())?;
    wait("app A candidate moves to the first position", || {
        let controls = automation.controls_with_parents(settings, process)?;
        Ok(controls
            .iter()
            .find(|(control, _)| control.id.starts_with("dialog-choice:"))
            .filter(|(control, _)| control.id == id)
            .map(|_| ()))
    })?;
    return_to(settings)?;
    require(
        automation.same_control(settings, process, &id, &checkbox)?,
        "app A candidate lost its native checkbox identity after history reordered",
    )?;
    focused(automation, process, &id)?;
    require(
        automation.toggled(settings, process, &id)? == Some(true),
        "app A candidate lost its checked state",
    )?;
    crate::capture::Capture::window(settings, process)?
        .snapshot(&captures.join("candidate-after-reorder.png"))?;
    require(
        automation.dialog_button(settings, process, DialogButton::Close)?,
        "candidate dialog Cancel is absent",
    )?;
    wait("candidate dialog closed", || {
        Ok(automation
            .enabled(settings, process, automation_id::DIALOG_BROWSE)?
            .is_none()
            .then_some(()))
    })?;
    Ok(())
}

fn order(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    first: &str,
    second: &str,
) -> io::Result<()> {
    let mut observed = Vec::new();
    let result = wait("recent apps reordered", || {
        let controls = automation.controls_with_parents(settings, process)?;
        observed = controls
            .iter()
            .filter(|(control, _)| control.id.starts_with("recent-add:"))
            .map(|(control, _)| control.id.clone())
            .collect();
        let first = controls.iter().position(|(control, _)| control.id == first);
        let second = controls
            .iter()
            .position(|(control, _)| control.id == second);
        Ok(first
            .zip(second)
            .filter(|(first, second)| first < second)
            .map(|_| ()))
    });
    result.map_err(|error| {
        io::Error::other(format!(
            "{error}; expected {first} before {second}; observed {observed:?}"
        ))
    })
}

fn focused(automation: &Automation, process: u32, id: &str) -> io::Result<()> {
    let current = automation.focused(process)?;
    require(
        current
            .as_ref()
            .is_some_and(|focus| focus.identifiers.iter().any(|current| current == id)),
        &format!("focus no longer identifies {id}: {current:?}"),
    )
}

fn arrange(settings: &HWND, first: &HWND, second: &HWND) -> io::Result<()> {
    let area = settings
        .MonitorFromWindow(winsafe::co::MONITOR::DEFAULTTONEAREST)
        .GetMonitorInfo()
        .map_err(io::Error::other)?
        .rcWork;
    let settings_bounds = settings.GetWindowRect().map_err(io::Error::other)?;
    let flags = winsafe::co::SWP::NOACTIVATE | winsafe::co::SWP::NOZORDER;
    settings
        .SetWindowPos(
            winsafe::HwndPlace::None,
            winsafe::POINT::with(area.left, area.top + 64),
            winsafe::SIZE::with(
                settings_bounds.right - settings_bounds.left,
                (settings_bounds.bottom - settings_bounds.top).min(area.bottom - area.top - 64),
            ),
            flags,
        )
        .map_err(io::Error::other)?;
    for (window, left) in [(first, area.left), (second, area.left + 400)] {
        window
            .SetWindowPos(
                winsafe::HwndPlace::None,
                winsafe::POINT::with(left, area.top),
                winsafe::SIZE::with(360, 180),
                flags,
            )
            .map_err(io::Error::other)?;
    }
    Ok(())
}

fn activate(window: &HWND, process: u32) -> io::Result<()> {
    require(
        window.GetWindowThreadProcessId().1 == process,
        "activation target changed owner",
    )?;
    if window.IsIconic() {
        window
            .ShowWindowAsync(winsafe::co::SW::RESTORE)
            .map_err(io::Error::other)?;
        wait("activation target restores", || {
            Ok((window.IsWindowVisible() && !window.IsIconic()).then_some(()))
        })?;
    }
    let bounds = window.GetWindowRect().map_err(io::Error::other)?;
    let point = winsafe::POINT::with(bounds.right - 200, bounds.top + 16);
    let mut at_point = None;
    wait("activation title-bar point is exposed", || {
        at_point = HWND::WindowFromPoint(point)
            .and_then(|window| window.GetAncestor(winsafe::co::GA::ROOT));
        Ok((at_point.as_ref() == Some(window)).then_some(()))
    })
    .map_err(|error| {
        io::Error::other(format!(
            "{error}; target={window:?}, point=({}, {}), actual={at_point:?}",
            point.x, point.y
        ))
    })?;
    crate::automation::click_point(point.x, point.y)?;
    wait("clicked window becomes foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(window)).then_some(()))
    })
    .map_err(|error| {
        io::Error::other(format!(
            "{error}; target={window:?}, actual={:?}",
            HWND::GetForegroundWindow()
        ))
    })?;
    let started = Instant::now();
    wait("clicked window sustains foreground activation", || {
        require_foreground(window)?;
        Ok((started.elapsed() >= Duration::from_millis(500)).then_some(()))
    })?;
    report(&format!(
        "Activated visible window {window:?}, PID {process}: {}",
        window.GetWindowText().map_err(io::Error::other)?
    ));
    Ok(())
}

fn require_foreground(window: &HWND) -> io::Result<()> {
    require(
        window.IsWindowVisible()
            && !window.IsIconic()
            && HWND::GetForegroundWindow().as_ref() == Some(window),
        &format!(
            "window is not visible, restored and foreground: target={window:?}, actual={:?}",
            HWND::GetForegroundWindow()
        ),
    )
}

fn return_to(window: &HWND) -> io::Result<()> {
    let inputs = [
        (winsafe::co::VK::MENU, winsafe::co::KEYEVENTF::NoValue),
        (winsafe::co::VK::TAB, winsafe::co::KEYEVENTF::NoValue),
        (winsafe::co::VK::TAB, winsafe::co::KEYEVENTF::KEYUP),
        (winsafe::co::VK::MENU, winsafe::co::KEYEVENTF::KEYUP),
    ]
    .map(|(key, flags)| {
        winsafe::HwKbMouse::Kb(winsafe::KEYBDINPUT {
            wVk: key,
            dwFlags: flags,
            ..winsafe::KEYBDINPUT::default()
        })
    });
    require(
        winsafe::SendInput(&inputs).map_err(io::Error::other)? == 4,
        "Alt+Tab input was incomplete",
    )?;
    wait("Alt+Tab restores Settings foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(window)).then_some(()))
    })?;
    require_foreground(window)
}

struct Fixture {
    process: Process,
    _path: tempfile::TempPath,
    window: HWND,
    name: String,
}

impl Fixture {
    fn new(prefix: &str) -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| io::Error::other("harness has no parent directory"))?;
        let path = tempfile::Builder::new()
            .prefix(prefix)
            .suffix(".exe")
            .tempfile_in(directory)?
            .into_temp_path();
        fs_err::copy(&executable, &path)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| io::Error::other("fixture name is not Unicode"))?
            .to_lowercase();
        let process = Process::foreground_fixture_at(&path)?;
        let window = wait("owned identity fixture window", || {
            crate::lifecycle::window(process.id(), "Circadia foreground fixture", true)
        })?;
        Ok(Self {
            process,
            _path: path,
            window,
            name,
        })
    }
}
