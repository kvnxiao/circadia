use crate::automation::Automation;
use crate::automation::DialogButton;
use crate::automation::wait;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use crate::settings::saved;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::Coordinates;
use circadia::DiagnosticCategory;
use circadia::DiagnosticFailure;
use circadia::GraphMeasurement;
use circadia::LocationMeasurement;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use windows_sys::Win32::winuser::WM_CLOSE;
use winsafe::HWND;

const DENIED: DiagnosticFailure = DiagnosticFailure {
    category: DiagnosticCategory::AccessDenied,
    code: None,
};

const POSITION: Coordinates = Coordinates {
    latitude: 12.0,
    longitude: -45.0,
};

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    crate::lifecycle::require_english()?;
    if let [_, scenario, directory] = arguments {
        let scenario = scenario
            .to_str()
            .ok_or_else(|| io::Error::other("invalid location scenario"))?
            .to_owned();
        let measurement = GraphMeasurement::default();
        if scenario != "real" {
            measurement.simulate_location(if scenario == "denied" {
                Err(DENIED)
            } else {
                Ok(())
            })?;
        }
        return crate::graph::desktop_check_at(
            &measurement,
            Path::new(directory),
            move |measurement, directory| exercise(measurement, directory, &scenario),
        );
    }
    require(
        arguments.len() == 1,
        "usage: native-lifecycle --native-location",
    )?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-location-")
        .tempdir()?;
    let input = ConfigInput {
        start_minimized: false,
        coordinates: Some(POSITION),
        ..ConfigInput::default()
    };
    ConfigStore::new(directory.path())
        .save(&input.validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    let mut scenarios = vec!["consent", "tray", "denied", "foreground"];
    let _apartment =
        winsafe::CoInitializeEx(winsafe::co::COINIT::MULTITHREADED).map_err(io::Error::other)?;
    let permission = windows::Security::Authorization::AppCapabilityAccess::AppCapability::Create(
        windows::core::h!("location"),
    )
    .and_then(|capability| capability.CheckAccess());
    match permission {
        Ok(windows::Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus::Allowed) => scenarios.push("real"),
        Ok(status) => report(&format!("Windows location host check pending: silent permission status {status:?}; privacy settings were not changed")),
        Err(error) => report(&format!("Windows location host check pending: silent permission check failed ({:?})", error.code())),
    }
    let result = scenarios.into_iter().try_for_each(|scenario| {
        if scenario == "real" {
            let mut input = saved(directory.path())?;
            input.start_minimized = true;
            ConfigStore::new(directory.path())
                .save(&input.validate().map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
        }
        let status = std::process::Command::new(std::env::current_exe()?)
            .args(["--native-location", scenario])
            .arg(directory.path())
            .status()?;
        require(
            status.success(),
            &format!("location scenario {scenario} failed"),
        )
    });
    if result.is_err() {
        report(&format!(
            "Failed location fixture retained: {}",
            directory.keep().display()
        ));
    }
    result
}

fn exercise(measurement: &GraphMeasurement, directory: &Path, scenario: &str) -> io::Result<()> {
    let automation = Automation::new()?;
    match scenario {
        "consent" => consent(measurement, &automation, directory),
        "tray" => tray(measurement, &automation, directory),
        "denied" => denied(measurement, &automation, directory),
        "foreground" => {
            let settings = crate::lifecycle::wait("foreground startup", || {
                settings_window(std::process::id())
            })?;
            show_page(
                &automation,
                &settings,
                std::process::id(),
                automation_id::NAV_SCHEDULE,
            )?;
            calls(
                measurement,
                LocationMeasurement {
                    permission_checks: 1,
                    consent_requests: 0,
                    position_requests: 1,
                },
            )?;
            measurement.complete_location(Ok(POSITION))?;
            require(
                saved(directory)?.use_windows_location,
                "foreground startup lost saved location preference",
            )?;
            report(
                "PASS: saved foreground startup requests once with solar timing off and without repeating Circadia consent",
            );
            Ok(())
        }
        "real" => {
            require(
                settings_window(std::process::id())?.is_none(),
                "real tray startup opened Settings",
            )?;
            crate::lifecycle::wait_until(
                "silent Windows position saved",
                std::time::Instant::now() + std::time::Duration::from_secs(90),
                || {
                    Ok(saved(directory)?
                        .coordinates
                        .filter(|position| *position != POSITION))
                },
            )?;
            require(
                settings_window(std::process::id())?.is_none(),
                "real acquisition opened Settings",
            )?;
            report(
                "PASS: existing Windows permission allowed a real tray-only acquisition; coordinates were saved without printing them",
            );
            Ok(())
        }
        _ => Err(io::Error::other("unknown location scenario")),
    }
}

fn consent(
    measurement: &GraphMeasurement,
    automation: &Automation,
    directory: &Path,
) -> io::Result<()> {
    let process = std::process::id();
    let settings = crate::lifecycle::wait("location settings", || settings_window(process))?;
    show_page(automation, &settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(&settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar expander enabled", || {
        Ok(automation
            .enabled(&settings, process, automation_id::SOLAR_EXPAND)?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(&settings, process, automation_id::SOLAR_EXPAND)?;
    wait("location toggle", || {
        Ok(automation
            .enabled(&settings, process, automation_id::USE_WINDOWS_LOCATION)?
            .filter(|enabled| *enabled))
    })?;
    for button in [DialogButton::Close, DialogButton::Primary] {
        choose_location(measurement, automation, &settings, directory, button)?;
    }
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 1,
            consent_requests: 0,
            position_requests: 1,
        },
    )?;
    require(
        saved(directory)?.use_windows_location,
        "confirmed location preference not saved",
    )?;
    automation.toggle(&settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing off with location enabled", || {
        Ok((!saved(directory)?.solar_timing).then_some(()))
    })?;
    close(&settings)?;
    let changed = Coordinates {
        latitude: 47.6,
        longitude: -122.3,
    };
    measurement.complete_location(Ok(changed))?;
    wait("closed-window location saved", || {
        Ok((saved(directory)?.coordinates == Some(changed)).then_some(()))
    })?;
    let settings = open()?;
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 2,
            consent_requests: 0,
            position_requests: 2,
        },
    )?;
    measurement.complete_location(Err(DENIED))?;
    location_failure(automation, &settings)?;
    show_page(automation, &settings, process, automation_id::NAV_GENERAL)?;
    automation.toggle(&settings, process, automation_id::START_MINIMIZED)?;
    wait("tray startup preference saved", || {
        Ok(saved(directory)?.start_minimized.then_some(()))
    })?;
    report(
        "PASS: Cancel does not request or save; confirmation persists; completion after Settings closes saves coordinates; reopening shows a failed request",
    );
    Ok(())
}

