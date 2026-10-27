use crate::platform::file_info::FileStamp;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;

pub(super) const ICON_PIXELS: [u16; 3] = [32, 48, 64];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AppFile {
    pub(super) executable: String,
    pub(super) path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IconImages(pub(super) [Arc<[u8]>; ICON_PIXELS.len()]);

impl IconImages {
    pub(super) fn at(&self, pixels: u16) -> &Arc<[u8]> {
        let index = ICON_PIXELS
            .iter()
            .position(|size| *size >= pixels)
            .unwrap_or(ICON_PIXELS.len() - 1);
        let [small, medium, large] = &self.0;
        match index {
            0 => small,
            1 => medium,
            _ => large,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IconRequest {
    pub(super) file: AppFile,
    pub(super) recorded: Option<FileStamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum IconRead {
    Unchanged,
    Icon {
        path: PathBuf,
        stamp: FileStamp,
        images: IconImages,
    },
    NoIcon,
    Failed,
}

pub(super) trait FileReader: Send + 'static {
    fn display_name(&self, path: &Path) -> Option<String>;
    fn stamp(&self, path: &Path) -> io::Result<FileStamp>;
    fn icon(&self, path: &Path, pixels: u16) -> io::Result<Option<Vec<u8>>>;
    fn generic_icon(&self, pixels: u16) -> io::Result<Vec<u8>>;
}

pub(super) struct WindowsFiles;

impl FileReader for WindowsFiles {
    fn display_name(&self, path: &Path) -> Option<String> {
        crate::platform::file_info::display_name(path)
    }

    fn stamp(&self, path: &Path) -> io::Result<FileStamp> {
        crate::platform::file_info::stamp(path)
    }

    fn icon(&self, path: &Path, pixels: u16) -> io::Result<Option<Vec<u8>>> {
        crate::platform::file_info::read_icon(path, pixels)
    }

    fn generic_icon(&self, pixels: u16) -> io::Result<Vec<u8>> {
        crate::platform::file_info::generic_icon(pixels)
    }
}

pub(super) fn read_icons(request: &IconRequest, reader: &dyn FileReader) -> IconRead {
    let path = &request.file.path;
    let stamp = match reader.stamp(path) {
        Ok(stamp) => stamp,
        Err(error) => {
            tracing::debug!(%error, path = %path.display(), "icon file unavailable");
            return IconRead::Failed;
        }
    };
    if request.recorded == Some(stamp) {
        return IconRead::Unchanged;
    }
    let mut images = Vec::with_capacity(ICON_PIXELS.len());
    for pixels in ICON_PIXELS {
        match reader.icon(path, pixels) {
            Ok(Some(png)) => images.push(Arc::from(png)),
            Ok(None) => return IconRead::NoIcon,
            Err(error) => {
                tracing::debug!(%error, path = %path.display(), pixels, "icon read failed");
                return IconRead::Failed;
            }
        }
    }
    images
        .try_into()
        .map_or(IconRead::Failed, |images| IconRead::Icon {
            path: path.clone(),
            stamp,
            images: IconImages(images),
        })
}

fn read_generic(reader: &dyn FileReader) -> Option<IconImages> {
    let images: Vec<Arc<[u8]>> = ICON_PIXELS
        .into_iter()
        .map(|pixels| reader.generic_icon(pixels).map(Arc::from))
        .collect::<io::Result<_>>()
        .inspect_err(|error| tracing::warn!(%error, "reading the generic application icon"))
        .ok()?;
    images.try_into().ok().map(IconImages)
}

enum Request {
    Name(AppFile),
    Icon(IconRequest),
    Generic,
}

enum Reply {
    Name(String, Option<String>),
    Icon(String, IconRead),
    Generic(Option<IconImages>),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Received {
    pub(super) names: Vec<(String, String)>,
    pub(super) icons: Vec<(String, IconRead)>,
    pub(super) generic: Option<IconImages>,
}

pub(super) struct FileInfoClient {
    requests: mpsc::Sender<Request>,
    replies: mpsc::Receiver<Reply>,
    requested: BTreeSet<String>,
    reading_icons: BTreeSet<String>,
    names: BTreeMap<String, String>,
}

impl FileInfoClient {
    pub(super) fn spawn(
        reader: impl FileReader,
        notify: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, received) = mpsc::channel::<Request>();
        let (sender, replies) = mpsc::channel();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::Builder::new()
            .name("circadia-file-info".into())
            .spawn(move || {
                let _log = tracing::dispatcher::set_default(&dispatch);
                serve(&received, &sender, &reader, &notify);
            })?;
        let _sent = requests.send(Request::Generic);
        Ok(Self {
            requests,
            replies,
            requested: BTreeSet::new(),
            reading_icons: BTreeSet::new(),
            names: BTreeMap::new(),
        })
    }

    pub(super) fn request(&mut self, executable: &str, path: &Path) {
        if !self.requested.contains(executable) {
            self.requested.insert(executable.to_owned());
            let _sent = self.requests.send(Request::Name(AppFile {
                executable: executable.to_owned(),
                path: path.to_owned(),
            }));
        }
    }

    pub(super) fn request_icon(&mut self, request: IconRequest) {
        if self.reading_icons.insert(request.file.executable.clone()) {
            let _sent = self.requests.send(Request::Icon(request));
        }
    }

    pub(super) fn receive(&mut self) -> Received {
        let mut received = Received::default();
        for reply in self.replies.try_iter() {
            match reply {
                Reply::Name(executable, Some(name)) => received.names.push((executable, name)),
                Reply::Name(_, None) => {}
                Reply::Icon(executable, read) => {
                    self.reading_icons.remove(&executable);
                    received.icons.push((executable, read));
                }
                Reply::Generic(images) => received.generic = images,
            }
        }
        self.names.extend(received.names.iter().cloned());
        received
    }

    pub(super) fn name(&self, executable: &str) -> Option<&str> {
        self.names.get(executable).map(String::as_str)
    }

    pub(super) fn names(&self) -> &BTreeMap<String, String> {
        &self.names
    }
}

fn serve(
    received: &mpsc::Receiver<Request>,
    sender: &mpsc::Sender<Reply>,
    reader: &dyn FileReader,
    notify: &dyn Fn(),
) {
    for request in received {
        let reply = match request {
            Request::Name(file) => {
                let name = reader.display_name(&file.path);
                tracing::debug!(
                    executable = %file.executable,
                    path = %file.path.display(),
                    name = name.as_deref(),
                    "display name read"
                );
                Reply::Name(file.executable, name)
            }
            Request::Icon(request) => {
                let read = read_icons(&request, reader);
                tracing::debug!(
                    executable = %request.file.executable,
                    path = %request.file.path.display(),
                    read = match &read {
                        IconRead::Unchanged => "unchanged",
                        IconRead::Icon { .. } => "icon",
                        IconRead::NoIcon => "no icon",
                        IconRead::Failed => "failed",
                    },
                    "icon checked"
                );
                Reply::Icon(request.file.executable, read)
            }
            Request::Generic => Reply::Generic(read_generic(reader)),
        };
        if sender.send(reply).is_err() {
            return;
        }
        notify();
    }
}

#[cfg(test)]
pub(super) struct Names<F>(pub(super) F);

#[cfg(test)]
impl<F: Fn(&Path) -> Option<String> + Send + 'static> FileReader for Names<F> {
    fn display_name(&self, path: &Path) -> Option<String> {
        (self.0)(path)
    }

    fn stamp(&self, _path: &Path) -> io::Result<FileStamp> {
        Err(io::ErrorKind::NotFound.into())
    }

    fn icon(&self, _path: &Path, _pixels: u16) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::NotFound.into())
    }

    fn generic_icon(&self, _pixels: u16) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::NotFound.into())
    }
}
