use crate::accept::settings::saved;
use crate::support::app::native_message;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::settings_window;
use crate::support::app::show_page;
use crate::support::automation::Automation;
use crate::support::automation::DialogButton;
use crate::support::automation::wait;
use crate::support::capture::Capture;
use circadia_app::ConfigInput;
use circadia_app::ConfigStore;
use circadia_app::Coordinates;
use circadia_app::DiagnosticCategory;
use circadia_app::DiagnosticFailure;
use circadia_app::GraphMeasurement;
use circadia_app::LocationMeasurement;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::ffi::OsString;
use std::io;
use std::path::Path;
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
    crate::support::app::require_english()?;
    if let [scenario, directory] = arguments {
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
        return crate::measure::graph::desktop_check_at(
            &measurement,
            Path::new(directory),
            move |measurement, directory| exercise(measurement, directory, &scenario),
        );
    }
    require(
        arguments.is_empty(),
        "usage: circadia-harness accept location",
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
        let status = crate::support::run::inherit(&mut std::process::Command::new(
            std::env::current_exe()?,
        ))?
        .args(["accept", "location", scenario])
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
            let settings = crate::support::app::wait("foreground startup", || {
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
            crate::support::app::wait_until(
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
    let settings = crate::support::app::wait("location settings", || settings_window(process))?;
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
            position_requests: 1,
        },
    )?;
    finding(automation, &settings)?;
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
    crate::support::app::wait("location consent action", || {
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
    let process = std::process::id();
    calls(
        measurement,
        LocationMeasurement {
            permission_checks: 1,
            position_requests: 0,
        },
    )?;
    require(
        settings_window(process)?.is_none(),
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
            position_requests: 0,
        },
    )?;
    location_failure(automation, &settings)?;
    require(
        automation
            .text(&settings, process, automation_id::LOCATION_REQUESTING)?
            .is_none(),
        "denied request showed Finding your location",
    )?;
    require(
        automation
            .text(
                &settings,
                process,
                &automation_id::link(automation_id::LOCATION),
            )?
            .as_deref()
            == Some("Open Windows location settings"),
        "denied request failure has no Windows location settings link",
    )?;
    snapshot(
        automation,
        &settings,
        &automation_id::link(automation_id::LOCATION),
        "location-denied.png",
        "Follow sunrise and sunset card with the Windows permission failure and its link",
    )?;
    show_page(automation, &settings, process, automation_id::NAV_GENERAL)?;
    automation.toggle(&settings, process, automation_id::START_MINIMIZED)?;
    wait("foreground startup preference saved", || {
        Ok((!saved(directory)?.start_minimized).then_some(()))
    })?;
    report(
        "PASS: denied permission fails at tray startup and again when Settings opens, without a position request or Finding your location; the failure stays visible with its Windows location settings link",
    );
    Ok(())
}

fn finding(automation: &Automation, settings: &HWND) -> io::Result<()> {
    let expected = "Finding your location…";
    let process = std::process::id();
    wait(&format!("location request shows {expected}"), || {
        let caption = automation
            .names(settings, process)?
            .iter()
            .any(|name| name == expected);
        // WinUI prefixes an active progress ring's accessible name with "Busy".
        let indicator = automation
            .text(settings, process, automation_id::LOCATION_REQUESTING)?
            .is_some_and(|name| name.ends_with(expected));
        Ok((caption && indicator).then_some(()))
    })?;
    snapshot(
        automation,
        settings,
        automation_id::LATITUDE,
        "location-finding.png",
        "Use my Windows location row while Windows finds the position",
    )
}

fn snapshot(
    automation: &Automation,
    settings: &HWND,
    shown: &str,
    image: &str,
    description: &str,
) -> io::Result<()> {
    let process = std::process::id();
    wait(&format!("{shown} visible"), || {
        Ok(automation
            .show_control(settings, process, shown)?
            .then_some(()))
    })?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    let path = crate::support::run::image(image, description)?;
    Capture::window(settings, process)?.snapshot(&path)?;
    report(&format!("CAPTURE: {}", path.display()));
    Ok(())
}

fn calls(measurement: &GraphMeasurement, expected: LocationMeasurement) -> io::Result<()> {
    wait("expected location calls", || {
        let actual = measurement.location_calls()?;
        require(
            actual.permission_checks <= expected.permission_checks
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
        automation.text(
            settings,
            process,
            &automation_id::message(automation_id::LOCATION),
        )
    })?;
    Ok(())
}

fn close(settings: &HWND) -> io::Result<()> {
    native_message(settings, Message::Close)?;
    wait("Settings closes", || {
        Ok(settings_window(std::process::id())?.is_none().then_some(()))
    })
}

fn open() -> io::Result<HWND> {
    match circadia_app::InstanceClaim::acquire()? {
        circadia_app::InstanceClaim::Existing(instance) => instance.activate()?,
        circadia_app::InstanceClaim::Primary(_) => {
            return Err(io::Error::other("running instance missing"));
        }
    }
    crate::support::app::wait("Settings opens", || settings_window(std::process::id()))
}