fn choose_location(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    directory: &Path,
    button: DialogButton,
) -> io::Result<()> {
    let process = std::process::id();
    wait("location toggle ready", || {
        Ok(
            (automation.toggled(settings, process, automation_id::USE_WINDOWS_LOCATION)?
                == Some(false)
                && automation.enabled(settings, process, automation_id::USE_WINDOWS_LOCATION)?
                    == Some(true))
            .then_some(()),
        )
    })?;
    automation.toggle(settings, process, automation_id::USE_WINDOWS_LOCATION)?;
    wait("location consent dialog", || {
        automation.dialog_button_enabled(settings, process, button)
    })?;
    require(
        !saved(directory)?.use_windows_location,
        "location saved before confirmation",
    )?;
    require(
        measurement.location_calls()? == LocationMeasurement::default(),
        "location requested before confirmation",
    )?;
    crate::lifecycle::wait("location consent action", || {
        Ok(automation
            .dialog_button(settings, process, button)?
            .then_some(()))
    })?;
    wait("location consent closes", || {
        Ok(automation
            .dialog_button_enabled(settings, process, button)?
            .is_none()
            .then_some(()))
    })?;
    if matches!(button, DialogButton::Close) {
        require(
            !saved(directory)?.use_windows_location,
            "Cancel saved location enablement",
        )?;
        require(
            measurement.location_calls()? == LocationMeasurement::default(),
            "Cancel requested Windows location",
        )?;
    }
    Ok(())
}

