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
