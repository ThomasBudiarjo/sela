//! S04 (backlog "Repeatable end-to-end scenarios", M1-13): a bundle made on
//! one offline profile is opened on a clean profile of another "machine"
//! with a different folder layout, a different font set and one asset that
//! never travelled. Preflight must explain each missing dependency, Locate…
//! must repair the asset, and the missing font must be reported as a
//! fallback, never as the exact face. Library-level (I): no GPUI, real
//! SQLite libraries, real files. The operator path is covered by the GPUI
//! tests in `src/tests/bundle_transfer.rs` and `scripts/bundle-x11.py`.
use image::ImageEncoder;
use sela::{
    arrangement::SectionId,
    background::{Background, ImageRef},
    bundle::{self, ExportOptions},
    fonts::{Catalog, Fonts},
    format::SlideFormat,
    images::{self, Substitute},
    preflight,
    schedule_file::{self, Item, ScheduleFile},
    storage::{Repository, Section, Song},
};
use sha2::Digest;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn png(rgb: [u8; 3]) -> Vec<u8> {
    let pixels: Vec<u8> = (0..16 * 9)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
        .collect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&pixels, 16, 9, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fonts(dir: &Path) -> Fonts {
    let fonts = Fonts::new();
    fonts.install_catalog(Catalog::scan([dir.to_path_buf()]));
    fonts
}

fn section(label: &str, font: Option<&str>, image: Option<ImageRef>) -> Section {
    Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: format!("{label}: original synthetic words"),
        format: SlideFormat {
            font: font.map(str::to_owned),
            ..SlideFormat::default()
        },
        background: image.map(Background::image),
    }
}

