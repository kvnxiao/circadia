use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AppFile {
    pub(super) executable: String,
    pub(super) path: PathBuf,
}

pub(super) struct FileInfoClient {
    requests: mpsc::Sender<AppFile>,
    replies: mpsc::Receiver<(String, Option<String>)>,
    requested: BTreeSet<String>,
    names: BTreeMap<String, String>,
}

impl FileInfoClient {
    pub(super) fn spawn(
        read: impl Fn(&Path) -> Option<String> + Send + 'static,
        notify: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, received) = mpsc::channel::<AppFile>();
        let (sender, replies) = mpsc::channel();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::Builder::new()
            .name("circadia-file-info".into())
            .spawn(move || {
                let _log = tracing::dispatcher::set_default(&dispatch);
                serve(&received, &sender, &read, &notify);
            })?;
        Ok(Self {
            requests,
            replies,
            requested: BTreeSet::new(),
            names: BTreeMap::new(),
        })
    }

    pub(super) fn request(&mut self, executable: &str, path: &Path) {
        if !self.requested.contains(executable) {
            self.requested.insert(executable.to_owned());
            let _sent = self.requests.send(AppFile {
                executable: executable.to_owned(),
                path: path.to_owned(),
            });
        }
    }

    pub(super) fn receive(&mut self) -> Vec<(String, String)> {
        let names: Vec<(String, String)> = self
            .replies
            .try_iter()
            .filter_map(|(executable, name)| Some((executable, name?)))
            .collect();
        self.names.extend(names.iter().cloned());
        names
    }

    pub(super) fn name(&self, executable: &str) -> Option<&str> {
        self.names.get(executable).map(String::as_str)
    }

    pub(super) fn names(&self) -> &BTreeMap<String, String> {
        &self.names
    }
}

fn serve(
    received: &mpsc::Receiver<AppFile>,
    sender: &mpsc::Sender<(String, Option<String>)>,
    read: &dyn Fn(&Path) -> Option<String>,
    notify: &dyn Fn(),
) {
    for file in received {
        let name = read(&file.path);
        tracing::debug!(
            executable = %file.executable,
            path = %file.path.display(),
            name = name.as_deref(),
            "display name read"
        );
        if sender.send((file.executable, name)).is_err() {
            return;
        }
        notify();
    }
}
