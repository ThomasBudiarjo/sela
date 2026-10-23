//! Portable bundles (M1-13): standalone schedules (`.selapackage`) and
//! profile backups (`.selabackup`) arrive from other machines, mail and USB
//! sticks, so the archive reader, the manifest and both import paths are
//! fully untrusted. Seeds are a valid package and a valid backup built here
//! (no retained corpus files yet); mutations are byte-level.
use super::{config, mutated, within};
use crate::background::{Background, ImageRef};
use crate::bundle::{
    self, ExportOptions, PROFILE_LIMITS, SCHEDULE_LIMITS, archive::Archive, manifest,
};
use crate::fonts::Fonts;
use crate::schedule_file::{Item, ScheduleFile};
use crate::storage::{Id, Repository, Section, Song, Version};
use image::ImageEncoder;
use proptest::prelude::*;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const TOKENS: &[&str] = &[
    "PK\u{3}\u{4}",
    "PK\u{1}\u{2}",
    "PK\u{5}\u{6}",
    "../",
    "/",
    "\\",
    "C:",
    "\"version\":2",
    "\"embedded\":true",
    "\u{ff}\u{ff}\u{ff}\u{ff}",
    "\0\0\0\0",
];

fn png() -> Vec<u8> {
    let pixels = [200u8, 100, 50, 255].repeat(4 * 3);
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&pixels, 4, 3, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

fn song() -> Song {
    let bytes = png();
    Song {
        title: "Fuzz Hymn".into(),
        authors: String::new(),
        copyright: String::new(),
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: Vec::new(),
        sections: vec![Section {
            id: crate::arrangement::SectionId(Id([3; 16])),
            label: "Verse".into(),
            lyrics: "synthetic".into(),
            format: Default::default(),
            background: None,
        }],
        master: Some(Background::image(ImageRef {
            name: "Seed.png".into(),
            sha256: sha2::Sha256::digest(&bytes).into(),
        })),
    }
}

/// A valid standalone schedule and a valid profile backup, built once.
fn seeds() -> &'static (Vec<u8>, Vec<u8>) {
    static SEEDS: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
    SEEDS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("p");
        let images = crate::images::images_dir(&profile);
        fs::create_dir_all(&images).unwrap();
        fs::write(images.join("Seed.png"), png()).unwrap();
        let mut repository = Repository::open(&profile.join("library.sqlite")).unwrap();
        repository.save_song(None, song()).unwrap();
        drop(repository);
        let file = ScheduleFile {
            title: "Fuzz".into(),
            items: vec![Item {
                source: Version {
                    id: Id([7; 16]),
                    revision: 1,
                },
                song: song(),
                arrangement: None,
                theme: None,
            }],
            scripture: Vec::new(),
        };
        let package = dir.path().join("seed.selapackage");
        bundle::export_schedule(
            &profile,
            &file,
            &package,
            &Fonts::new(),
            ExportOptions::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let backup = dir.path().join("seed.selabackup");
        bundle::backup_profile(&profile, &backup, &AtomicBool::new(false)).unwrap();
        (fs::read(package).unwrap(), fs::read(backup).unwrap())
    })
}

/// Every file under `root`, relative and `/`-separated.
fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out
}

