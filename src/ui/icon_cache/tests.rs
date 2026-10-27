use super::*;
use crate::ui::file_info::FileReader;
use crate::ui::file_info::read_icons;
use std::collections::BTreeMap;
use std::io;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Icon {
    Drawn(u8),
    Missing,
    Unreadable,
}

struct FakeFiles {
    stamps: Mutex<BTreeMap<PathBuf, FileStamp>>,
    icon: Mutex<Icon>,
    icon_reads: AtomicUsize,
}

impl FakeFiles {
    fn new(icon: Icon) -> Self {
        Self {
            stamps: Mutex::new(BTreeMap::new()),
            icon: Mutex::new(icon),
            icon_reads: AtomicUsize::new(0),
        }
    }

    fn file(&self, path: &str, size: u64) {
        self.stamps.lock().expect("stamps").insert(
            PathBuf::from(path),
            FileStamp {
                size,
                modified: 1000,
            },
        );
    }

    fn delete(&self, path: &str) {
        self.stamps.lock().expect("stamps").remove(Path::new(path));
    }

    fn draw(&self, icon: Icon) {
        *self.icon.lock().expect("icon") = icon;
    }

    fn reads(&self) -> usize {
        self.icon_reads.load(Ordering::SeqCst)
    }
}

impl FileReader for FakeFiles {
    fn display_name(&self, _path: &Path) -> Option<String> {
        None
    }

    fn stamp(&self, path: &Path) -> io::Result<FileStamp> {
        self.stamps
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
            .get(path)
            .copied()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn icon(&self, _path: &Path, pixels: u16) -> io::Result<Option<Vec<u8>>> {
        self.icon_reads.fetch_add(1, Ordering::SeqCst);
        let icon = *self
            .icon
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        match icon {
            Icon::Drawn(tag) => Ok(Some(vec![
                tag,
                u8::try_from(pixels).map_err(io::Error::other)?,
            ])),
            Icon::Missing => Ok(None),
            Icon::Unreadable => Err(io::ErrorKind::PermissionDenied.into()),
        }
    }

    fn generic_icon(&self, _pixels: u16) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

fn show(cache: &mut IconCache, files: &FakeFiles, executable: &str, known: Option<&str>) {
    if let Some(request) = cache.request(executable, known.map(Path::new)) {
        let read = read_icons(&request, files);
        cache.apply(executable, read);
    }
}

fn shown(cache: &IconCache, executable: &str) -> Option<Vec<u8>> {
    cache
        .view()
        .apps
        .get(executable)
        .map(|images| images.at(32).to_vec())
}

const CS2: &str = "C:\\Games\\cs2.exe";

fn cached_cs2(directory: &Path, files: &FakeFiles) {
    let mut cache = IconCache::load(directory);
    files.file(CS2, 100);
    show(&mut cache, files, "cs2.exe", Some(CS2));
    assert_eq!(shown(&cache, "cs2.exe"), Some(vec![1, 32]));
}

#[test]
fn a_cached_icon_with_equal_path_size_and_time_shows_after_a_restart_without_a_read() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let reads = files.reads();
    files.draw(Icon::Drawn(2));
    let mut restarted = IconCache::load(directory.path());
    assert_eq!(shown(&restarted, "cs2.exe"), Some(vec![1, 32]));
    show(&mut restarted, &files, "cs2.exe", Some(CS2));
    assert_eq!(files.reads(), reads);
    assert_eq!(shown(&restarted, "cs2.exe"), Some(vec![1, 32]));
}

#[test]
fn a_changed_size_reads_the_icon_again_and_replaces_the_cached_one() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let mut cache = IconCache::load(directory.path());
    files.file(CS2, 200);
    files.draw(Icon::Drawn(2));
    show(&mut cache, &files, "cs2.exe", None);
    assert_eq!(shown(&cache, "cs2.exe"), Some(vec![2, 32]));
    let restarted = IconCache::load(directory.path());
    assert_eq!(
        restarted
            .view()
            .apps
            .get("cs2.exe")
            .map(|images| images.at(64).to_vec()),
        Some(vec![2, 64])
    );
}

#[test]
fn a_deleted_old_folder_keeps_the_cached_icon_until_the_app_is_seen_in_a_new_folder() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    let old = "C:\\Discord\\app-1.0\\Discord.exe";
    let new = "C:\\Discord\\app-1.1\\Discord.exe";
    files.file(old, 100);
    show(&mut cache, &files, "discord.exe", Some(old));
    files.delete(old);
    show(&mut cache, &files, "discord.exe", None);
    assert_eq!(shown(&cache, "discord.exe"), Some(vec![1, 32]));
    files.file(new, 120);
    files.draw(Icon::Drawn(3));
    show(&mut cache, &files, "discord.exe", Some(new));
    assert_eq!(shown(&cache, "discord.exe"), Some(vec![3, 32]));
    assert_eq!(
        cache
            .request("discord.exe", None)
            .map(|request| request.file.path),
        Some(PathBuf::from(new))
    );
}

