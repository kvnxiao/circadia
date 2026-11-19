//! Record the faulting thread's exception and stack addresses.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use windows_sys::Win32::errhandlingapi::SetUnhandledExceptionFilter;
use windows_sys::Win32::winnt::EXCEPTION_POINTERS;
use windows_sys::Win32::winnt::RtlCaptureStackBackTrace;

static REPORT: OnceLock<fn(u32, usize)> = OnceLock::new();
// A fault inside the filter can dispatch a second exception on the same thread.
static FILTER_ENTERED: AtomicBool = AtomicBool::new(false);

/// Install the process-wide filter once. It reports the exception code and
/// address on the faulting thread and lets Windows continue crash reporting.
#[expect(unsafe_code, reason = "install a static Windows exception callback")]
pub fn install_exception_filter(report: fn(u32, usize)) {
    if REPORT.set(report).is_ok() {
        // SAFETY: The callback has the filter ABI and remains installed for the
        // life of the process.
        unsafe { SetUnhandledExceptionFilter(Some(record_exception)) };
    }
}

#[expect(
    unsafe_code,
    reason = "read the exception record supplied for this callback"
)]
extern "system" fn record_exception(pointers: *const EXCEPTION_POINTERS) -> i32 {
    if FILTER_ENTERED.swap(true, Ordering::Relaxed) {
        return 0;
    }
    // SAFETY: Windows supplies a valid exception structure for this callback,
    // or null.
    let pointers = unsafe { pointers.as_ref() };
    // SAFETY: A non-null ExceptionRecord belongs to the same callback.
    let record = pointers.and_then(|pointers| unsafe { pointers.ExceptionRecord.as_ref() });
    if let (Some(report), Some(record)) = (REPORT.get(), record) {
        report(record.ExceptionCode, record.ExceptionAddress.addr());
    }
    0
}

/// Write the current thread's return addresses and borrow the captured prefix.
#[expect(
    unsafe_code,
    reason = "capture the stack into the caller's address buffer"
)]
pub fn capture_frames(buffer: &mut [*mut c_void]) -> &[*mut c_void] {
    let capacity = u16::try_from(buffer.len()).unwrap_or(u16::MAX);
    // SAFETY: The output has room for at least capacity pointers. The optional
    // hash is null.
    let count = unsafe {
        RtlCaptureStackBackTrace(
            0,
            u32::from(capacity),
            buffer.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    buffer.get(..usize::from(count)).unwrap_or(buffer)
}
