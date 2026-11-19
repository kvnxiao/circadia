//! Windows fixtures for the application's platform tests.

pub use cancellation::counting_cancellation;

#[expect(
    unsafe_code,
    clippy::inline_always,
    reason = "generated COM implementation of the cancellation test fixture"
)]
mod cancellation {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use windows_future::AsyncStatus;
    use windows_future::IAsyncInfo;
    use windows_future::IAsyncInfo_Impl;

    /// Create an async operation interface that counts every cancellation
    /// request.
    pub fn counting_cancellation(calls: Arc<AtomicUsize>) -> IAsyncInfo {
        Cancellation(calls).into()
    }

    #[windows_core::implement(IAsyncInfo)]
    struct Cancellation(Arc<AtomicUsize>);

    impl IAsyncInfo_Impl for Cancellation_Impl {
        fn Id(&self) -> windows_core::Result<u32> {
            Ok(1)
        }
        fn Status(&self) -> windows_core::Result<AsyncStatus> {
            Ok(AsyncStatus::Started)
        }
        fn ErrorCode(&self) -> windows_core::Result<windows_core::HRESULT> {
            Ok(windows_core::HRESULT(0))
        }
        fn Cancel(&self) -> windows_core::Result<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn Close(&self) -> windows_core::Result<()> {
            Ok(())
        }
    }
}

/// Disable Windows fault dialogs for a crash-test child process.
#[expect(
    unsafe_code,
    reason = "keep the intentional child abort out of Windows Error Reporting"
)]
pub fn suppress_crash_reporting() {
    use windows_sys::Win32::errhandlingapi::SetErrorMode;
    use windows_sys::Win32::winbase::SEM_FAILCRITICALERRORS;
    use windows_sys::Win32::winbase::SEM_NOGPFAULTERRORBOX;
    // SAFETY: SetErrorMode changes only this child process's error mode
    // and takes no pointers.
    unsafe { SetErrorMode((SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX).cast_unsigned()) };
}

/// Raise the test exception on the calling thread.
#[expect(
    unsafe_code,
    reason = "raise a native exception that no handler catches"
)]
pub fn raise_native_exception() {
    // SAFETY: The exception has no arguments, so the argument pointer may
    // be null; the call does not return because no handler
    // continues it.
    unsafe {
        windows_sys::Win32::errhandlingapi::RaiseException(0xE000_0001, 0, 0, std::ptr::null());
    };
}
