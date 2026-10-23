use crate::NativeWake;
use crate::WakeReason;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::BorrowedHandle;
use std::path::Path;
use std::time::Duration;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_FILE_NAME;
use windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_LAST_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_SECURITY;
use windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_SIZE;
use windows_sys::Win32::Storage::FileSystem::FindCloseChangeNotification;
use windows_sys::Win32::Storage::FileSystem::FindFirstChangeNotificationW;
use windows_sys::Win32::Storage::FileSystem::FindNextChangeNotification;

/// Observe configuration-directory changes, including atomic file replacement.
///
/// A notification requests a validated reload; it does not establish changed
/// configuration content or identify the changed file.
#[derive(Debug)]
pub struct ConfigurationNotifications(HANDLE);

#[expect(
    unsafe_code,
    reason = "transfer an exclusively owned kernel notification"
)]
// SAFETY: This independently owned kernel notification can be waited and closed
// on any thread. Rearming requires exclusive access to the owner.
unsafe impl Send for ConfigurationNotifications {}

impl ConfigurationNotifications {
    /// Begin observing an existing directory without reading configuration.
    ///
    /// # Errors
    /// Report invalid paths, missing directories, and notification setup
    /// failures.
    #[expect(unsafe_code, reason = "create a notification from a live UTF-16 path")]
    pub fn open(directory: &Path) -> io::Result<Self> {
        if directory.as_os_str().encode_wide().any(|unit| unit == 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory path has a NUL",
            ));
        }
        let path: Vec<_> = fs_err::canonicalize(directory)?
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: The path is NUL-terminated and remains live throughout the
        // call. The returned notification transfers to this owner.
        let handle = unsafe {
            FindFirstChangeNotificationW(
                path.as_ptr(),
                0,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_ATTRIBUTES
                    | FILE_NOTIFY_CHANGE_SECURITY,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }

    /// Wait for a directory change or an explicit worker wake.
    ///
    /// - Use an indefinite timeout for ordinary background observation.
    /// - Reload configuration after `WakeReason::Observation`; unchanged
    ///   content must remain unchanged in the configuration session.
    /// - A worker wake takes priority over a simultaneously pending change.
    ///
    /// # Errors
    /// Report failed waits, unsupported timeouts, or failed notification
    /// rearming.
    #[expect(
        unsafe_code,
        reason = "borrow and rearm an exclusively owned notification"
    )]
    pub fn wait(&mut self, wake: &NativeWake, timeout: Option<Duration>) -> io::Result<WakeReason> {
        // SAFETY: The notification remains owned throughout the borrowed wait.
        let handle = unsafe { BorrowedHandle::borrow_raw(self.0) };
        let reason = wake.wait(&[handle], timeout)?;
        if reason == WakeReason::Observation {
            // SAFETY: This live notification completed its preceding wait.
            if unsafe { FindNextChangeNotification(self.0) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(reason)
    }
}

impl Drop for ConfigurationNotifications {
    #[expect(
        unsafe_code,
        reason = "close the uniquely owned notification exactly once"
    )]
    fn drop(&mut self) {
        // SAFETY: No wait borrows this exclusively owned notification during
        // drop.
        unsafe { FindCloseChangeNotification(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::io::Write;
    use std::os::windows::ffi::OsStringExt;

    #[test]
    fn file_replacement_and_deletion_wake_an_armed_directory_observer() {
        let directory = tempfile::tempdir().expect("configuration directory");
        let config = directory.path().join("config.toml");
        fs_err::write(&config, "wake = '07:00'").expect("initial configuration");
        let mut replacement =
            tempfile::NamedTempFile::new_in(directory.path()).expect("replacement");
        replacement
            .write_all(b"wake = '08:00'")
            .expect("replacement content");
        replacement.as_file().sync_all().expect("flush replacement");
        let wake = NativeWake::new().expect("worker wake");
        let mut changes =
            ConfigurationNotifications::open(directory.path()).expect("watch directory");
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::ZERO))
                .expect("idle wait"),
            WakeReason::Deadline
        );
        replacement.persist(&config).expect("atomic replacement");
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::from_secs(2)))
                .expect("replacement event"),
            WakeReason::Observation
        );
        drop(changes);

        let mut changes =
            ConfigurationNotifications::open(directory.path()).expect("watch deletion");
        fs_err::remove_file(&config).expect("delete configuration");
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::from_secs(2)))
                .expect("deletion event"),
            WakeReason::Observation
        );
    }

    #[test]
    fn worker_wake_preserves_a_pending_directory_change() {
        let directory = tempfile::tempdir().expect("configuration directory");
        let mut changes =
            ConfigurationNotifications::open(directory.path()).expect("watch directory");
        let wake = NativeWake::new().expect("worker wake");
        fs_err::write(directory.path().join("config.toml"), "wake = '07:00'")
            .expect("create configuration");
        wake.signal().expect("stop worker");
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::from_secs(2)))
                .expect("stop takes priority"),
            WakeReason::Command
        );
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::from_secs(2)))
                .expect("pending file change"),
            WakeReason::Observation
        );
    }

    #[test]
    fn directory_notification_can_transfer_to_a_worker() {
        fn assert_send<T: Send>() {}
        assert_send::<ConfigurationNotifications>();
        let directory = tempfile::tempdir().expect("configuration directory");
        let mut changes =
            ConfigurationNotifications::open(directory.path()).expect("watch directory");
        let wake = NativeWake::new().expect("worker wake");
        let worker = std::thread::spawn(move || changes.wait(&wake, Some(Duration::from_secs(2))));
        fs_err::write(directory.path().join("config.toml"), "wake = '07:00'")
            .expect("create configuration");
        assert_eq!(
            worker.join().expect("worker join").expect("notification"),
            WakeReason::Observation
        );
    }

    #[test]
    fn invalid_directory_paths_return_errors() {
        let directory = tempfile::tempdir().expect("configuration directory");
        assert_eq!(
            ConfigurationNotifications::open(&directory.path().join("absent"))
                .expect_err("missing directory")
                .kind(),
            io::ErrorKind::NotFound
        );
        let nul = OsString::from_wide(&[b'a'.into(), 0, b'b'.into()]);
        assert_eq!(
            ConfigurationNotifications::open(Path::new(&nul))
                .expect_err("embedded NUL")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