fn written(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

/// Opening never panics, never writes more than an entry declares and stays
/// within its time bound.
fn check_archive(bytes: &[u8]) {
    let dir = tempfile::tempdir().unwrap();
    let path = written(dir.path(), "x.zip", bytes);
    within(Duration::from_secs(10), "bundle archive", || {
        let Ok(mut archive) = Archive::open(fs::File::open(&path).unwrap(), SCHEDULE_LIMITS) else {
            return;
        };
        for index in 0..archive.entries().len() {
            let declared = archive.entries()[index].size;
            let mut sink = Vec::new();
            let result = archive.extract(index, &mut sink, &|| false);
            assert!(
                sink.len() as u64 <= declared,
                "never past the declared size"
            );
            if let Ok(digest) = result {
                assert_eq!(digest.size, declared);
            }
        }
    });
}

/// A failed import leaves the profile as it was (empty); a successful one
/// only adds images and one schedule file inside it.
fn check_import(bytes: &[u8]) {
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile");
    fs::create_dir_all(&profile).unwrap();
    let package = written(dir.path(), "x.selapackage", bytes);
    let result = within(Duration::from_secs(10), "bundle import", || {
        bundle::import_schedule(&profile, &package, &AtomicBool::new(false))
    });
    let files = tree(&profile);
    match result {
        Ok(summary) => {
            assert!(summary.schedule.starts_with(profile.join("Schedules")));
            for file in &files {
                let image = file
                    .strip_prefix("Resources/Images/")
                    .is_some_and(crate::images::valid_name);
                let schedule = file.starts_with("Schedules/") && file.ends_with(".selaschedule");
                assert!(image || schedule, "unexpected file {file}");
            }
            assert!(crate::schedule_file::read(&summary.schedule).is_ok());
        }
        Err(error) => assert!(files.is_empty(), "{error:?} left {files:?}"),
    }
    let around: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        around.len(),
        2,
        "nothing written beside the profile: {around:?}"
    );
}

/// A failed restore leaves no destination and nothing beside it.
fn check_restore(bytes: &[u8]) {
    let dir = tempfile::tempdir().unwrap();
    let backup = written(dir.path(), "x.selabackup", bytes);
    let destination = dir.path().join("restored");
    let result = within(Duration::from_secs(20), "bundle restore", || {
        bundle::restore_profile(&backup, &destination, &AtomicBool::new(false))
    });
    match result {
        Ok(summary) => {
            assert!(bundle::library_in(&destination).is_file());
            assert!(summary.files >= 1);
            Repository::open(&bundle::library_in(&destination)).unwrap();
        }
        Err(error) => {
            assert!(!destination.exists(), "{error:?}");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "{error:?}");
        }
    }
}

fn manifest_seed() -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = written(dir.path(), "x.zip", &seeds().0);
    let mut archive = Archive::open(fs::File::open(&path).unwrap(), PROFILE_LIMITS).unwrap();
    let index = archive.find(manifest::NAME).unwrap();
    archive.read(index, &|| false).unwrap().0
}

proptest! {
    #![proptest_config(config(0x5e1a_0d01, 96, "tests/fixtures/fuzz/portable-archive.proptest-regressions"))]

    #[test]
    fn mutated_archives_open_or_fail_within_bounds(
        bytes in mutated(vec![seeds().0.clone(), seeds().1.clone()], TOKENS),
    ) {
        check_archive(&bytes);
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0d02, 64, "tests/fixtures/fuzz/portable-import.proptest-regressions"))]

    #[test]
    fn mutated_packages_import_cleanly_or_change_nothing(
        bytes in mutated(vec![seeds().0.clone()], TOKENS),
    ) {
        check_import(&bytes);
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0d03, 32, "tests/fixtures/fuzz/portable-restore.proptest-regressions"))]

    #[test]
    fn mutated_backups_restore_fully_or_not_at_all(
        bytes in mutated(vec![seeds().1.clone()], TOKENS),
    ) {
        check_restore(&bytes);
    }
}

proptest! {
    #![proptest_config(config(0x5e1a_0d04, 256, "tests/fixtures/fuzz/portable-manifest.proptest-regressions"))]

    #[test]
    fn mutated_manifests_decode_or_refuse_and_round_trip(
        bytes in mutated(vec![manifest_seed()], TOKENS),
    ) {
        let decoded = within(Duration::from_secs(5), "manifest decode", || manifest::decode(&bytes));
        if let Ok(manifest) = decoded {
            prop_assert_eq!(manifest::decode(&manifest::encode(&manifest)), Ok(manifest));
        }
    }
}

#[test]
fn the_seeds_themselves_import_and_restore() {
    check_archive(&seeds().0);
    check_import(&seeds().0);
    check_restore(&seeds().1);
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("p");
    fs::create_dir_all(&profile).unwrap();
    let package = written(dir.path(), "x.selapackage", &seeds().0);
    assert!(bundle::import_schedule(&profile, &package, &AtomicBool::new(false)).is_ok());
}
