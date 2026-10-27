use super::file_info::AppFile;
use super::file_info::ICON_PIXELS;
use super::file_info::IconImages;
use super::file_info::IconRead;
use super::file_info::IconRequest;
use crate::RecentApp;
use crate::platform::file_info::FileStamp;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    path: PathBuf,
    size: u64,
    modified: u64,
}

impl Entry {
    fn stamp(&self) -> FileStamp {
        FileStamp {
            size: self.size,
            modified: self.modified,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    apps: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct AppIcons {
    pub(super) apps: BTreeMap<String, IconImages>,
    pub(super) generic: Option<IconImages>,
}

pub(super) struct IconCache {
    folder: PathBuf,
    icons: BTreeMap<String, (Entry, IconImages)>,
    generic: Option<IconImages>,
    view: Rc<AppIcons>,
}

pub(super) fn latest_path<'a>(
    recent: Option<&'a RecentApp>,
    browsed: Option<&'a (PathBuf, Duration)>,
) -> Option<&'a Path> {
    match (recent, browsed) {
        (Some(recent), Some((path, at))) if recent.focus_ended.is_some_and(|ended| ended < *at) => {
            Some(path)
        }
        (Some(recent), _) => Some(&recent.path),
        (None, browsed) => browsed.map(|(path, _)| path.as_path()),
    }
}

impl IconCache {
    pub(super) fn load(directory: &Path) -> Self {
        let folder = directory.join("icons");
        let index = fs_err::read_to_string(folder.join("index.toml"))
            .map_err(|error| error.to_string())
            .and_then(|text| toml::from_str::<Index>(&text).map_err(|error| error.to_string()))
            .inspect_err(|error| tracing::debug!(%error, "no icon cache index"))
            .unwrap_or_default();
        let icons = index
            .apps
            .into_iter()
            .filter_map(|(executable, entry)| {
                let images = read_images(&folder, &executable)
                    .inspect_err(|error| {
                        tracing::debug!(%error, %executable, "cached icon unavailable");
                    })
                    .ok()?;
                Some((executable, (entry, images)))
            })
            .collect();
        let mut cache = Self {
            folder,
            icons,
            generic: None,
            view: Rc::default(),
        };
        cache.update_view();
        cache
    }

    pub(super) fn view(&self) -> Rc<AppIcons> {
        Rc::clone(&self.view)
    }

    fn update_view(&mut self) {
        self.view = Rc::new(AppIcons {
            apps: self
                .icons
                .iter()
                .map(|(executable, (_, images))| (executable.clone(), images.clone()))
                .collect(),
            generic: self.generic.clone(),
        });
    }

    pub(super) fn set_generic(&mut self, images: IconImages) {
        self.generic = Some(images);
        self.update_view();
    }

    pub(super) fn request(&self, executable: &str, known: Option<&Path>) -> Option<IconRequest> {
        let entry = self.icons.get(executable).map(|(entry, _)| entry);
        let path = known.or_else(|| entry.map(|entry| entry.path.as_path()))?;
        Some(IconRequest {
            file: AppFile {
                executable: executable.to_owned(),
                path: path.to_owned(),
            },
            recorded: entry.filter(|entry| entry.path == path).map(Entry::stamp),
        })
    }

    pub(super) fn apply(&mut self, executable: &str, read: IconRead) {
        match read {
            IconRead::Unchanged | IconRead::Failed => {}
            IconRead::Icon {
                path,
                stamp,
                images,
            } => {
                let entry = Entry {
                    path,
                    size: stamp.size,
                    modified: stamp.modified,
                };
                // A partly written set would load next run with the new
                // stamp and stale images, so the files are removed and the
                // icon is read again then.
                if let Err(error) = write_images(&self.folder, executable, &images) {
                    tracing::warn!(%error, %executable, "writing a cached icon");
                    self.delete_images(executable);
                }
                self.icons.insert(executable.to_owned(), (entry, images));
                self.write_index();
                self.update_view();
            }
            IconRead::NoIcon => {
                if self.icons.remove(executable).is_some() {
                    self.delete_images(executable);
                    self.write_index();
                    self.update_view();
                }
            }
        }
    }

    pub(super) fn prune(&mut self, keep: &BTreeSet<String>) {
        let removed: Vec<String> = self
            .icons
            .keys()
            .filter(|executable| !keep.contains(*executable))
            .cloned()
            .collect();
        for executable in &removed {
            self.icons.remove(executable);
            self.delete_images(executable);
        }
        if !removed.is_empty() {
            tracing::debug!(?removed, "cached icons pruned");
            self.write_index();
            self.update_view();
        }
    }

    fn delete_images(&self, executable: &str) {
        for pixels in ICON_PIXELS {
            let path = image_path(&self.folder, executable, pixels);
            if let Err(error) = fs_err::remove_file(&path)
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!(%error, "deleting a cached icon");
            }
        }
    }

    fn write_index(&self) {
        let index = Index {
            apps: self
                .icons
                .iter()
                .map(|(executable, (entry, _))| (executable.clone(), entry.clone()))
                .collect(),
        };
        // Renaming a complete file over the index keeps an interrupted
        // write from truncating it.
        let written = toml::to_string(&index)
            .map_err(io::Error::other)
            .and_then(|text| {
                fs_err::create_dir_all(&self.folder)?;
                let partial = self.folder.join("index.toml.tmp");
                fs_err::write(&partial, text)?;
                fs_err::rename(partial, self.folder.join("index.toml"))
            });
        if let Err(error) = written {
            tracing::warn!(%error, "writing the icon cache index");
        }
    }
}

fn image_path(folder: &Path, executable: &str, pixels: u16) -> PathBuf {
    folder.join(format!("{executable}-{pixels}.png"))
}

fn read_images(folder: &Path, executable: &str) -> io::Result<IconImages> {
    let [small, medium, large] = ICON_PIXELS
        .map(|pixels| fs_err::read(image_path(folder, executable, pixels)).map(Arc::<[u8]>::from));
    Ok(IconImages([small?, medium?, large?]))
}

fn write_images(folder: &Path, executable: &str, images: &IconImages) -> io::Result<()> {
    fs_err::create_dir_all(folder)?;
    for (pixels, png) in ICON_PIXELS.into_iter().zip(&images.0) {
        fs_err::write(image_path(folder, executable, pixels), png)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
