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
        render_xml(|buffer, used| {
            let size = u32::try_from(size_of_val(buffer)).map_err(io::Error::other)?;
            let mut properties = 0;
            // SAFETY: The event is live, the buffer is writable for size bytes,
            // and both outputs remain valid for the synchronous call.
            if unsafe {
                EvtRender(
                    std::ptr::null_mut(),
                    self.0.0,
                    EvtRenderEventXml.cast_unsigned(),
                    size,
                    buffer.as_mut_ptr().cast(),
                    used,
                    &raw mut properties,
                )
            } == 0
            {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        })
    }
}

fn render_xml(
    mut render: impl FnMut(&mut [u16], &mut u32) -> io::Result<()>,
) -> io::Result<String> {
    let mut buffer = vec![0_u16; 1024];
    loop {
        let mut bytes = 0;
        let result = render(&mut buffer, &mut bytes);
        let units = usize::try_from(bytes)
            .map_err(io::Error::other)?
            .div_ceil(2);
        match result {
            Ok(()) => {
                let xml = buffer
                    .get(..units)
                    .filter(|_| bytes.is_multiple_of(2))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid event XML size")
                    })?;
                return Ok(String::from_utf16_lossy(
                    xml.strip_suffix(&[0]).unwrap_or(xml),
                ));
            }
            Err(error)
                if error.raw_os_error() == Some(ERROR_INSUFFICIENT_BUFFER)
                    && units > buffer.len() =>
            {
                buffer.resize(units, 0);
            }
            Err(error) => return Err(error),
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_xml_uses_one_call_and_only_the_written_bytes() {
        let mut calls = 0;
        let xml = render_xml(|buffer, bytes| {
            calls += 1;
            buffer.fill(0x58);
            buffer[..5].copy_from_slice(&[0x3c, 0x61, 0x2f, 0x3e, 0]);
            *bytes = 10;
            Ok(())
        })
        .expect("render XML");
        assert_eq!(calls, 1);
        assert_eq!(xml, "<a/>");
    }

    #[test]
    fn oversized_xml_retries_with_the_required_byte_count() {
        let expected = format!("<Event>{}</Event>", "x".repeat(4096));
        let record: Vec<_> = expected.encode_utf16().chain([0]).collect();
        let mut calls = 0;
        let xml = render_xml(|buffer, bytes| {
            calls += 1;
            *bytes = u32::try_from(size_of_val(record.as_slice())).expect("test size fits DWORD");
            if buffer.len() < record.len() {
                return Err(io::Error::from_raw_os_error(ERROR_INSUFFICIENT_BUFFER));
            }
            buffer.copy_from_slice(&record);
            Ok(())
        })
        .expect("render large XML");
        assert_eq!(calls, 2);
        assert_eq!(xml, expected);
    }

    #[test]
    fn invalid_xml_sizes_and_windows_errors_end_the_read() {
        for size in [1, u32::MAX - 1] {
            let error = render_xml(|_, bytes| {
                *bytes = size;
                Ok(())
            })
            .expect_err("invalid UTF-16 byte count");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
        for code in [
            ERROR_INSUFFICIENT_BUFFER,
            windows_sys::Win32::winerror::ERROR_ACCESS_DENIED,
        ] {
            let error = render_xml(|_, _| Err(io::Error::from_raw_os_error(code)))
                .expect_err("failed render");
            assert_eq!(error.raw_os_error(), Some(code));
        }
    }
}
