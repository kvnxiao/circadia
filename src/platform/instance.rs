use std::io;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;
use winsafe::HEVENT;
use winsafe::HWND;
use winsafe::co;
use winsafe::guard::CloseHandleGuard;

pub(crate) const EVENT_WINDOW_TITLE: &str = "Circadia event receiver";
pub(crate) const ACTIVATE_SETTINGS: u32 = WM_APP + 1;

/// Distinguish the session's controller owner from a subsequent invocation.
#[derive(Debug)]
pub enum InstanceClaim {
    /// Retain the guard until the application and its controller have stopped.
    Primary(PrimaryInstance),
    /// Activate the existing process without creating a controller.
    Existing(ExistingInstance),
}

/// Reserve controller ownership in the current Windows session.
#[derive(Debug)]
pub struct PrimaryInstance(InstanceHandles);

/// Request settings activation in the current session's running instance.
#[derive(Debug)]
pub struct ExistingInstance(InstanceHandles);

struct InstanceHandles {
    _mutex: OwnedHandle,
    ready: CloseHandleGuard<HEVENT>,
}

impl std::fmt::Debug for InstanceHandles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstanceHandles")
            .finish_non_exhaustive()
    }
}

impl InstanceClaim {
    /// Claim the session before constructing any controller or native backend.
    ///
    /// # Errors
    /// Report failed creation or access to the session mutex or readiness
    /// event.
    pub fn acquire() -> io::Result<Self> {
        Self::acquire_named("Local\\Circadia.Instance.v1")
    }

    #[expect(unsafe_code, reason = "create and own a named Windows mutex")]
    pub(crate) fn acquire_named(name: &str) -> io::Result<Self> {
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: The terminated name remains live, and null security
        // attributes select a noninheritable handle with the caller's
        // default security.
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
        // SAFETY: Capture this thread's creation result before another Win32
        // call.
        let status = unsafe { GetLastError() };
        if mutex.is_null() {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
        // SAFETY: CreateMutexW returned a new CloseHandle-owned handle,
        // including when it opened an existing object. This transfers
        // ownership once.
        let mutex = unsafe { OwnedHandle::from_raw_handle(mutex) };
        let ready = HEVENT::CreateEvent(None, true, false, Some(&format!("{name}.Ready")))
            .map_err(io::Error::other)?;
        let handles = InstanceHandles {
            _mutex: mutex,
            ready,
        };
        if status == ERROR_ALREADY_EXISTS {
            Ok(Self::Existing(ExistingInstance(handles)))
        } else {
            handles.ready.ResetEvent().map_err(io::Error::other)?;
            Ok(Self::Primary(PrimaryInstance(handles)))
        }
    }
}

impl PrimaryInstance {
    /// Publish readiness after creating the application-lifetime event window.
    ///
    /// # Errors
    /// Report failure to signal the readiness event.
    pub fn publish_ready(&self) -> io::Result<()> {
        self.0.ready.SetEvent().map_err(io::Error::other)
    }
}

impl ExistingInstance {
    /// Wait for startup and post an activation request to the existing process.
    ///
    /// - Do not construct a controller in this invocation.
    /// - Allow ten seconds for the primary to create its event receiver.
    /// - Foreground activation remains subject to Windows foreground rules.
    ///
    /// # Errors
    /// Report startup timeout, a missing receiver, or rejected message
    /// delivery.
    #[expect(unsafe_code, reason = "post a pointer-free activation message")]
    pub fn activate(&self) -> io::Result<()> {
        self.wait_ready(10_000)?;
        let window = HWND::FindWindow(None, Some(EVENT_WINDOW_TITLE))
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "running instance has exited")
            })?;
        let (_, process) = window.GetWindowThreadProcessId();
        if process != 0 {
            let _foreground_permission = winsafe::AllowSetForegroundWindow(Some(process));
        }
        // SAFETY: Both parameters are immediate zero values, so asynchronous
        // delivery retains no pointers. A stale HWND is reported by Windows.
        if unsafe { PostMessageW(window.ptr(), ACTIVATE_SETTINGS, 0, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn wait_ready(&self, milliseconds: u32) -> io::Result<()> {
        match self
            .0
            .ready
            .WaitForSingleObject(Some(milliseconds))
            .map_err(io::Error::other)?
        {
            co::WAIT::OBJECT_0 => Ok(()),
            co::WAIT::TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "running instance did not finish startup",
            )),
            _ => Err(io::Error::other("unexpected startup event wait result")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Barrier;

    fn name() -> String {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        format!(
            "Local\\Circadia.Instance.Test.{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )
    }

    #[test]
    fn concurrent_claims_create_exactly_one_primary() {
        let name = name();
        let barrier = Arc::new(Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let name = name.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let claim = InstanceClaim::acquire_named(&name);
                    barrier.wait();
                    let claim = claim.expect("session claim");
                    let primary = matches!(claim, InstanceClaim::Primary(_));
                    drop(claim);
                    primary
                })
            })
            .collect();
        let primaries = workers
            .into_iter()
            .map(|worker| usize::from(worker.join().expect("claim worker")))
            .sum::<usize>();
        assert_eq!(primaries, 1);
        assert!(matches!(
            InstanceClaim::acquire_named(&name).expect("claim after every handle closes"),
            InstanceClaim::Primary(_)
        ));
    }

    #[test]
    fn subsequent_invocation_waits_until_the_primary_publishes_readiness() {
        let name = name();
        let InstanceClaim::Primary(primary) = InstanceClaim::acquire_named(&name).expect("primary")
        else {
            panic!("new name must create the primary");
        };
        let InstanceClaim::Existing(existing) =
            InstanceClaim::acquire_named(&name).expect("second invocation")
        else {
            panic!("second invocation must not own a controller");
        };
        assert_eq!(
            existing
                .wait_ready(0)
                .expect_err("startup still pending")
                .kind(),
            io::ErrorKind::TimedOut
        );
        primary.publish_ready().expect("receiver is ready");
        existing
            .wait_ready(0)
            .expect("pending activation can proceed");
        existing
            .wait_ready(0)
            .expect("readiness stays signaled for later invocations");
    }
}
