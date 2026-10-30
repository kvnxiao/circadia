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
    if let Some(path) = known {
        cache.observe(&AppFile {
            executable: executable.to_owned(),
            path: path.into(),
        });
    }
    if let Some(request) = cache.request(executable, known.map(Path::new)) {
        let read = read_icons(&request, files);
        cache.apply(&request.file.identity().expect("valid fixture path"), read);
    }
}

fn shown(cache: &IconCache, executable: &str) -> Option<Vec<u8>> {
    let view = cache.view();
    view.sources
        .get(&ExecutableName::new(executable).expect("fixture filename"))
        .and_then(|path| view.apps.get(path))
        .map(|images| images.at(32).to_vec())
}

const CS2: &str = "C:\\Games\\cs2.exe";

#[test_case::test_case(false; "same_process")]
#[test_case::test_case(true; "reload_with_missing_images")]
fn failed_index_writes_never_reassign_an_old_image_identifier(reload: bool) {
    let directory = tempfile::tempdir().expect("temporary cache");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    files.file(r"C:\A\browser.exe", 100);
    files.file(r"C:\B\browser.exe", 200);
    show(&mut cache, &files, "browser.exe", Some(r"C:\A\browser.exe"));
    let partial = directory.path().join("icons/index.toml.tmp");
    fs_err::create_dir(&partial).expect("block index replacement");
    cache.prune(&BTreeSet::new(), &BTreeSet::new());
    if reload {
        cache = IconCache::load(directory.path());
    }
    files.draw(Icon::Drawn(2));
    show(&mut cache, &files, "browser.exe", Some(r"C:\B\browser.exe"));
    let restarted = IconCache::load(directory.path());
    assert_eq!(shown(&restarted, "browser.exe"), None);
}

#[test]
fn metadata_sources_use_ordinal_filename_identity() {
    let directory = tempfile::tempdir().expect("temporary cache");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    files.file(r"C:\Apps\Σ.exe", 100);
    show(&mut cache, &files, "σ.exe", Some(r"C:\Apps\Σ.exe"));
    assert_eq!(shown(&cache, "σ.exe"), Some(vec![1, 32]));
    assert_eq!(
        shown(&IconCache::load(directory.path()), "Σ.exe"),
        Some(vec![1, 32])
    );
    assert_eq!(shown(&cache, "ς.exe"), None);
    files.file(r"C:\Apps\İ.exe", 101);
    show(&mut cache, &files, "İ.exe", Some(r"C:\Apps\İ.exe"));
    assert_eq!(shown(&cache, "i\u{307}.exe"), None);
}

#[test]
fn same_name_installations_keep_independent_cached_stamps() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    files.file(r"C:\A\browser.exe", 100);
    files.file(r"C:\B\browser.exe", 200);
    show(&mut cache, &files, "browser.exe", Some(r"C:\A\browser.exe"));
    files.draw(Icon::Drawn(2));
    show(&mut cache, &files, "browser.exe", Some(r"C:\B\browser.exe"));
    assert_eq!(
        cache
            .request("browser.exe", Some(Path::new(r"C:\A\browser.exe")))
            .expect("A request")
            .recorded,
        Some(FileStamp {
            size: 100,
            modified: 1000
        })
    );
}

#[test]
fn paths_reload_independently_and_a_late_reply_does_not_change_the_global_source() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(1));
    let mut cache = IconCache::load(directory.path());
    let a = ExecutablePath::new(r"C:\A\browser.exe").expect("A");
    let b = ExecutablePath::new(r"C:\B\browser.exe").expect("B");
    files.file(a.as_str(), 100);
    files.file(b.as_str(), 200);
    let pending_a = cache
        .request("browser.exe", Some(a.as_path()))
        .expect("A request");
    let read_a = read_icons(&pending_a, &files);
    files.draw(Icon::Drawn(2));
    show(&mut cache, &files, "browser.exe", Some(b.as_str()));
    cache.apply(&a, read_a);
    assert_eq!(shown(&cache, "browser.exe"), Some(vec![2, 32]));
    let mut restarted = IconCache::load(directory.path());
    assert_eq!(
        restarted
            .view()
            .apps
            .get(&a)
            .map(|images| images.at(32).to_vec()),
        Some(vec![1, 32])
    );
    assert_eq!(shown(&restarted, "browser.exe"), Some(vec![2, 32]));
    files.delete(a.as_str());
    let request = restarted
        .request("browser.exe", Some(a.as_path()))
        .expect("missing A still identified");
    restarted.apply(&a, read_icons(&request, &files));
    assert_eq!(
        restarted
            .view()
            .apps
            .get(&a)
            .map(|images| images.at(32).to_vec()),
        Some(vec![1, 32])
    );
    restarted.prune(
        &BTreeSet::from([a.clone()]),
        &BTreeSet::from(["browser.exe".into()]),
    );
    assert_eq!(restarted.view().apps.len(), 2);
    restarted.prune(&BTreeSet::from([a.clone()]), &BTreeSet::new());
    assert_eq!(restarted.view().apps.keys().collect::<Vec<_>>(), [&a]);
}

#[test]
fn malformed_cache_ids_and_legacy_indexes_are_cache_misses_without_external_file_access() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let folder = directory.path().join("icons");
    fs_err::create_dir(&folder).expect("cache folder");
    let outside = directory.path().join("outside-32.png");
    fs_err::write(&outside, [7]).expect("sentinel");
    for image in ["\"../outside\"", "0", "4294967295", "-1"] {
        let text = format!(
            "version = 1\nsources = {{}}\n[[apps]]\npath = 'C:\\A\\app.exe'\nimage = {image}\nsize = 1\nmodified = 1\n"
        );
        fs_err::write(folder.join("index.toml"), text).expect("invalid index");
        let mut cache = IconCache::load(directory.path());
        assert_eq!(cache.view().apps.len(), 0);
        cache.prune(&BTreeSet::new(), &BTreeSet::new());
        assert_eq!(fs_err::read(&outside).expect("sentinel survives"), [7]);
    }
    fs_err::write(folder.join("index.toml"), "[apps]\n").expect("legacy index");
    assert_eq!(IconCache::load(directory.path()).view().apps.len(), 0);
}

#[test]
fn a_known_restriction_is_read_after_the_cache_is_deleted() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let files = FakeFiles::new(Icon::Drawn(3));
    files.file(CS2, 100);
    let mut cache = IconCache::load(directory.path());
    let request = cache
        .request("cs2.exe", Some(Path::new(CS2)))
        .expect("restricted source");
    assert_eq!(request.recorded, None);
    cache.apply(
        &request.file.identity().expect("valid path"),
        read_icons(&request, &files),
    );
    assert_eq!(cache.view().apps.len(), 1);
    assert_eq!(cache.view().sources.len(), 0);
}

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
            .get(&ExecutablePath::new(CS2).expect("fixture path"))
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
    assert!(!directory.path().join("icons").join("1-32.png").exists());
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
    cache.prune(&BTreeSet::new(), &BTreeSet::from(["game.exe".to_owned()]));
    assert_eq!(shown(&cache, "cs2.exe"), None);
    assert_eq!(shown(&cache, "game.exe"), Some(vec![1, 32]));
    let icons = directory.path().join("icons");
    assert!(!icons.join("1-32.png").exists());
    assert!(icons.join("2-32.png").exists());
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
        covering: 0,
        removable: false,
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
    let blocked = directory.path().join("icons").join("1-48.png");
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
