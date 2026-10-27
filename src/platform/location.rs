use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use windows::Devices::Geolocation::GeolocationAccessStatus;
use windows::Devices::Geolocation::Geolocator;
use windows::Devices::Geolocation::Geoposition;
use windows::core::Interface;
use windows_future::AsyncOperationCompletedHandler;
use windows_future::IAsyncInfo;
use windows_future::IAsyncOperation;

pub(crate) struct Request(IAsyncInfo);

impl Request {
    pub(crate) fn access(
        completed: impl Fn(Result<(), DiagnosticFailure>) + Send + 'static,
    ) -> Result<Option<Self>, DiagnosticFailure> {
        let foreground =
            winsafe::HWND::GetForegroundWindow().map(|window| window.GetWindowThreadProcessId());
        if foreground != Some((winsafe::GetCurrentThreadId(), std::process::id())) {
            return Ok(None);
        }
        let operation = Geolocator::RequestAccessAsync().map_err(|error| failure(&error))?;
        let request = Self(operation.cast().map_err(|error| failure(&error))?);
        operation
            .SetCompleted(&AsyncOperationCompletedHandler::new(move |operation, _| {
                let result = operation
                    .ok()
                    .and_then(IAsyncOperation::GetResults)
                    .map_err(|error| failure(&error))
                    .and_then(access_result);
                completed(result);
                Ok(())
            }))
            .map_err(|error| failure(&error))?;
        Ok(Some(request))
    }

    pub(crate) fn position(
        completed: impl Fn(Result<Coordinates, DiagnosticFailure>) + Send + 'static,
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

impl Drop for Request {
    fn drop(&mut self) {
        drop(self.0.Cancel());
    }
}

fn access_result(access: GeolocationAccessStatus) -> Result<(), DiagnosticFailure> {
    let category = match access {
        GeolocationAccessStatus::Allowed => return Ok(()),
        GeolocationAccessStatus::Denied => DiagnosticCategory::AccessDenied,
        _ => DiagnosticCategory::Windows,
    };
    Err(DiagnosticFailure {
        category,
        code: None,
    })
}

// A stopped or disabled Geolocation service fails the position request with
// the service-disabled error rather than an access denial.
fn failure(error: &windows::core::Error) -> DiagnosticFailure {
    let code = error.code();
    let category = if code.0 == windows_sys::Win32::Foundation::E_ACCESSDENIED {
        DiagnosticCategory::AccessDenied
    } else if code
        == windows::core::HRESULT::from_win32(
            windows_sys::Win32::Foundation::ERROR_SERVICE_DISABLED,
        )
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
#[expect(
    unsafe_code,
    clippy::ref_as_ptr,
    clippy::inline_always,
    reason = "generated COM interface for the cancellation test"
)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use test_case::test_case;
    use windows_future::AsyncStatus;
    use windows_future::IAsyncInfo_Impl;

    #[windows::core::implement(IAsyncInfo)]
    struct Cancellation(Arc<AtomicUsize>);

    impl IAsyncInfo_Impl for Cancellation_Impl {
        fn Id(&self) -> windows::core::Result<u32> {
            Ok(1)
        }
        fn Status(&self) -> windows::core::Result<AsyncStatus> {
            Ok(AsyncStatus::Started)
        }
        fn ErrorCode(&self) -> windows::core::Result<windows::core::HRESULT> {
            Ok(windows::core::HRESULT(0))
        }
        fn Cancel(&self) -> windows::core::Result<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn Close(&self) -> windows::core::Result<()> {
            Ok(())
        }
    }

    #[test_case(GeolocationAccessStatus::Allowed, None)]
    #[test_case(
        GeolocationAccessStatus::Denied,
        Some(DiagnosticCategory::AccessDenied)
    )]
    #[test_case(
        GeolocationAccessStatus::Unspecified,
        Some(DiagnosticCategory::Windows)
    )]
    fn permission_outcomes(access: GeolocationAccessStatus, expected: Option<DiagnosticCategory>) {
        assert_eq!(
            access_result(access).err().map(|failure| failure.category),
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
        let request = Request(Cancellation(cancelled.clone()).into());
        drop(request);
        assert_eq!(cancelled.load(Ordering::Relaxed), 1);
    }
}
