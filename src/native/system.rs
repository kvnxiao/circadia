use super::NativeBackend;
use super::NativeFailure;
use super::NativeNotifications;
use super::NativeSettings;
use super::ObservationMode;
use super::TemperatureMode;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticOperation;
use std::error::Error;
use std::os::windows::io::AsHandle;
use std::os::windows::io::BorrowedHandle;
#[cfg(feature = "output-measurement")]
use std::time::Instant;
use win_nightlight_lib::ColorTemperature;
use win_nightlight_lib::ErrorKind;
use win_nightlight_lib::Nightlight;
use win_nightlight_watch::Snapshot;
use win_nightlight_watch::Watcher;

pub(super) struct SystemBackend {
    nightlight: Nightlight,
    settings: Option<Watcher>,
    state: Option<Watcher>,
}

impl SystemBackend {
    pub(super) fn new() -> Self {
        Self {
            nightlight: Nightlight::new().with_startup_margin(),
            settings: None,
            state: None,
        }
    }
}

impl NativeBackend for SystemBackend {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        let mut errors = Vec::new();
        for (slot, create) in [
            (&mut self.settings, Watcher::settings as fn() -> _),
            (&mut self.state, Watcher::state as fn() -> _),
        ] {
            if slot.is_some() {
                continue;
            }
            #[cfg(feature = "output-measurement")]
            let started = Instant::now();
            let result: Result<Watcher, win_nightlight_watch::Error> = create();
            #[cfg(feature = "output-measurement")]
            crate::measurement::call("register_watcher", None, started, result.is_ok());
            match result {
                Ok(watcher) => *slot = Some(watcher),
                Err(error) => {
                    errors.push(watch_failure(DiagnosticOperation::Observation, error));
                }
            }
        }
        errors
    }

    fn notifications(&mut self, mode: ObservationMode) -> NativeNotifications {
        let mut report = NativeNotifications::default();
        for (slot, operation) in [
            (&mut self.settings, DiagnosticOperation::NativeSettingsRead),
            (&mut self.state, DiagnosticOperation::NativeStateRead),
        ] {
            consume(slot, operation, mode, &mut report);
        }
        report
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        self.settings
            .iter()
            .chain(self.state.iter())
            .map(AsHandle::as_handle)
            .collect()
    }

    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = self.nightlight.read_settings();
        #[cfg(feature = "output-measurement")]
        crate::measurement::call("read_settings", None, started, result.is_ok());
        result
            .map(|settings| settings_values(&settings))
            .map_err(|error| access_failure(DiagnosticOperation::NativeSettingsRead, error))
    }

    fn active(&mut self) -> Result<bool, NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = self.nightlight.read_state();
        #[cfg(feature = "output-measurement")]
        crate::measurement::call("read_state", None, started, result.is_ok());
        result
            .map(|state| state.is_active())
            .map_err(|error| access_failure(DiagnosticOperation::NativeStateRead, error))
    }

    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = self
            .nightlight
            .update_settings(|settings| settings.set_schedule(win_nightlight_lib::Schedule::Off));
        #[cfg(feature = "output-measurement")]
        crate::measurement::call("disable_schedule", None, started, result.is_ok());
        result
            .map(|_written| ())
            .map_err(|error| access_failure(DiagnosticOperation::NativeScheduleWrite, error))
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = match mode {
            TemperatureMode::Set => self.nightlight.set_color_temperature(value),
            TemperatureMode::Apply => self.nightlight.apply_color_temperature(value),
            TemperatureMode::Hold | TemperatureMode::ApplyHeld | TemperatureMode::FinishHeld => {
                self.nightlight.preview_color_temperature(value)
            }
        };
        #[cfg(feature = "output-measurement")]
        crate::measurement::call(
            match mode {
                TemperatureMode::Set => "set_color_temperature",
                TemperatureMode::Apply => "apply_color_temperature",
                TemperatureMode::Hold
                | TemperatureMode::ApplyHeld
                | TemperatureMode::FinishHeld => "preview_color_temperature",
            },
            Some(value.kelvin()),
            started,
            result.is_ok(),
        );
        result
            .map(|_previous_or_written| ())
            .map_err(|error| access_failure(DiagnosticOperation::TemperatureWrite, error))
    }

    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = self.nightlight.end_preview();
        #[cfg(feature = "output-measurement")]
        crate::measurement::call("end_preview", None, started, result.is_ok());
        result
            .map(|_previous| ())
            .map_err(|error| access_failure(DiagnosticOperation::PreviewCleanup, error))
    }

    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        #[cfg(feature = "output-measurement")]
        let started = Instant::now();
        let result = self.nightlight.set_active(active);
        #[cfg(feature = "output-measurement")]
        crate::measurement::call("set_active", None, started, result.is_ok());
        result
            .map(|_written| ())
            .map_err(|error| access_failure(DiagnosticOperation::NativeActivationWrite, error))
    }
}

