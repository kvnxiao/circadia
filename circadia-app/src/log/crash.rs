use std::ffi::c_void;
use std::panic::PanicHookInfo;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) fn panic_record(info: &PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    format!(
        "{:.6} ERROR circadia_app::log: panic process_id={} created={:#x} thread={:?} location={:?} message={:?} frames={}\n",
        jiff::Timestamp::now(),
        std::process::id(),
        super::run::process_created().unwrap_or_default(),
        thread.name().unwrap_or("unnamed"),
        info.location().map(ToString::to_string).unwrap_or_default(),
        info.payload_as_str().unwrap_or("non-string payload"),
        frames(),
    )
}

static EXCEPTION_LOG: OnceLock<PathBuf> = OnceLock::new();
pub(crate) fn install_exception_filter(data_directory: &Path) {
    if EXCEPTION_LOG.set(data_directory.to_owned()).is_ok() {
        circadia_winsafe::crash::install_exception_filter(record_exception);
    }
}

fn record_exception(code: u32, address: usize) {
    if let Some(directory) = EXCEPTION_LOG.get() {
        let (module, offset) = module_offset(std::ptr::without_provenance(address))
            .unwrap_or_else(|| ("unknown".to_owned(), address));
        let line = format!(
            "{:.6} ERROR circadia_app::log: exception process_id={} created={:#x} thread={:?} code={code:#010x} module={module} offset={offset:#x} frames={}\n",
            jiff::Timestamp::now(),
            std::process::id(),
            super::run::process_created().unwrap_or_default(),
            std::thread::current().name().unwrap_or("unnamed"),
            frames(),
        );
        super::append_to_current_file(directory, line.as_bytes());
    }
}

fn frames() -> String {
    let mut addresses = [std::ptr::null_mut::<c_void>(); 62];
    circadia_winsafe::crash::capture_frames(&mut addresses)
        .iter()
        .map(|address| {
            module_offset(address.cast_const()).map_or_else(
                || format!("{:#x}", address.addr()),
                |(module, offset)| format!("{module}+{offset:#x}"),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn module_offset(address: *const c_void) -> Option<(String, usize)> {
    let module = winsafe::HINSTANCE::GetModuleHandleEx(
        winsafe::AddrStr::Addr(address.cast_mut()),
        winsafe::co::GET_MODULE_HANDLE_EX_FLAG::NoValue,
    )
    .ok()?;
    let path = module.GetModuleFileName().ok()?;
    let name = Path::new(&path).file_name().map_or_else(
        || "unknown".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    Some((name, address.addr().wrapping_sub(module.ptr().addr())))
}