fn tray(
    measurement: &GraphMeasurement,
    automation: &Automation,
    directory: &Path,
) -> io::Result<()> {
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 1,
            consent_requests: 0,
            position_requests: 1,
        },
    )?;
    require(
        settings_window(std::process::id())?.is_none(),
        "tray startup opened Settings",
    )?;
    let settings = open()?;
    show_page(
        automation,
        &settings,
        std::process::id(),
        automation_id::NAV_GENERAL,
    )?;
    show_page(
        automation,
        &settings,
        std::process::id(),
        automation_id::NAV_SCHEDULE,
    )?;
    require(
        measurement.location_calls()?
            == LocationMeasurement {
                permission_checks: 1,
                consent_requests: 0,
                position_requests: 1,
            },
        "opening Settings duplicated the pending startup request",
    )?;
    close(&settings)?;
    measurement.complete_location(Ok(POSITION))?;
    wait("tray-only position saved", || {
        Ok((saved(directory)?.coordinates == Some(POSITION)).then_some(()))
    })?;
    require(
        saved(directory)?.use_windows_location,
        "tray startup lost saved location preference",
    )?;
    report(
        "PASS: saved tray startup requests silently; opening Settings coalesces the pending request; completion with only the tray icon saves coordinates",
    );
    Ok(())
}

fn denied(
    measurement: &GraphMeasurement,
    automation: &Automation,
    directory: &Path,
) -> io::Result<()> {
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 1,
            consent_requests: 0,
            position_requests: 0,
        },
    )?;
    require(
        settings_window(std::process::id())?.is_none(),
        "denied tray startup opened Settings",
    )?;
    require(
        saved(directory)?.use_windows_location && saved(directory)?.coordinates == Some(POSITION),
        "denied startup changed saved preference or coordinates",
    )?;
    let settings = open()?;
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 2,
            consent_requests: 1,
            position_requests: 0,
        },
    )?;
    location_failure(automation, &settings)?;
    measurement.complete_location_access(Ok(()))?;
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 2,
            consent_requests: 1,
            position_requests: 1,
        },
    )?;
    location_failure(automation, &settings)?;
    measurement.complete_location(Ok(POSITION))?;
    wait("successful request clears failure", || {
        Ok(automation
            .text(&settings, std::process::id(), "location-message")?
            .is_none()
            .then_some(()))
    })?;
    show_page(
        automation,
        &settings,
        std::process::id(),
        automation_id::NAV_GENERAL,
    )?;
    automation.toggle(
        &settings,
        std::process::id(),
        automation_id::START_MINIMIZED,
    )?;
    wait("foreground startup preference saved", || {
        Ok((!saved(directory)?.start_minimized).then_some(()))
    })?;
    report(
        "PASS: denied tray startup never requests consent; Settings keeps the failure visible during foreground permission and acquisition; success clears it",
    );
    Ok(())
}

fn calls(measurement: &GraphMeasurement, expected: LocationMeasurement) -> io::Result<()> {
    wait("expected location calls", || {
        let actual = measurement.location_calls()?;
        require(
            actual.permission_checks <= expected.permission_checks
                && actual.consent_requests <= expected.consent_requests
                && actual.position_requests <= expected.position_requests,
            &format!("extra location calls: {actual:?}, expected {expected:?}"),
        )?;
        Ok((actual == expected).then_some(()))
    })
}

fn location_failure(automation: &Automation, settings: &HWND) -> io::Result<()> {
    let process = std::process::id();
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    if automation
        .enabled(settings, process, automation_id::USE_WINDOWS_LOCATION)?
        .is_none()
    {
        automation.invoke(settings, process, automation_id::SOLAR_EXPAND)?;
    }
    wait("location failure", || {
        automation.text(settings, process, "location-message")
    })?;
    Ok(())
}

fn close(settings: &HWND) -> io::Result<()> {
    native_message(settings, WM_CLOSE, 0)?;
    wait("Settings closes", || {
        Ok(settings_window(std::process::id())?.is_none().then_some(()))
    })
}

fn open() -> io::Result<HWND> {
    match circadia::InstanceClaim::acquire()? {
        circadia::InstanceClaim::Existing(instance) => instance.activate()?,
        circadia::InstanceClaim::Primary(_) => {
            return Err(io::Error::other("running instance missing"));
        }
    }
    crate::lifecycle::wait("Settings opens", || settings_window(std::process::id()))
}
