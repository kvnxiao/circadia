use crate::Coordinates;
use std::io;
use windows::Devices::Geolocation::GeolocationAccessStatus;
use windows::Devices::Geolocation::Geolocator;
use windows::Devices::Geolocation::Geoposition;
use windows::core::Interface;
use windows_future::AsyncOperationCompletedHandler;
use windows_future::IAsyncInfo;
use windows_future::IAsyncOperation;

pub(crate) struct Request(IAsyncInfo);

impl Request {
    pub(crate) fn access(completed: impl Fn(io::Result<()>) + Send + 'static) -> io::Result<Self> {
        let foreground =
            winsafe::HWND::GetForegroundWindow().map(|window| window.GetWindowThreadProcessId());
        if foreground != Some((winsafe::GetCurrentThreadId(), std::process::id())) {
            return Err(io::Error::other(
                "focus settings before requesting location",
            ));
        }
        let operation = Geolocator::RequestAccessAsync().map_err(io::Error::other)?;
        let request = Self(operation.cast().map_err(io::Error::other)?);
        operation
            .SetCompleted(&AsyncOperationCompletedHandler::new(move |operation, _| {
                let result = operation
                    .ok()
                    .and_then(IAsyncOperation::GetResults)
                    .map_err(io::Error::other)
                    .and_then(access_result);
                completed(result);
                Ok(())
            }))
            .map_err(io::Error::other)?;
        Ok(request)
    }

    pub(crate) fn position(
        completed: impl Fn(io::Result<Coordinates>) + Send + 'static,
    ) -> io::Result<Self> {
        let operation = Geolocator::new()
            .and_then(|locator| locator.GetGeopositionAsync())
            .map_err(io::Error::other)?;
        let request = Self(operation.cast().map_err(io::Error::other)?);
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
                        .map_err(io::Error::other);
                    completed(result);
                    Ok(())
                },
            ))
            .map_err(io::Error::other)?;
        Ok(request)
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        drop(self.0.Cancel());
    }
}

fn access_result(access: GeolocationAccessStatus) -> io::Result<()> {
    match access {
        GeolocationAccessStatus::Allowed => Ok(()),
        GeolocationAccessStatus::Denied => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows denied location access; enter coordinates manually or check Windows location privacy settings",
        )),
        _ => Err(io::Error::other(
            "Windows could not grant location access; enter coordinates manually",
        )),
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
    #[test_case(GeolocationAccessStatus::Denied, Some(io::ErrorKind::PermissionDenied))]
    #[test_case(GeolocationAccessStatus::Unspecified, Some(io::ErrorKind::Other))]
    fn permission_outcomes(access: GeolocationAccessStatus, expected: Option<io::ErrorKind>) {
        assert_eq!(
            access_result(access).err().map(|error| error.kind()),
            expected
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
