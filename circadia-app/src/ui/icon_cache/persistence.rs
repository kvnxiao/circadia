use super::ICON_PIXELS;
use super::IconImages;
use super::Index;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::JoinHandle;

#[derive(Default)]
pub(super) struct Pending {
    pub(super) dirty: bool,
    pub(super) images: BTreeMap<u32, IconImages>,
}

#[derive(Default)]
struct Work {
    index: Option<Index>,
    images: BTreeMap<u32, IconImages>,
    stopping: bool,
}

pub(super) struct Writer {
    shared: Arc<(Mutex<Work>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub(super) fn start(folder: PathBuf) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Work::default()), Condvar::new()));
        let state = Arc::clone(&shared);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let thread = std::thread::Builder::new()
            .name("circadia-icon-cache".into())
            .spawn(move || {
                let _log = tracing::dispatcher::set_default(&dispatch);
                run(&folder, &state);
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub(super) fn submit(&self, index: Index, pending: Pending) {
        let mut work = self
            .shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work.images.extend(pending.images);
        let keep: BTreeSet<u32> = index.apps.iter().map(|entry| entry.image).collect();
        work.images.retain(|image, _| keep.contains(image));
        work.index = Some(index);
        self.shared.1.notify_one();
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        {
            let mut work = self
                .shared
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            work.stopping = true;
        }
        self.shared.1.notify_one();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("icon cache writer panicked");
        }
    }
}

fn run(folder: &Path, shared: &(Mutex<Work>, Condvar)) {
    let mut stored: BTreeSet<u32> = fs_err::read_to_string(folder.join("index.toml"))
        .ok()
        .and_then(|text| toml::from_str::<Index>(&text).ok())
        .and_then(super::valid_index)
        .map(|(entries, _)| entries.into_values().map(|entry| entry.image).collect())
        .unwrap_or_default();
    loop {
        let work = shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut work = shared
            .1
            .wait_while(work, |work| work.index.is_none() && !work.stopping)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(index) = work.index.take() else {
            return;
        };
        let images = std::mem::take(&mut work.images);
        drop(work);
        let keep: BTreeSet<u32> = index.apps.iter().map(|entry| entry.image).collect();
        for image in stored.difference(&keep) {
            delete_images(folder, *image);
        }
        for (image, images) in images {
            if let Err(error) = super::write_images(folder, image, &images) {
                tracing::warn!(%error, "writing a cached icon");
                delete_images(folder, image);
            }
        }
        stored = keep;
        let written = toml::to_string(&index)
            .map_err(io::Error::other)
            .and_then(|text| {
                fs_err::create_dir_all(folder)?;
                let partial = folder.join("index.toml.tmp");
                fs_err::write(&partial, text)?;
                fs_err::rename(partial, folder.join("index.toml"))
            });
        if let Err(error) = written {
            tracing::warn!(%error, "writing the icon cache index");
        }
    }
}

fn delete_images(folder: &Path, image: u32) {
    for pixels in ICON_PIXELS {
        if let Err(error) = fs_err::remove_file(super::image_path(folder, image, pixels))
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "deleting a cached icon");
        }
    }
}

#[cfg(test)]
mod tests;
