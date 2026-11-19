//! Query the local Windows Event Log and render event XML.

use std::io;
use std::marker::PhantomData;
use windows_sys::Win32::winerror::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::winerror::ERROR_NO_MORE_ITEMS;
use windows_sys::Win32::winevt::EVT_HANDLE;
use windows_sys::Win32::winevt::EvtClose;
use windows_sys::Win32::winevt::EvtNext;
use windows_sys::Win32::winevt::EvtQuery;
use windows_sys::Win32::winevt::EvtQueryChannelPath;
use windows_sys::Win32::winevt::EvtQueryReverseDirection;
use windows_sys::Win32::winevt::EvtRender;
use windows_sys::Win32::winevt::EvtRenderEventXml;

struct Handle<'query>(EVT_HANDLE, PhantomData<&'query Query>);

impl Drop for Handle<'_> {
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

/// A reverse-chronological query on the calling thread.
pub struct Query(Handle<'static>);

/// An event that Windows closes when its parent query closes.
///
/// ```compile_fail,E0597
/// use circadia_winsafe::event_log::Query;
/// # fn read() -> std::io::Result<()> {
/// let events = {
///     let mut query = Query::newest("Application", "*")?;
///     query.next_batch::<1>(1000)?
/// };
/// # drop(events);
/// # Ok(())
/// # }
/// ```
pub struct Event<'query>(Handle<'query>);

impl Query {
    /// Open a local event log query, newest first.
    ///
    /// # Errors
    /// Returns the Windows error if the channel or `XPath` query cannot be
    /// opened.
    #[expect(
        unsafe_code,
        reason = "pass a channel and XPath to the local event log"
    )]
    pub fn newest(channel: &str, xpath: &str) -> io::Result<Self> {
        let channel = wide(channel);
        let xpath = wide(xpath);
        // SAFETY: Both strings are NUL-terminated UTF-16 buffers that outlive
        // this synchronous call; a zero session selects the local computer.
        let results = unsafe {
            EvtQuery(
                std::ptr::null_mut(),
                channel.as_ptr(),
                xpath.as_ptr(),
                (EvtQueryChannelPath | EvtQueryReverseDirection).cast_unsigned(),
            )
        };
        if results.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(Handle(results, PhantomData)))
    }

    /// Read up to N events in one call. A timeout of `u32::MAX` waits
    /// indefinitely.
    ///
    /// # Errors
    /// Returns the Windows error for an invalid request, timeout, or other read
    /// failure.
    #[expect(unsafe_code, reason = "receive owned handles in a fixed-size array")]
    pub fn next_batch<const N: usize>(&mut self, timeout_ms: u32) -> io::Result<Vec<Event<'_>>> {
        let mut raw = [std::ptr::null_mut(); N];
        let mut returned = 0_u32;
        let capacity = u32::try_from(raw.len()).map_err(io::Error::other)?;
        // EvtNext fails with ERROR_TIMEOUT when the filtered scan has not
        // produced results within the timeout; a bounded wait keeps a large log
        // from delaying startup indefinitely.
        // SAFETY: The event array holds `capacity` handles and the count output
        // is writable; both outlive this synchronous call.
        let found = unsafe {
            EvtNext(
                self.0.0,
                capacity,
                raw.as_mut_ptr(),
                timeout_ms,
                0,
                &raw mut returned,
            )
        };
        if found == 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS) {
                Ok(Vec::new())
            } else {
                Err(error)
            };
        }
        Ok(raw
            .into_iter()
            .take(returned as usize)
            .map(|event| Event(Handle(event, PhantomData)))
            .collect())
    }
}

impl Event<'_> {
    /// Render the event as XML.
    ///
    /// # Errors
    /// Returns the Windows error if the event cannot be rendered.
    #[expect(unsafe_code, reason = "render one event log record as XML")]
    pub fn xml(&self) -> io::Result<String> {
        let mut used = 0_u32;
        let mut properties = 0_u32;
        // SAFETY: A zero-size request with a null buffer only reports the
        // required size through the writable outputs.
        let sized = unsafe {
            EvtRender(
                std::ptr::null_mut(),
                self.0.0,
                EvtRenderEventXml.cast_unsigned(),
                0,
                std::ptr::null_mut(),
                &raw mut used,
                &raw mut properties,
            )
        };
        if sized == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER) {
                return Err(error);
            }
        }
        let mut buffer = vec![0_u16; usize::try_from(used).map_err(io::Error::other)?.div_ceil(2)];
        let size = u32::try_from(buffer.len() * 2).map_err(io::Error::other)?;
        // SAFETY: The buffer is writable for `size` bytes and outlives this
        // synchronous call, and the outputs are writable.
        let rendered = unsafe {
            EvtRender(
                std::ptr::null_mut(),
                self.0.0,
                EvtRenderEventXml.cast_unsigned(),
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
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}
