use crate::ConfigurationNotifications;
use crate::NativeWake;
use crate::UiWake;
use crate::WakeReason;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

pub(super) struct FileChanges {
    stop: Arc<NativeWake>,
    pending: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<Arc<io::Error>>>>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl FileChanges {
    pub(super) fn start(directory: &Path, wake: UiWake) -> io::Result<Self> {
        fs_err::create_dir_all(directory)?;
        let mut changes = ConfigurationNotifications::open(directory)?;
        let stop = Arc::new(NativeWake::new()?);
        let worker_stop = Arc::clone(&stop);
        let pending = Arc::new(AtomicBool::new(true));
        let pending_write = Arc::clone(&pending);
        let failure = Arc::new(Mutex::new(None));
        let failed = Arc::clone(&failure);
        let thread = std::thread::Builder::new()
            .name("circadia-config-watch".into())
            .spawn(move || {
                let Err(error) = observe(&mut changes, &worker_stop, &pending_write, &wake) else {
                    return Ok(());
                };
                let error = Arc::new(error);
                *failed.lock().map_err(|_poison| {
                    io::Error::other("configuration observer lock poisoned")
                })? = Some(Arc::clone(&error));
                Err::<(), _>(io::Error::other(error)).and(wake.notify())
            })?;
        Ok(Self {
            stop,
            pending,
            failure,
            thread: Some(thread),
        })
    }

    pub(super) fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    pub(super) fn failure(&self) -> io::Result<Option<Arc<io::Error>>> {
        Ok(self
            .failure
            .lock()
            .map_err(|_poison| io::Error::other("configuration observer lock poisoned"))?
            .clone())
    }

    pub(super) fn shutdown(mut self) -> io::Result<()> {
        self.finish()
    }

    fn finish(&mut self) -> io::Result<()> {
        let signal = self.stop.signal();
        let joined = self.thread.take().map_or(Ok(()), |thread| {
            thread
                .join()
                .map_err(|_panic| io::Error::other("configuration observer panicked"))?
        });
        joined.and(signal)
    }
}

impl Drop for FileChanges {
    fn drop(&mut self) {
        drop(self.finish());
    }
}

fn observe(
    changes: &mut ConfigurationNotifications,
    stop: &NativeWake,
    pending: &AtomicBool,
    wake: &UiWake,
) -> io::Result<()> {
    loop {
        match changes.wait(stop, None)? {
            WakeReason::Command => return Ok(()),
            WakeReason::Observation => pending.store(true, Ordering::Release),
            WakeReason::Deadline => continue,
        }
        wake.notify()?;
    }
}
