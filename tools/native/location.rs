use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::show_page;
use crate::settings::saved;
use circadia::Coordinates;
use circadia::Field;
use circadia::automation_id;
use std::io;
use std::path::Path;
use winsafe::HWND;

#[cfg(feature = "graph-measurement")]
#[path = "location/consent.rs"]
mod consent;
#[cfg(feature = "graph-measurement")]
pub(super) use consent::run;

pub(super) fn exercise(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    require(
        automation.toggled(settings, process, automation_id::USE_WINDOWS_LOCATION)? != Some(true),
        "Use my Windows location is on in the fixture",
    )?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    let message = automation_id::field_message(Field::Coordinates);
    wait("solar timing without coordinates asks for them", || {
        automation.text(settings, process, &message)
    })?;
    wait("coordinate fields are editable", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .filter(|enabled| *enabled))
    })?;
    require(
        !saved(directory)?.solar_timing,
        "solar timing without coordinates was written",
    )?;
    enter(automation, settings, process, automation_id::LATITUDE, "12")?;
    enter(
        automation,
        settings,
        process,
        automation_id::LONGITUDE,
        "-45",
    )?;
    automation.focus_control(settings, process, automation_id::LATITUDE)?;
    wait("entered coordinates are written with solar timing", || {
        let saved = saved(directory)?;
        Ok((saved.solar_timing
            && saved.coordinates
                == Some(Coordinates {
                    latitude: 12.0,
                    longitude: -45.0,
                }))
        .then_some(()))
    })?;
    wait("valid coordinates clear the message", || {
        Ok(automation
            .text(settings, process, &message)?
            .is_none()
            .then_some(()))
    })?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait(
        "solar timing off is written with the coordinates kept",
        || {
            let saved = saved(directory)?;
            Ok((!saved.solar_timing && saved.coordinates.is_some()).then_some(()))
        },
    )?;
    report(
        "PASS: solar timing asks for coordinates, typed latitude and longitude apply when the field loses focus, and both are written; Windows location was not requested",
    );
    Ok(())
}

fn enter(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    id: &str,
    value: &str,
) -> io::Result<()> {
    automation.focus_control(settings, process, id)?;
    automation.set_value(settings, process, id, value)?;
    wait("typed coordinate", || {
        Ok(automation
            .value(settings, process, id)?
            .filter(|text| text == value))
    })?;
    Ok(())
}