fn consume(
    slot: &mut Option<Watcher>,
    operation: DiagnosticOperation,
    mode: ObservationMode,
    report: &mut NativeNotifications,
) {
    let Some(watcher) = slot.as_ref() else {
        return;
    };
    let result = match mode {
        ObservationMode::Read => {
            #[cfg(feature = "output-measurement")]
            let started = Instant::now();
            let result = watcher.read_pending();
            #[cfg(feature = "output-measurement")]
            if !matches!(&result, Ok(None)) {
                crate::measurement::call("read_notification", None, started, result.is_ok());
            }
            result.map(|snapshot| {
                let Some(snapshot) = snapshot else {
                    return;
                };
                report.changed = true;
                match snapshot {
                    Snapshot::Settings(settings) => {
                        report.settings = Some(settings_values(&settings));
                    }
                    Snapshot::State(state) => report.active = Some(state.is_active()),
                }
            })
        }
        ObservationMode::Defer => watcher
            .discard_pending()
            .map(|changed| report.changed |= changed),
    };
    if let Err(error) = result {
        let operation = if error.kind() == win_nightlight_watch::ErrorKind::Read {
            operation
        } else {
            *slot = None;
            DiagnosticOperation::Observation
        };
        report.errors.push(watch_failure(operation, error));
    }
}

fn settings_values(settings: &win_nightlight_lib::Settings) -> NativeSettings {
    NativeSettings {
        schedule_enabled: settings.is_schedule_enabled(),
        temperature: settings.color_temperature(),
        previewing: settings.is_previewing(),
    }
}

fn diagnostic(error: &(dyn Error + 'static)) -> DiagnosticFailure {
    let mut current = Some(error);
    let mut category = DiagnosticCategory::Windows;
    while let Some(cause) = current {
        if let Some(error) = cause.downcast_ref::<windows_result::Error>() {
            let code = error.code().0;
            category = match windows_result::WIN32_ERROR::from_error(error).map(|value| value.0) {
                Some(2 | 3 | 1018) => DiagnosticCategory::Missing,
                Some(5) => DiagnosticCategory::AccessDenied,
                _ => category,
            };
            return DiagnosticFailure {
                category,
                code: Some(code),
            };
        }
        if let Some(error) = cause.downcast_ref::<win_nightlight_lib::Error>() {
            category = match error.kind() {
                ErrorKind::NotFound => DiagnosticCategory::Missing,
                ErrorKind::Decode | ErrorKind::Encode | ErrorKind::InvalidInput => {
                    DiagnosticCategory::InvalidData
                }
                _ => category,
            };
        }
        current = cause.source();
    }
    DiagnosticFailure {
        category,
        code: None,
    }
}

fn access_failure(
    operation: DiagnosticOperation,
    source: win_nightlight_lib::Error,
) -> NativeFailure {
    NativeFailure {
        operation,
        diagnostic: diagnostic(&source),
        temperature_stored: source.kind() == ErrorKind::PreviewCleanup,
        source: Box::new(source),
    }
}

fn watch_failure(
    operation: DiagnosticOperation,
    source: win_nightlight_watch::Error,
) -> NativeFailure {
    NativeFailure {
        operation,
        diagnostic: diagnostic(&source),
        temperature_stored: false,
        source: Box::new(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(2, DiagnosticCategory::Missing)]
    #[test_case(3, DiagnosticCategory::Missing)]
    #[test_case(1018, DiagnosticCategory::Missing)]
    #[test_case(5, DiagnosticCategory::AccessDenied)]
    #[test_case(234, DiagnosticCategory::Windows)]
    fn windows_failures_preserve_codes_without_messages(code: u32, category: DiagnosticCategory) {
        let source = windows_result::Error::from(windows_result::WIN32_ERROR(code));
        assert_eq!(
            diagnostic(&source),
            DiagnosticFailure {
                category,
                code: Some(source.code().0)
            }
        );
    }

    #[test]
    fn malformed_native_records_report_invalid_data() {
        let source = win_nightlight_lib::Settings::from_bytes(&[]).expect_err("empty record");
        let error = access_failure(DiagnosticOperation::NativeSettingsRead, source);
        assert_eq!(error.diagnostic().category, DiagnosticCategory::InvalidData);
        assert!(!error.temperature_stored);
    }

    #[test]
    fn native_handle_can_move_to_its_owning_worker() {
        fn assert_send<T: Send>() {}
        assert_send::<super::super::Native>();
        assert_send::<SystemBackend>();
    }
}
