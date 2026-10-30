use std::ffi::OsString;
use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::panic::PanicHookInfo;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use windows_sys::Win32::errhandlingapi::SetUnhandledExceptionFilter;
use windows_sys::Win32::libloaderapi::GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS;
use windows_sys::Win32::libloaderapi::GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
use windows_sys::Win32::libloaderapi::GetModuleFileNameW;
use windows_sys::Win32::libloaderapi::GetModuleHandleExW;
use windows_sys::Win32::minwindef::HMODULE;
use windows_sys::Win32::winnt::EXCEPTION_POINTERS;
use windows_sys::Win32::winnt::RtlCaptureStackBackTrace;

pub(crate) fn panic_record(info: &PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    format!(
        "{:.6} ERROR circadia::log: panic process_id={} thread={:?} location={:?} message={:?} frames={}\n",
        jiff::Timestamp::now(),
        std::process::id(),
        thread.name().unwrap_or("unnamed"),
        info.location().map(ToString::to_string).unwrap_or_default(),
        info.payload_as_str().unwrap_or("non-string payload"),
        frames(),
    )
}

static EXCEPTION_LOG: OnceLock<PathBuf> = OnceLock::new();
// A fault inside the filter dispatches a nested exception to the filter again.
static FILTER_ENTERED: AtomicBool = AtomicBool::new(false);

#[expect(
    unsafe_code,
    reason = "install the process-wide unhandled exception filter"
)]
pub(crate) fn install_exception_filter(data_directory: &Path) {
    if EXCEPTION_LOG.set(data_directory.to_owned()).is_ok() {
        // SAFETY: The filter is a function with static lifetime that matches
        // the documented filter signature.
        unsafe { SetUnhandledExceptionFilter(Some(record_exception)) };
    }
}

// Returning EXCEPTION_CONTINUE_SEARCH keeps Windows Error Reporting's record of
// the crash.
#[expect(
    unsafe_code,
    reason = "read the exception record Windows passes to the filter"
)]
extern "system" fn record_exception(pointers: *const EXCEPTION_POINTERS) -> i32 {
    if FILTER_ENTERED.swap(true, Ordering::Relaxed) {
        return 0;
    }
    // SAFETY: Windows passes exception pointers that stay valid for the
    // duration of the filter call, or null.
    let pointers = unsafe { pointers.as_ref() };
    // SAFETY: The exception record pointer comes from the same valid
    // structure and stays valid for the duration of the filter call.
    let record = pointers.and_then(|pointers| unsafe { pointers.ExceptionRecord.as_ref() });
    if let (Some(directory), Some(record)) = (EXCEPTION_LOG.get(), record) {
        let (module, offset) = module_offset(record.ExceptionAddress.cast_const())
            .unwrap_or_else(|| ("unknown".to_owned(), record.ExceptionAddress.addr()));
        let line = format!(
            "{:.6} ERROR circadia::log: exception process_id={} thread={:?} code={:#010x} module={module} offset={offset:#x} frames={}\n",
            jiff::Timestamp::now(),
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed"),
            record.ExceptionCode,
            frames(),
        );
        super::append_to_current_file(directory, line.as_bytes());
    }
    0
}

#[expect(unsafe_code, reason = "capture the current thread's return addresses")]
fn frames() -> String {
    let mut addresses = [std::ptr::null_mut::<c_void>(); 62];
    let capacity = u32::try_from(addresses.len()).unwrap_or(0);
    // SAFETY: The buffer has room for `capacity` addresses and outlives this
    // synchronous call; the optional hash output may be null.
    let captured = unsafe {
        RtlCaptureStackBackTrace(0, capacity, addresses.as_mut_ptr(), std::ptr::null_mut())
    };
    addresses
        .iter()
        .take(usize::from(captured))
        .map(|address| {
            module_offset(address.cast_const()).map_or_else(
                || format!("{:#x}", address.addr()),
                |(module, offset)| format!("{module}+{offset:#x}"),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[expect(unsafe_code, reason = "resolve an address to its loaded module")]
fn module_offset(address: *const c_void) -> Option<(String, usize)> {
    let mut module: HMODULE = std::ptr::null_mut();
    // SAFETY: With FROM_ADDRESS, the name argument is only compared with loaded
    // module ranges. UNCHANGED_REFCOUNT leaves no reference to release, and the
    // output is one writable handle.
    let found = unsafe {
        GetModuleHandleExW(
            (GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT)
                .cast_unsigned(),
            address.cast(),
            &raw mut module,
        )
    };
    if found == 0 {
        return None;
    }
    let mut path = [0_u16; 1024];
    let capacity = u32::try_from(path.len()).unwrap_or(0);
    // SAFETY: The buffer is writable for `capacity` UTF-16 units and outlives
    // this synchronous call.
    let length = unsafe { GetModuleFileNameW(module, path.as_mut_ptr(), capacity) };
    let path = OsString::from_wide(
        path.get(..usize::try_from(length).unwrap_or(0))
            .unwrap_or_default(),
    );
    let name = Path::new(&path).file_name().map_or_else(
        || "unknown".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    Some((name, address.addr().wrapping_sub(module.addr())))
}