fn song(title: &str, sections: Vec<Section>) -> Song {
    Song {
        title: title.into(),
        authors: "S04 author".into(),
        copyright: String::new(),
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: Vec::new(),
        sections,
        master: None,
    }
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn s04_transfer_to_a_clean_machine_with_a_missing_font_and_asset() {
    // Machine A: profile in a home folder, DejaVu Sans installed.
    let a = tempfile::tempdir().unwrap();
    let profile_a = a.path().join("home").join("ana").join(".local/share/sela");
    let images_a = images::images_dir(&profile_a);
    fs::create_dir_all(&images_a).unwrap();
    let sunrise = png([230, 150, 30]);
    let lantern = png([30, 60, 200]);
    fs::write(images_a.join("Sunrise.png"), &sunrise).unwrap();
    let lantern_ref = ImageRef {
        name: "Lantern.png".into(),
        sha256: sha2::Sha256::digest(&lantern).into(),
    };
    let sunrise_ref = ImageRef {
        name: "Sunrise.png".into(),
        sha256: sha2::Sha256::digest(&sunrise).into(),
    };
    // Lantern.png was chosen once but is no longer in profile A: the bundle
    // cannot carry it. Its original is on a USB stick.
    let usb = a.path().join("usb");
    fs::create_dir_all(&usb).unwrap();
    fs::write(usb.join("Lantern.png"), &lantern).unwrap();
    let mut library_a = Repository::open(&profile_a.join("library.sqlite")).unwrap();
    let morning = song(
        "Morning Light",
        vec![
            section("Verse 1", Some("DejaVu Sans"), Some(sunrise_ref.clone())),
            section("Chorus", Some("DejaVu Sans"), None),
        ],
    );
    let evening = song(
        "Evening Lantern",
        vec![section("Verse 1", None, Some(lantern_ref.clone()))],
    );
    let v1 = library_a.save_song(None, morning.clone()).unwrap();
    let v2 = library_a.save_song(None, evening.clone()).unwrap();
    let fonts_a = fonts(&fixtures());
    let file = ScheduleFile {
        title: "Sunday S04".into(),
        items: [(v1, morning), (v2, evening)]
            .into_iter()
            .map(|(source, song)| Item {
                source,
                song,
                arrangement: None,
                theme: None,
            })
            .collect(),
        scripture: Vec::new(),
    };
    let package = a.path().join("usb").join("Sunday S04.selapackage");
    let exported = bundle::export_schedule(
        &profile_a,
        &file,
        &package,
        &fonts_a,
        ExportOptions::default(),
        &no(),
    )
    .unwrap();
    assert_eq!(exported.embedded, ["Sunrise.png"]);
    assert_eq!(
        exported.not_embedded,
        [("Lantern.png".to_owned(), Substitute::Missing)]
    );
    assert_eq!(exported.fonts.len(), 1);
    assert_eq!(exported.fonts[0].installed, Some(true), "installed on A");
    let backup = a.path().join("usb").join("Ana.selabackup");
    let backed_up = bundle::backup_profile(&profile_a, &backup, &no()).unwrap();
    drop(library_a);

    // Machine B: a different drive layout, no DejaVu Sans installed (an
    // empty font folder), a fresh library with a song of its own.
    let b = tempfile::tempdir().unwrap();
    let profile_b = b.path().join("D").join("Church Data").join("Sela Profile");
    fs::create_dir_all(&profile_b).unwrap();
    let empty_fonts = b.path().join("fonts");
    fs::create_dir_all(&empty_fonts).unwrap();
    let fonts_b = fonts(&empty_fonts);
    let mut library_b = Repository::open(&profile_b.join("library.sqlite")).unwrap();
    library_b
        .save_song(None, song("Local Song", vec![section("Verse", None, None)]))
        .unwrap();

    // Open the standalone schedule on B (what File › Open Schedule does).
    let imported = bundle::import_schedule(&profile_b, &package, &no()).unwrap();
    assert_eq!(imported.added, ["Sunrise.png"]);
    assert_eq!(imported.not_included, ["Lantern.png"]);
    assert_eq!(imported.fonts[0].family, "DejaVu Sans");
    let opened = schedule_file::read(&imported.schedule).unwrap();
    assert_eq!(opened.title, "Sunday S04");
    let songs: Vec<Song> = opened
        .items
        .iter()
        .map(|item| {
            let version = library_b.import_hidden(item.source, &item.song).unwrap();
            library_b.song(version).unwrap()
        })
        .collect();
    assert_eq!(songs[0].title, "Morning Light");

    // Preflight explains both missing dependencies.
    let report = preflight::check(&profile_b, &songs, &fonts_b, &|| false).unwrap();
    assert!(report.fonts_checked);
    assert_eq!(report.items.len(), 2, "{report:?}");
    assert_eq!(report.items[0].title, "Morning Light");
    assert_eq!(report.items[0].fonts[0].family, "DejaVu Sans");
    assert_eq!(report.items[0].fonts[0].slides, [0, 1]);
    assert!(report.items[0].images.is_empty(), "Sunrise.png travelled");
    assert_eq!(report.items[1].images[0].image, lantern_ref);
    assert_eq!(report.items[1].images[0].problem, Substitute::Missing);
    // No silent substitute claimed exact: the face falls back with a warning.
    let resolved = fonts_b.resolve(&songs[0].sections[0].format);
    assert!(resolved.warning().is_some());

    // Repair: Locate… a file with other bytes is refused; the original works.
    let impostor = b.path().join("Lantern.png");
    fs::write(&impostor, png([1, 1, 1])).unwrap();
    assert_eq!(
        images::relink(&profile_b, &lantern_ref, &impostor, &no()),
        Err(images::Error::Mismatch)
    );
    assert_eq!(
        images::relink(&profile_b, &lantern_ref, &usb.join("Lantern.png"), &no()),
        Ok(images::Relinked::Restored)
    );
    let repaired = preflight::check(&profile_b, &songs, &fonts_b, &|| false).unwrap();
    assert_eq!(repaired.items.len(), 1, "only the font remains");
    assert!(repaired.items[0].images.is_empty());
    // Installing the font clears the last issue.
    let installed = fonts(&fixtures());
    let clean = preflight::check(&profile_b, &songs, &installed, &|| false).unwrap();
    assert!(clean.is_clean(), "{clean:?}");

    // The whole-profile backup restores on B beside its profile, at another
    // path, without touching B's own profile.
    drop(library_b);
    let library_b_before = fs::read(profile_b.join("library.sqlite")).unwrap();
    let destination = profile_b.parent().unwrap().join("Ana restored");
    let restored = bundle::restore_profile(&backup, &destination, &no()).unwrap();
    assert_eq!(restored.files, backed_up.files);
    let library = Repository::open(&bundle::library_in(&destination)).unwrap();
    let titles: Vec<String> = library
        .catalog(None)
        .unwrap()
        .into_iter()
        .map(|(_, title)| title)
        .collect();
    assert_eq!(titles.len(), 2);
    assert!(titles.contains(&"Morning Light".to_owned()));
    assert_eq!(
        images::content_hash(&destination, "Sunrise.png"),
        Ok(sunrise_ref.sha256)
    );
    assert_eq!(
        fs::read(profile_b.join("library.sqlite")).unwrap(),
        library_b_before
    );
}
