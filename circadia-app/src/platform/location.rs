use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use windows::Devices::Geolocation::Geolocator;
use windows::Devices::Geolocation::Geoposition;
use windows::Security::Authorization::AppCapabilityAccess::AppCapability;
use windows::Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus;
use windows::core::Interface;
use windows::core::h;
use windows_future::AsyncOperationCompletedHandler;
use windows_future::IAsyncInfo;
use windows_future::IAsyncOperation;

pub(crate) struct Request(IAsyncInfo);

impl Request {
    pub(crate) fn permission() -> Result<(), DiagnosticFailure> {
        AppCapability::Create(h!("location"))
            .and_then(|capability| capability.CheckAccess())
            .map_err(|error| failure(&error))
            .and_then(permission_result)
    }

    pub(crate) fn position(
        completed: impl Fn(Result<Coordinates, DiagnosticFailure>) + Send + Sync + 'static,
    ) -> Result<Self, DiagnosticFailure> {
        let operation = Geolocator::new()
            .and_then(|locator| locator.GetGeopositionAsync())
            .map_err(|error| failure(&error))?;
        let request = Self(operation.cast().map_err(|error| failure(&error))?);
        operation
            .SetCompleted(&AsyncOperationCompletedHandler::<Geoposition>::new(
                move |operation, _| {
                    let result = operation
                        .ok()
                        .and_then(IAsyncOperation::GetResults)
                        .and_then(|position| position.Coordinate())
                        .and_then(|coordinate| coordinate.Point())
                        .and_then(|point| point.Position())
                        .map(|position| Coordinates {
                            latitude: position.Latitude,
                            longitude: position.Longitude,
                        })
                        .map_err(|error| failure(&error));
                    completed(result);
                    Ok(())
                },
            ))
            .map_err(|error| failure(&error))?;
        Ok(request)
    }
}

fn permission_result(access: AppCapabilityAccessStatus) -> Result<(), DiagnosticFailure> {
    let category = match access {
        AppCapabilityAccessStatus::Allowed => return Ok(()),
        AppCapabilityAccessStatus::DeniedByUser
        | AppCapabilityAccessStatus::DeniedBySystem
        | AppCapabilityAccessStatus::UserPromptRequired => DiagnosticCategory::AccessDenied,
        _ => DiagnosticCategory::Windows,
    };
    Err(DiagnosticFailure {
        category,
        code: None,
    })
}

impl Drop for Request {
    fn drop(&mut self) {
        drop(self.0.Cancel());
    }
}

// A stopped or disabled Geolocation service fails the position request with
// the service-disabled error rather than an access denial.
fn failure(error: &windows::core::Error) -> DiagnosticFailure {
    let code = error.code();
    let category = if code.0 == windows_sys::Win32::winerror::E_ACCESSDENIED {
        DiagnosticCategory::AccessDenied
    } else if code
        == windows_result::WIN32_ERROR(
            windows_sys::Win32::winerror::ERROR_SERVICE_DISABLED.cast_unsigned(),
        )
        .to_hresult()
    {
        DiagnosticCategory::Unavailable
    } else {
        DiagnosticCategory::Windows
    };
    DiagnosticFailure {
        category,
        code: Some(code.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use test_case::test_case;

    #[test_case(AppCapabilityAccessStatus::Allowed, None)]
    #[test_case(
        AppCapabilityAccessStatus::DeniedByUser,
        Some(DiagnosticCategory::AccessDenied)
    )]
    #[test_case(
        AppCapabilityAccessStatus::DeniedBySystem,
        Some(DiagnosticCategory::AccessDenied)
    )]
    #[test_case(
        AppCapabilityAccessStatus::UserPromptRequired,
        Some(DiagnosticCategory::AccessDenied)
    )]
    #[test_case(
        AppCapabilityAccessStatus::NotDeclaredByApp,
        Some(DiagnosticCategory::Windows)
    )]
    fn silent_permission_outcomes(
        access: AppCapabilityAccessStatus,
        expected: Option<DiagnosticCategory>,
    ) {
        assert_eq!(
            permission_result(access)
                .err()
                .map(|failure| failure.category),
            expected
        );
    }

    #[test_case(0x8007_0005_u32, DiagnosticCategory::AccessDenied; "access denied")]
    #[test_case(0x8007_0422_u32, DiagnosticCategory::Unavailable; "service disabled")]
    #[test_case(0x8000_4005_u32, DiagnosticCategory::Windows; "unspecified failure")]
    fn position_failures_keep_their_code(code: u32, category: DiagnosticCategory) {
        let code = windows::core::HRESULT(code.cast_signed());
        assert_eq!(
            failure(&windows::core::Error::from_hresult(code)),
            DiagnosticFailure {
                category,
                code: Some(code.0),
            }
        );
    }

    #[test]
    fn dropping_request_asks_windows_to_cancel_once() {
        let cancelled = Arc::new(AtomicUsize::new(0));
        let request = Request(circadia_winsafe::test_support::counting_cancellation(
            cancelled.clone(),
        ));
        drop(request);
        assert_eq!(cancelled.load(Ordering::Relaxed), 1);
    }
}
