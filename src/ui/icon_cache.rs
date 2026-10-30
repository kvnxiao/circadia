use super::file_info::AppFile;
use super::file_info::ICON_PIXELS;
use super::file_info::IconImages;
use super::file_info::IconRead;
use super::file_info::IconRequest;
use crate::AppEntry;
use crate::ExecutableName;
use crate::ExecutablePath;
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
    path: String,
    image: u32,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u32,
    apps: Vec<Entry>,
    sources: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct AppIcons {
    pub(super) apps: BTreeMap<ExecutablePath, IconImages>,
    pub(super) sources: BTreeMap<ExecutableName, ExecutablePath>,
    pub(super) generic: Option<IconImages>,
}

pub(super) fn source(
    entry: &AppEntry,
    sources: &BTreeMap<ExecutableName, ExecutablePath>,
) -> Option<ExecutablePath> {
    entry.restriction().ok()?.or_else(|| {
        sources
            .get(&ExecutableName::new(&entry.file_name()?).ok()?)
            .cloned()
    })
}

pub(super) struct IconCache {
    folder: PathBuf,
    icons: BTreeMap<ExecutablePath, (Entry, IconImages)>,
    sources: BTreeMap<ExecutableName, ExecutablePath>,
    last_image: u32,
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
        let loaded = fs_err::read_to_string(folder.join("index.toml"))
            .map_err(|error| error.to_string())
            .and_then(|text| toml::from_str::<Index>(&text).map_err(|error| error.to_string()))
            .inspect_err(|error| tracing::debug!(%error, "no icon cache index"))
            .ok()
            .and_then(valid_index);
        let (entries, sources) = loaded.unwrap_or_default();
        let last_image = entries.values().map(|entry| entry.image).max().unwrap_or(0);
        let icons = entries.into_iter().filter_map(|(path, entry)| {
            let images = read_images(&folder, entry.image)
                .inspect_err(|error| tracing::debug!(%error, path = path.as_str(), "cached icon unavailable")).ok()?;
            Some((path, (entry, images)))
        }).collect();
        let mut cache = Self {
            folder,
            icons,
            sources,
            last_image,
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
                .map(|(path, (_, images))| (path.clone(), images.clone()))
                .collect(),
            sources: self.sources.clone(),
            generic: self.generic.clone(),
        });
    }

    pub(super) fn observe(&mut self, file: &AppFile) {
        let Some(path) = file.identity() else {
            return;
        };
        let Ok(name) = ExecutableName::new(&file.executable) else {
            return;
        };
        if self.sources.get(&name) == Some(&path) {
            return;
        }
        self.sources.insert(name, path);
        self.write_index();
        self.update_view();
    }

    pub(super) fn set_generic(&mut self, images: IconImages) {
        self.generic = Some(images);
        self.update_view();
    }

    pub(super) fn request(&self, executable: &str, known: Option<&Path>) -> Option<IconRequest> {
        let path = match known {
            Some(path) => ExecutablePath::new(path.to_str()?).ok()?,
            None => self
                .sources
                .get(&ExecutableName::new(executable).ok()?)?
                .clone(),
        };
        let entry = self.icons.get(&path).map(|(entry, _)| entry);
        Some(IconRequest {
            file: AppFile {
                executable: executable.to_owned(),
                path: path.as_path().to_path_buf(),
            },
            recorded: entry.map(Entry::stamp),
        })
    }

    pub(super) fn apply(&mut self, path: &ExecutablePath, read: IconRead) {
        match read {
            IconRead::Unchanged | IconRead::Failed => {}
            IconRead::Icon {
                path: read_path,
                stamp,
                images,
            } => {
                if read_path
                    .to_str()
                    .and_then(|path| ExecutablePath::new(path).ok())
                    .as_ref()
                    != Some(path)
                {
                    return;
                }
                let Some(image) = self
                    .icons
                    .get(path)
                    .map(|(entry, _)| entry.image)
                    .or_else(|| self.last_image.checked_add(1))
                    .filter(|image| *image < u32::MAX)
                else {
                    tracing::warn!("icon identifier space exhausted");
                    return;
                };
                self.last_image = self.last_image.max(image);
                let entry = Entry {
                    path: path.as_str().to_owned(),
                    image,
                    size: stamp.size,
                    modified: stamp.modified,
                };
                // A partial write cannot be reused with the new file stamp.
                if let Err(error) = write_images(&self.folder, image, &images) {
                    tracing::warn!(%error, path = path.as_str(), "writing a cached icon");
                    self.delete_images(image);
                }
                self.icons.insert(path.clone(), (entry, images));
                self.write_index();
                self.update_view();
            }
            IconRead::NoIcon => {
                if let Some((entry, _)) = self.icons.remove(path) {
                    self.delete_images(entry.image);
                    self.write_index();
                    self.update_view();
                }
            }
        }
    }

    pub(super) fn prune(&mut self, keep: &BTreeSet<ExecutablePath>, globals: &BTreeSet<String>) {
        let before = self.sources.len();
        self.sources.retain(|name, path| {
            globals
                .iter()
                .any(|global| crate::same_executable(name.as_str(), global))
                || keep.contains(path)
        });
        let referenced: BTreeSet<&ExecutablePath> =
            keep.iter().chain(self.sources.values()).collect();
        let removed: Vec<ExecutablePath> = self
            .icons
            .keys()
            .filter(|path| !referenced.contains(path))
            .cloned()
            .collect();
        for path in &removed {
            if let Some((entry, _)) = self.icons.remove(path) {
                self.delete_images(entry.image);
            }
        }
        if !removed.is_empty() || before != self.sources.len() {
            self.write_index();
            self.update_view();
        }
    }

    fn delete_images(&self, image: u32) {
        for pixels in ICON_PIXELS {
            if let Err(error) = fs_err::remove_file(image_path(&self.folder, image, pixels))
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!(%error, "deleting a cached icon");
            }
        }
    }

    fn write_index(&self) {
        let index = Index {
            version: 1,
            apps: self
                .icons
                .values()
                .map(|(entry, _)| entry.clone())
                .collect(),
            sources: self
                .sources
                .iter()
                .map(|(name, path)| (name.as_str().to_owned(), path.as_str().to_owned()))
                .collect(),
        };
        // Renaming a complete index avoids exposing a partial write.
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

fn valid_index(
    index: Index,
) -> Option<(
    BTreeMap<ExecutablePath, Entry>,
    BTreeMap<ExecutableName, ExecutablePath>,
)> {
    if index.version != 1 {
        return None;
    }
    let mut ids = BTreeSet::new();
    let mut entries = BTreeMap::new();
    for entry in index.apps {
        if entry.image == 0 || entry.image == u32::MAX || !ids.insert(entry.image) {
            return None;
        }
        let path = ExecutablePath::new(&entry.path).ok()?;
        if entries.insert(path, entry).is_some() {
            return None;
        }
    }
    let mut sources = BTreeMap::new();
    for (name, path) in index.sources {
        let path = ExecutablePath::new(&path).ok()?;
        let name = ExecutableName::new(&name).ok()?;
        if ExecutableName::new(path.file_name()).ok()? != name
            || sources.insert(name, path).is_some()
        {
            return None;
        }
    }
    Some((entries, sources))
}

fn image_path(folder: &Path, image: u32, pixels: u16) -> PathBuf {
    folder.join(format!("{image}-{pixels}.png"))
}
fn read_images(folder: &Path, image: u32) -> io::Result<IconImages> {
    let [small, medium, large] = ICON_PIXELS
        .map(|pixels| fs_err::read(image_path(folder, image, pixels)).map(Arc::<[u8]>::from));
    Ok(IconImages([small?, medium?, large?]))
}
fn write_images(folder: &Path, image: u32, images: &IconImages) -> io::Result<()> {
    fs_err::create_dir_all(folder)?;
    for (pixels, png) in ICON_PIXELS.into_iter().zip(&images.0) {
        fs_err::write(image_path(folder, image, pixels), png)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