#[test]
fn a_changed_file_without_an_icon_removes_the_cached_icon_and_its_files() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let mut cache = IconCache::load(directory.path());
    files.file(CS2, 300);
    files.draw(Icon::Missing);
    show(&mut cache, &files, "cs2.exe", None);
    assert_eq!(shown(&cache, "cs2.exe"), None);
    assert!(
        !directory
            .path()
            .join("icons")
            .join("cs2.exe-32.png")
            .exists()
    );
    assert_eq!(shown(&IconCache::load(directory.path()), "cs2.exe"), None);
}

#[test]
fn a_failed_read_keeps_the_cached_icon() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let mut cache = IconCache::load(directory.path());
    files.file(CS2, 400);
    files.draw(Icon::Unreadable);
    show(&mut cache, &files, "cs2.exe", None);
    assert_eq!(shown(&cache, "cs2.exe"), Some(vec![1, 32]));
    assert_eq!(
        shown(&IconCache::load(directory.path()), "cs2.exe"),
        Some(vec![1, 32])
    );
}

#[test]
fn an_app_that_is_neither_listed_nor_recent_loses_its_entry_and_files() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    files.file(CS2, 100);
    files.file("C:\\Apps\\game.exe", 100);
    show(&mut cache, &files, "cs2.exe", Some(CS2));
    show(&mut cache, &files, "game.exe", Some("C:\\Apps\\game.exe"));
    cache.prune(&BTreeSet::from(["game.exe".to_owned()]));
    assert_eq!(shown(&cache, "cs2.exe"), None);
    assert_eq!(shown(&cache, "game.exe"), Some(vec![1, 32]));
    let icons = directory.path().join("icons");
    assert!(!icons.join("cs2.exe-32.png").exists());
    assert!(icons.join("game.exe-32.png").exists());
    let restarted = IconCache::load(directory.path());
    assert_eq!(shown(&restarted, "cs2.exe"), None);
    assert_eq!(shown(&restarted, "game.exe"), Some(vec![1, 32]));
}

#[test]
fn a_deleted_icons_folder_shows_no_cached_icon_until_the_app_is_seen_again() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    fs_err::remove_dir_all(directory.path().join("icons")).expect("deleted cache");
    let mut cache = IconCache::load(directory.path());
    assert_eq!(shown(&cache, "cs2.exe"), None);
    assert_eq!(cache.request("cs2.exe", None), None);
    show(&mut cache, &files, "cs2.exe", Some(CS2));
    assert_eq!(shown(&cache, "cs2.exe"), Some(vec![1, 32]));
}

#[test]
fn an_unknown_app_without_a_path_is_not_read() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    assert_eq!(
        IconCache::load(directory.path()).request("game.exe", None),
        None
    );
}

fn recent(path: &str, focus_ended: Option<Duration>) -> crate::RecentApp {
    crate::RecentApp {
        executable: "discord.exe".into(),
        path: PathBuf::from(path),
        focus_ended,
        title: None,
        included: false,
        condition: String::new(),
        excluded: false,
    }
}

#[test]
fn the_latest_known_path_is_the_more_recent_of_focus_and_browse() {
    let focused = recent("C:\\Focused\\Discord.exe", None);
    let earlier = recent("C:\\Earlier\\Discord.exe", Some(Duration::from_secs(10)));
    let browsed = (
        PathBuf::from("C:\\Browsed\\Discord.exe"),
        Duration::from_secs(20),
    );
    assert_eq!(
        latest_path(Some(&focused), Some(&browsed)),
        Some(Path::new("C:\\Focused\\Discord.exe"))
    );
    assert_eq!(
        latest_path(Some(&earlier), Some(&browsed)),
        Some(Path::new("C:\\Browsed\\Discord.exe"))
    );
    assert_eq!(
        latest_path(Some(&earlier), None),
        Some(Path::new("C:\\Earlier\\Discord.exe"))
    );
    assert_eq!(latest_path(None, None), None);
}

#[test]
fn a_partly_written_icon_shows_this_run_and_is_read_again_after_a_restart() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let blocked = directory.path().join("icons").join("cs2.exe-48.png");
    let mut permissions = fs_err::metadata(&blocked)
        .expect("48 px image")
        .permissions();
    permissions.set_readonly(true);
    fs_err::set_permissions(&blocked, permissions).expect("block the 48 px image");
    let mut cache = IconCache::load(directory.path());
    files.file(CS2, 200);
    files.draw(Icon::Drawn(2));
    show(&mut cache, &files, "cs2.exe", Some(CS2));
    assert_eq!(shown(&cache, "cs2.exe"), Some(vec![2, 32]));
    assert!(!blocked.exists());
    let restarted = IconCache::load(directory.path());
    assert_eq!(shown(&restarted, "cs2.exe"), None);
    assert!(
        restarted
            .request("cs2.exe", Some(Path::new(CS2)))
            .is_some_and(|request| request.recorded.is_none()),
        "the restarted cache has no recorded stamp, so the icon is read again"
    );
}

#[test]
fn the_index_is_replaced_whole_without_a_leftover_partial_file() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    cached_cs2(directory.path(), &files);
    let icons = directory.path().join("icons");
    assert!(icons.join("index.toml").is_file());
    assert!(!icons.join("index.toml.tmp").exists());
}
