use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::platform::startup::Effective;
use std::io;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Startup {
    pub(super) intent: Option<bool>,
    pub(super) effective: Option<Effective>,
    pub(super) read_error: Option<Arc<io::Error>>,
    pub(super) registration_error: Option<Arc<io::Error>>,
}

impl Startup {
    pub(super) fn refresh(
        &mut self,
        result: io::Result<Effective>,
        requested: Option<bool>,
    ) -> bool {
        match result {
            Ok(effective) => {
                self.effective = Some(effective);
                self.read_error = None;
            }
            Err(error) => {
                self.effective = None;
                self.read_error = Some(Arc::new(error));
            }
        }
        if matches!(
            (requested, self.effective),
            (Some(true), Some(Effective::Enabled)) | (Some(false), Some(Effective::NotRegistered))
        ) {
            return self.registration_error.take().is_some();
        }
        false
    }

    pub(super) fn saved(
        &mut self,
        write: impl FnOnce(bool) -> io::Result<()>,
        read: impl FnOnce() -> io::Result<Effective>,
    ) -> Option<Result<(), DiagnosticFailure>> {
        self.intent
            .take()
            .map(|requested| self.apply(requested, write, read))
    }

    pub(super) fn apply(
        &mut self,
        requested: bool,
        write: impl FnOnce(bool) -> io::Result<()>,
        read: impl FnOnce() -> io::Result<Effective>,
    ) -> Result<(), DiagnosticFailure> {
        let written = write(requested).map_err(Arc::new);
        self.refresh(read(), Some(requested));
        let result = written.and_then(|()| {
            if let Some(error) = &self.read_error {
                return Err(Arc::clone(error));
            }
            match (requested, self.effective) {
                (true, Some(Effective::Enabled)) | (false, Some(Effective::NotRegistered)) => Ok(()),
                (true, Some(Effective::DisabledByWindows)) => Err(Arc::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Windows has disabled startup. Enable Circadia in Windows Settings > Apps > Startup, then refresh its status.",
                ))),
                _ => Err(Arc::new(io::Error::other("Windows startup does not match the saved preference; retry startup registration"))),
            }
        });
        self.registration_error = result.err();
        self.registration_error.as_ref().map_or(Ok(()), |error| {
            Err(DiagnosticFailure {
                category: match error.kind() {
                    io::ErrorKind::PermissionDenied => DiagnosticCategory::AccessDenied,
                    io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => {
                        DiagnosticCategory::InvalidData
                    }
                    _ => DiagnosticCategory::Windows,
                },
                code: error.raw_os_error(),
            })
        })
    }

    pub(super) fn status(&self, requested: bool) -> &'static str {
        match (self.effective, requested) {
            (Some(Effective::Enabled), true) => "Windows startup: Enabled.",
            (Some(Effective::Enabled), false) => {
                "Windows startup: Enabled. The saved preference is off; retry startup registration to remove it."
            }
            (Some(Effective::NotRegistered), false) => "Windows startup: Not registered.",
            (Some(Effective::NotRegistered), true) => {
                "Windows startup: Not registered. The saved preference is on; retry startup registration to apply it."
            }
            (Some(Effective::DisabledByWindows), _) => {
                "Windows startup: Disabled by Windows. To enable it, open Windows Settings > Apps > Startup and enable Circadia."
            }
            (Some(Effective::DifferentCommand), _) => {
                "Windows startup: Registered for another application location or data directory. Retry startup registration to apply the saved preference."
            }
            (None, _) => "Windows startup: Status unavailable. Refresh to try reading it again.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use test_case::test_case;

    #[test_case(true, Effective::Enabled)]
    #[test_case(false, Effective::NotRegistered)]
    fn refresh_clears_a_resolved_registration_error(requested: bool, effective: Effective) {
        let mut startup = Startup::default();
        startup
            .apply(
                requested,
                |_| Err(io::Error::from_raw_os_error(5)),
                || Ok(Effective::DisabledByWindows),
            )
            .expect_err("registration failed");
        assert!(startup.refresh(Ok(effective), Some(requested)));
        assert!(
            startup.registration_error.is_none(),
            "effective state now matches the saved request"
        );
        assert!(
            !startup.refresh(Ok(effective), Some(requested)),
            "recovery is reported once"
        );
    }

    #[test_case(Some(true), Effective::NotRegistered)]
    #[test_case(Some(false), Effective::Enabled)]
    #[test_case(Some(true), Effective::DisabledByWindows)]
    #[test_case(None, Effective::Enabled)]
    fn refresh_preserves_unresolved_registration_errors(
        requested: Option<bool>,
        effective: Effective,
    ) {
        let mut startup = Startup {
            registration_error: Some(Arc::new(io::Error::from_raw_os_error(5))),
            ..Startup::default()
        };
        assert!(!startup.refresh(Ok(effective), requested));
        assert!(startup.registration_error.is_some());
        assert!(!startup.refresh(Err(io::Error::from_raw_os_error(5)), requested));
        assert!(startup.registration_error.is_some());
    }

    #[test]
    fn opening_and_unrelated_saves_do_not_register_or_retry_startup() {
        let mut startup = Startup::default();
        startup.refresh(Ok(Effective::DisabledByWindows), Some(true));
        assert!(
            startup
                .saved(|_| panic!("no explicit intent"), || panic!("no attempt"))
                .is_none()
        );
        startup.intent = Some(true);
        assert!(
            startup
                .saved(
                    |_| Err(io::Error::from_raw_os_error(5)),
                    || Ok(Effective::DisabledByWindows)
                )
                .expect("explicit attempt")
                .is_err()
        );
        assert!(startup.intent.is_none());
        assert!(startup.registration_error.is_some());
        assert!(
            startup
                .saved(
                    |_| panic!("unrelated Save must not retry"),
                    || panic!("no attempt")
                )
                .is_none()
        );
        assert!(startup.registration_error.is_some());
    }

    #[test]
    fn failed_registration_preserves_effective_state_and_explicit_retry_recovers() {
        let mut startup = Startup {
            intent: Some(false),
            ..Startup::default()
        };
        let writes = Cell::new(0);
        let result = startup.saved(
            |requested| {
                assert!(!requested);
                writes.set(writes.get() + 1);
                Err(io::Error::from_raw_os_error(5))
            },
            || Ok(Effective::Enabled),
        );
        assert_eq!(
            result,
            Some(Err(DiagnosticFailure {
                category: DiagnosticCategory::AccessDenied,
                code: Some(5)
            }))
        );
        assert_eq!(startup.effective, Some(Effective::Enabled));
        assert!(
            startup.registration_error.is_some(),
            "close-with-Save must keep the failure visible"
        );
        assert!(
            startup
                .apply(
                    false,
                    |requested| {
                        assert!(!requested);
                        writes.set(writes.get() + 1);
                        Ok(())
                    },
                    || Ok(Effective::NotRegistered)
                )
                .is_ok()
        );
        assert_eq!(writes.get(), 2);
        assert_eq!(startup.effective, Some(Effective::NotRegistered));
        assert!(startup.registration_error.is_none());
    }

    #[test]
    fn windows_disable_is_not_reported_as_successful_opt_in() {
        let mut startup = Startup::default();
        assert!(
            startup
                .apply(
                    true,
                    |requested| {
                        assert!(requested);
                        Ok(())
                    },
                    || Ok(Effective::DisabledByWindows)
                )
                .is_err()
        );
        assert_eq!(startup.effective, Some(Effective::DisabledByWindows));
        assert!(
            startup
                .registration_error
                .as_ref()
                .expect("Windows action required")
                .to_string()
                .contains("Windows Settings > Apps > Startup")
        );
    }

    #[test]
    fn failed_read_does_not_preserve_a_stale_enabled_claim() {
        let mut startup = Startup::default();
        startup.refresh(Ok(Effective::Enabled), Some(true));
        startup.refresh(Err(io::Error::from_raw_os_error(5)), Some(true));
        assert_eq!(startup.effective, None);
        assert!(startup.read_error.is_some());
    }
}
