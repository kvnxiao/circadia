use std::io;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::ERROR_NO_MORE_ITEMS;
use windows_sys::Win32::System::EventLog::EVT_HANDLE;
use windows_sys::Win32::System::EventLog::EvtClose;
use windows_sys::Win32::System::EventLog::EvtNext;
use windows_sys::Win32::System::EventLog::EvtQuery;
use windows_sys::Win32::System::EventLog::EvtQueryChannelPath;
use windows_sys::Win32::System::EventLog::EvtQueryReverseDirection;
use windows_sys::Win32::System::EventLog::EvtRender;
use windows_sys::Win32::System::EventLog::EvtRenderEventXml;

struct Handle(EVT_HANDLE);

impl Drop for Handle {
    #[expect(
        unsafe_code,
        reason = "close an event log handle owned by this wrapper"
    )]
    fn drop(&mut self) {
        // SAFETY: The wrapper owns this handle, which EvtQuery or EvtNext
        // returned, and closes it once.
        unsafe { EvtClose(self.0) };
    }
}

/// Read up to 16 Application log events matching `xpath`, newest first, as
/// event XML.
#[expect(unsafe_code, reason = "query the Application event log")]
pub(crate) fn query(xpath: &str) -> io::Result<Vec<String>> {
    let channel = wide("Application");
    let xpath = wide(xpath);
    // SAFETY: Both strings are NUL-terminated UTF-16 buffers that outlive this
    // synchronous call; a zero session selects the local computer.
    let results = unsafe {
        EvtQuery(
            0,
            channel.as_ptr(),
            xpath.as_ptr(),
            EvtQueryChannelPath | EvtQueryReverseDirection,
        )
    };
    if results == 0 {
        return Err(io::Error::last_os_error());
    }
    let results = Handle(results);
    let mut raw = [0_isize; 16];
    let mut returned = 0_u32;
    let capacity = u32::try_from(raw.len()).map_err(io::Error::other)?;
    // EvtNext fails with ERROR_TIMEOUT when the filtered scan has not produced
    // results within the timeout; a bounded wait keeps a large log from
    // delaying startup indefinitely.
    // SAFETY: The event array holds `capacity` handles and the count output is
    // writable; both outlive this synchronous call.
    let found = unsafe {
        EvtNext(
            results.0,
            capacity,
            raw.as_mut_ptr(),
            1000,
            0,
            &raw mut returned,
        )
    };
    if found == 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS.cast_signed()) {
            Ok(Vec::new())
        } else {
            Err(error)
        };
    }
    let events: Vec<_> = raw
        .iter()
        .take(usize::try_from(returned).map_err(io::Error::other)?)
        .map(|event| Handle(*event))
        .collect();
    events.iter().map(render).collect()
}

#[expect(unsafe_code, reason = "render one event log record as XML")]
fn render(event: &Handle) -> io::Result<String> {
    let mut used = 0_u32;
    let mut properties = 0_u32;
    // SAFETY: A zero-size request with a null buffer only reports the required
    // size through the writable outputs.
    let sized = unsafe {
        EvtRender(
            0,
            event.0,
            EvtRenderEventXml,
            0,
            std::ptr::null_mut(),
            &raw mut used,
            &raw mut properties,
        )
    };
    if sized == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER.cast_signed()) {
            return Err(error);
        }
    }
    let mut buffer = vec![0_u16; usize::try_from(used).map_err(io::Error::other)?.div_ceil(2)];
    let size = u32::try_from(buffer.len() * 2).map_err(io::Error::other)?;
    // SAFETY: The buffer is writable for `size` bytes and outlives this
    // synchronous call, and the outputs are writable.
    let rendered = unsafe {
        EvtRender(
            0,
            event.0,
            EvtRenderEventXml,
            size,
            buffer.as_mut_ptr().cast(),
            &raw mut used,
            &raw mut properties,
        )
    };
    if rendered == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(String::from_utf16_lossy(&buffer)
        .trim_end_matches('\0')
        .to_owned())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
