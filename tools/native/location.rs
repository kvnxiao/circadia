use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::settings::saved;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use winsafe::HWND;

pub(super) fn exercise(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let original = saved(directory)?;
    require(
        automation.enabled(settings, process, "open-diagnostics")? == Some(true),
        "diagnostics-folder action is unavailable",
    )?;
    automation.set_value(settings, process, "latitude", "12.5")?;
    automation.set_value(settings, process, "longitude", "-45.25")?;
    automation.focus(settings, process)?;
    wait("foreground settings before location permission", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(settings)).then_some(()))
    })?;
    report("Requesting one Windows location; coordinates are not printed or saved");
    automation.invoke(settings, process, "current-location")?;
    let started = Instant::now();
    let result = wait("current location request begins", || {
        let pending = automation.enabled(settings, process, "current-location")? == Some(false);
        let error = automation.text(settings, process, "error-details")?;
        let changed = automation.value(settings, process, "latitude")?.as_deref() != Some("12.5")
            || automation.value(settings, process, "longitude")?.as_deref() != Some("-45.25");
        Ok((pending || error.is_some() || changed).then_some(()))
    })
    .and_then(|()| {
        wait_for_completion(started, || {
            automation.enabled(settings, process, "current-location")
        })
    });
    if result.is_err() {
        drop(automation.invoke(settings, process, "cancel-location"));
    }
    result?;
    let error = automation.text(settings, process, "error-details")?;
    if error.is_some() {
        require(
            automation.value(settings, process, "latitude")?.as_deref() == Some("12.5")
                && automation.value(settings, process, "longitude")?.as_deref() == Some("-45.25"),
            "failed location request replaced the existing coordinates",
        )?;
        report("PASS: Windows location failure is visible and preserves draft coordinates");
    } else {
        let latitude = automation
            .value(settings, process, "latitude")?
            .ok_or_else(|| io::Error::other("latitude field is absent"))?;
        let longitude = automation
            .value(settings, process, "longitude")?
            .ok_or_else(|| io::Error::other("longitude field is absent"))?;
        let latitude = latitude.parse::<f64>().map_err(io::Error::other)?;
        let longitude = longitude.parse::<f64>().map_err(io::Error::other)?;
        require(
            (-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude),
            "Windows supplied invalid coordinates",
        )?;
        report("PASS: one Windows location updates valid draft coordinates");
    }
    require(
        saved(directory)? == original,
        "location request persisted coordinates",
    )?;
    automation.set_value(settings, process, "latitude", "15")?;
    automation.set_value(settings, process, "longitude", "-20")?;
    wait("manual coordinates remain editable", || {
        Ok(
            (automation.value(settings, process, "latitude")?.as_deref() == Some("15")
                && automation.value(settings, process, "longitude")?.as_deref() == Some("-20"))
            .then_some(()),
        )
    })?;
    automation.invoke(settings, process, "discard")?;
    wait(
        "Discard removes unsaved acquired and manual coordinates",
        || {
            Ok(
                (automation.value(settings, process, "latitude")?.as_deref() == Some("")
                    && automation.value(settings, process, "longitude")?.as_deref() == Some(""))
                .then_some(()),
            )
        },
    )?;
    require(
        saved(directory)? == original,
        "Discard changed the saved location",
    )?;
    report(
        "PASS: location stays unsaved, manual entry remains available, and Discard restores the saved fields",
    );
    Ok(())
}

fn wait_for_completion(
    started: Instant,
    mut enabled: impl FnMut() -> io::Result<Option<bool>>,
) -> io::Result<()> {
    crate::automation::wait_until(
        "Windows location completion within 75 seconds",
        started + Duration::from_secs(75),
        || Ok(enabled()?.filter(|enabled| *enabled).map(|_| ())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;
    use windows::Win32::UI::Accessibility::UIA_E_ELEMENTNOTAVAILABLE;
    use windows::Win32::UI::Accessibility::UIA_E_TIMEOUT;

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    fn transient_completion_observation_does_not_cancel_location(code: u32) {
        let mut attempts = 0;
        let result = wait_for_completion(Instant::now(), || {
            attempts += 1;
            match attempts {
                1 => Err(io::Error::other(windows::core::Error::from_hresult(
                    windows::core::HRESULT(code.cast_signed()),
                ))),
                2 => Ok(Some(false)),
                _ => Ok(Some(true)),
            }
        });
        result.expect("transient observation must retry without cancelling location");
        assert_eq!(attempts, 3);
    }

    #[test_case(UIA_E_ELEMENTNOTAVAILABLE)]
    #[test_case(UIA_E_TIMEOUT)]
    fn observation_retry_keeps_the_original_location_deadline(code: u32) {
        let started = Instant::now()
            .checked_sub(Duration::from_secs(75))
            .expect("elapsed request start");
        let mut attempts = 0;
        let result = wait_for_completion(started, || {
            attempts += 1;
            Err(io::Error::other(windows::core::Error::from_hresult(
                windows::core::HRESULT(code.cast_signed()),
            )))
        });
        assert_eq!(
            result.expect_err("original deadline expired").kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(attempts, 1);
    }
}
