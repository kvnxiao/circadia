use crate::ExecutablePath;
use crate::platform::file_info::FileStamp;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
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

impl AppFile {
    pub(super) fn identity(&self) -> Option<ExecutablePath> {
        let path = ExecutablePath::new(self.path.to_str()?).ok()?;
        crate::same_executable(path.file_name(), &self.executable).then_some(path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AppSelection {
    pub(super) file: AppFile,
    pub(super) restricted: bool,
}

impl From<AppFile> for AppSelection {
    fn from(file: AppFile) -> Self {
        Self {
            file,
            restricted: false,
        }
    }
}

impl AppSelection {
    pub(super) fn entry(&self, name: String) -> crate::AppEntry {
        crate::AppEntry::new(self.file.executable.clone(), name).with_path(
            self.restricted
                .then(|| self.file.path.to_string_lossy().into_owned()),
        )
    }
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

#[derive(Clone)]
enum Request {
    Name(ExecutablePath),
    Icon(ExecutablePath, IconRequest),
    Generic,
}

enum Reply {
    Name(ExecutablePath, Option<String>),
    Icon(ExecutablePath, IconRead),
    Generic(Option<IconImages>),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Received {
    pub(super) names: Vec<(ExecutablePath, String)>,
    pub(super) icons: Vec<(ExecutablePath, IconRead)>,
    pub(super) generic: Option<IconImages>,
}

pub(super) struct FileInfoClient {
    requests: mpsc::SyncSender<Request>,
    replies: mpsc::Receiver<Reply>,
    pending: VecDeque<Request>,
    in_flight: Option<Request>,
    requested: BTreeSet<ExecutablePath>,
    reading_icons: BTreeSet<ExecutablePath>,
    names: BTreeMap<ExecutablePath, String>,
}

impl FileInfoClient {
    pub(super) fn spawn(
        reader: impl FileReader,
        notify: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, received) = mpsc::sync_channel::<Request>(1);
        let (sender, replies) = mpsc::sync_channel(1);
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
            pending: VecDeque::new(),
            in_flight: Some(Request::Generic),
            requested: BTreeSet::new(),
            reading_icons: BTreeSet::new(),
            names: BTreeMap::new(),
        })
    }

    pub(super) fn request(&mut self, executable: &str, path: &Path) {
        let Some(path) = (AppFile {
            executable: executable.to_owned(),
            path: path.to_owned(),
        })
        .identity() else {
            return;
        };
        if self.requested.insert(path.clone())
            && !matches!(&self.in_flight, Some(Request::Name(active)) if active == &path)
        {
            self.pending.push_back(Request::Name(path));
            self.dispatch_next();
        }
    }

    pub(super) fn request_icon(&mut self, request: IconRequest) {
        let Some(path) = request.file.identity() else {
            return;
        };
        if self.reading_icons.insert(path.clone())
            && !matches!(&self.in_flight, Some(Request::Icon(active, _)) if active == &path)
        {
            self.pending.push_back(Request::Icon(path, request));
            self.dispatch_next();
        }
    }

    pub(super) fn receive(&mut self) -> Received {
        let mut received = Received::default();
        if let Ok(reply) = self.replies.try_recv() {
            self.in_flight = None;
            match reply {
                Reply::Name(path, Some(name)) if self.requested.contains(&path) => {
                    received.names.push((path, name));
                }
                Reply::Icon(executable, read) if self.reading_icons.remove(&executable) => {
                    received.icons.push((executable, read));
                }
                Reply::Name(_, _) | Reply::Icon(_, _) => {}
                Reply::Generic(images) => received.generic = images,
            }
        }
        self.names.extend(received.names.iter().cloned());
        self.dispatch_next();
        received
    }

    pub(super) fn retain(&mut self, paths: &BTreeSet<ExecutablePath>) {
        self.requested.retain(|path| paths.contains(path));
        self.reading_icons.retain(|path| paths.contains(path));
        self.names.retain(|path, _| paths.contains(path));
        self.pending.retain(|request| match request {
            Request::Name(path) | Request::Icon(path, _) => paths.contains(path),
            Request::Generic => true,
        });
    }

    fn dispatch_next(&mut self) {
        if self.in_flight.is_none()
            && let Some(request) = self.pending.pop_front()
            && self.requests.send(request.clone()).is_ok()
        {
            self.in_flight = Some(request);
        }
    }

    pub(super) fn name(&self, path: &ExecutablePath) -> Option<&str> {
        self.names.get(path).map(String::as_str)
    }

    pub(super) fn names(&self) -> &BTreeMap<ExecutablePath, String> {
        &self.names
    }
}

fn serve(
    received: &mpsc::Receiver<Request>,
    sender: &mpsc::SyncSender<Reply>,
    reader: &dyn FileReader,
    notify: &dyn Fn(),
) {
    for request in received {
        let reply = match request {
            Request::Name(path) => {
                let name = reader.display_name(path.as_path());
                tracing::debug!(
                    path = path.as_str(),
                    name = name.as_deref(),
                    "display name read"
                );
                Reply::Name(path, name)
            }
            Request::Icon(path, request) => {
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
                Reply::Icon(path, read)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn request_icon(client: &mut FileInfoClient, path: &ExecutablePath) {
        client.request_icon(IconRequest {
            file: AppFile {
                executable: path.file_name().to_owned(),
                path: path.as_path().to_owned(),
            },
            recorded: None,
        });
    }

    #[test]
    fn pending_reads_follow_current_paths_and_discard_obsolete_replies() {
        let (ready, replies) = mpsc::channel();
        let (started, observations) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let mut client = FileInfoClient::spawn(
            Names(move |path: &Path| {
                started.send(path.to_owned()).expect("read observer");
                gate.recv().expect("read gate");
                Some("App".into())
            }),
            move || {
                let _sent = ready.send(());
            },
        )
        .expect("worker");
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("generic reply");
        client.receive();
        let old = ExecutablePath::new(r"C:\Old\app.exe").expect("old path");
        client.request("app.exe", old.as_path());
        assert_eq!(
            observations
                .recv_timeout(Duration::from_secs(5))
                .expect("active read"),
            old.as_path()
        );
        let mut current = old;
        for index in 0..1000 {
            current = ExecutablePath::new(&format!(r"C:\Apps\{index}\app.exe")).expect("path");
            client.retain(&BTreeSet::from([current.clone()]));
            client.request("app.exe", current.as_path());
            request_icon(&mut client, &current);
            assert_eq!(client.pending.len(), 2);
            assert_eq!(client.requested.len(), 1);
            assert_eq!(client.reading_icons.len(), 1);
        }
        release.send(()).expect("release obsolete read");
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("obsolete reply");
        assert_eq!(client.receive(), Received::default());
        assert_eq!(
            observations
                .recv_timeout(Duration::from_secs(5))
                .expect("current read"),
            current.as_path()
        );
        release.send(()).expect("release current read");
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("current reply");
        assert_eq!(client.receive().names, [(current.clone(), "App".into())]);
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("icon reply");
        assert_eq!(client.receive().icons, [(current, IconRead::Failed)]);
        assert!(client.pending.is_empty());
        assert!(client.in_flight.is_none());
        client.retain(&BTreeSet::new());
        assert!(client.names().is_empty());
        assert!(client.requested.is_empty());
    }

    #[test]
    fn every_current_icon_request_completes_through_the_single_worker_slot() {
        let (ready, replies) = mpsc::channel();
        let mut client = FileInfoClient::spawn(Names(|_: &Path| Some("App".into())), move || {
            let _sent = ready.send(());
        })
        .expect("worker");
        let paths: Vec<_> = (0..64)
            .map(|index| ExecutablePath::new(&format!(r"C:\Apps\{index}\app.exe")).expect("path"))
            .collect();
        for path in &paths {
            client.request("app.exe", path.as_path());
            request_icon(&mut client, path);
        }
        let mut icons = Vec::new();
        for _ in 0..=paths.len() * 2 {
            replies
                .recv_timeout(Duration::from_secs(5))
                .expect("pending reply");
            icons.extend(client.receive().icons);
        }
        assert_eq!(
            icons,
            paths
                .iter()
                .cloned()
                .map(|path| (path, IconRead::Failed))
                .collect::<Vec<_>>()
        );
        assert_eq!(client.names().len(), paths.len());
        assert!(client.pending.is_empty());
        assert!(client.in_flight.is_none());
    }

    #[test]
    fn pending_names_with_the_same_filename_are_read_and_returned_by_path() {
        let (ready, received) = mpsc::channel();
        let mut client = FileInfoClient::spawn(
            Names(|path: &Path| Some(path.display().to_string())),
            move || {
                let _sent = ready.send(());
            },
        )
        .expect("worker");
        let a = ExecutablePath::new(r"C:\A\browser.exe").expect("A");
        let b = ExecutablePath::new(r"C:\B\browser.exe").expect("B");
        client.request("browser.exe", a.as_path());
        client.request("browser.exe", b.as_path());
        let mut names = Vec::new();
        for _ in 0..3 {
            received
                .recv_timeout(Duration::from_secs(5))
                .expect("generic and two name replies");
            names.extend(client.receive().names);
        }
        assert_eq!(
            names,
            [
                (a.clone(), r"C:\A\browser.exe".into()),
                (b.clone(), r"C:\B\browser.exe".into())
            ]
        );
        assert_eq!(client.name(&a), Some(r"C:\A\browser.exe"));
        assert_eq!(client.name(&b), Some(r"C:\B\browser.exe"));
    }
}
