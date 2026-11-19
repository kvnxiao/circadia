use super::*;
use crate::ui::icon_cache::Entry;
use std::sync::mpsc;

#[test]
fn a_blocked_writer_coalesces_replacements_and_preserves_pending_images() {
    let directory = tempfile::tempdir().expect("temporary cache");
    let shared = Arc::new((Mutex::new(Work::default()), Condvar::new()));
    let state = Arc::clone(&shared);
    let folder = directory.path().to_owned();
    let (release, gate) = mpsc::channel();
    let writer = Writer {
        shared,
        thread: Some(std::thread::spawn(move || {
            gate.recv().expect("release writer");
            run(&folder, &state);
        })),
    };
    for image in 1..=100 {
        writer.submit(
            Index {
                version: 1,
                apps: vec![entry(image)],
                sources: BTreeMap::new(),
                titles: BTreeMap::new(),
            },
            Pending {
                dirty: true,
                images: BTreeMap::from([(image, images(1))]),
            },
        );
    }
    writer.submit(
        Index {
            version: 1,
            apps: vec![entry(100), entry(101)],
            sources: BTreeMap::new(),
            titles: BTreeMap::new(),
        },
        Pending {
            dirty: true,
            images: BTreeMap::from([(101, images(2))]),
        },
    );
    assert_eq!(writer.shared.0.lock().expect("queue").images.len(), 2);
    release.send(()).expect("start disk writes");
    drop(writer);
    let index: Index = toml::from_str(
        &fs_err::read_to_string(directory.path().join("index.toml")).expect("saved index"),
    )
    .expect("valid index");
    assert_eq!(index.apps, [entry(100), entry(101)]);
    assert_eq!(
        super::super::read_images(directory.path(), 100).expect("first retained icon"),
        images(1)
    );
    assert_eq!(
        super::super::read_images(directory.path(), 101).expect("latest icon"),
        images(2)
    );
    assert!(!directory.path().join("1-32.png").exists());
}

fn entry(image: u32) -> Entry {
    Entry {
        path: format!(r"C:\Apps\game{image}.exe"),
        image,
        size: 1,
        modified: 1,
    }
}

fn images(tag: u8) -> IconImages {
    IconImages(std::array::from_fn(|_| Arc::from([tag])))
}

#[test]
fn a_repeated_cache_write_failure_is_logged_once_and_its_recovery_once() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let folder = directory.path().join("icons");
    fs_err::write(&folder, "").expect("block the cache folder with a file");
    let log = crate::log::capture(tracing::Level::INFO, || {
        let writer = Writer::start(folder.clone()).expect("start writer");
        for game in 1..=2 {
            writer.submit(
                sources(game),
                Pending {
                    dirty: true,
                    images: BTreeMap::from([(game, images(1))]),
                },
            );
            wait_until_taken(&writer);
        }
        // Taking the third index means the second write finished, but the
        // third write can still have the blocking file open. Windows does not
        // remove a deleted file's path until its last handle closes, so
        // creating the folder there fails with access denied, a second error
        // kind. Renaming the file frees the path at once.
        writer.submit(sources(3), Pending::default());
        wait_until_taken(&writer);
        fs_err::rename(&folder, directory.path().join("blocker")).expect("free the cache folder");
        writer.submit(sources(4), Pending::default());
    });
    let [failure, recovery] = log.as_slice() else {
        panic!("expected one failure and one recovery record: {log:?}");
    };
    assert!(
        failure.starts_with(
            "WARN circadia_app::ui::icon_cache::persistence: writing the icon cache error="
        ),
        "{failure}"
    );
    assert_eq!(
        recovery,
        "INFO circadia_app::ui::icon_cache::persistence: icon cache writes recovered"
    );
    let index: Index =
        toml::from_str(&fs_err::read_to_string(folder.join("index.toml")).expect("saved index"))
            .expect("valid index");
    assert_eq!(index.sources, sources(4).sources);
}

fn sources(game: u32) -> Index {
    Index {
        version: 1,
        apps: Vec::new(),
        sources: BTreeMap::from([(
            format!("game{game}.exe"),
            format!(r"C:\Apps\game{game}.exe"),
        )]),
        titles: BTreeMap::new(),
    }
}

fn wait_until_taken(writer: &Writer) {
    while writer.shared.0.lock().expect("queue").index.is_some() {
        assert!(
            !writer.thread.as_ref().is_some_and(JoinHandle::is_finished),
            "the writer thread ended before taking the index"
        );
        std::thread::yield_now();
    }
}
