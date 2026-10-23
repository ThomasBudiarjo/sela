//! U/I tests for bundles: the strict ZIP subset against hostile archives,
//! the manifest, and export/import/backup/restore on temporary profiles.
//! Hostile archives are built byte by byte here, not by the writer under
//! test.
use super::archive::{Archive, ArchiveError, Limits, Writer, safe_name};
use super::manifest::{self, Kind, Manifest, ManifestError, Role};
use super::*;
use crate::arrangement::{Occurrence, OccurrenceId, SectionId, Variant, VariantId};
use crate::background::Background;
use crate::schedule_file::{Item, ScriptureItem};
use crate::scripture::{Book, Snapshot, VerseId};
use crate::storage::{Id, Section, Song, Version};
use image::ImageEncoder;
use sha2::{Digest, Sha256};

const LIMITS: Limits = Limits {
    entries: 64,
    entry_bytes: 1024 * 1024,
    total_bytes: 4 * 1024 * 1024,
};

fn png(rgb: [u8; 3]) -> Vec<u8> {
    let (w, h) = (8u32, 4u32);
    let pixels: Vec<u8> = (0..w * h)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
        .collect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&pixels, w, h, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

fn sha(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_sha(bytes: &[u8]) -> String {
    manifest::hex(&sha(bytes))
}

fn put_image(profile: &Path, name: &str, bytes: &[u8]) -> ImageRef {
    let dir = images::images_dir(profile);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), bytes).unwrap();
    ImageRef {
        name: name.into(),
        sha256: sha(bytes),
    }
}

fn song(title: &str, image: Option<ImageRef>, font: Option<&str>) -> Song {
    let section = SectionId(Id([1; 16]));
    Song {
        title: title.into(),
        authors: "Original author".into(),
        copyright: "© test".into(),
        license: String::new(),
        administrator: String::new(),
        song_number: String::new(),
        variants: vec![Variant {
            id: VariantId(Id([9; 16])),
            name: "Default".into(),
            occurrences: vec![Occurrence {
                id: OccurrenceId(Id([8; 16])),
                section,
            }],
        }],
        sections: vec![Section {
            id: section,
            label: "Verse 1".into(),
            lyrics: format!("{title} line one\nline two"),
            format: SlideFormat {
                font: font.map(str::to_owned),
                ..SlideFormat::default()
            },
            background: None,
        }],
        master: image.map(Background::image),
    }
}

fn item(n: u8, song: Song) -> Item {
    Item {
        source: Version {
            id: Id([n; 16]),
            revision: 1,
        },
        song,
        arrangement: None,
        theme: None,
    }
}

fn verses(translation: &str, redistributable: bool, position: usize) -> ScriptureItem {
    let john = Book::from_number(43).unwrap();
    ScriptureItem {
        position,
        title: format!("John 3:16 ({translation})"),
        snapshot: Snapshot {
            translation: translation.into(),
            name: "Synthetic".into(),
            language: "en".into(),
            redistributable,
            verses: vec![(VerseId::new(john, 3, 16).unwrap(), "Synthetic verse".into())],
        },
    }
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

/// Sorted relative paths of every file under `root`.
fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
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
    out.sort();
    out
}

/// Files and their bytes under `root`, for before/after comparisons.
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    tree(root)
        .into_iter()
        .map(|rel| {
            let bytes = fs::read(under(root, &rel)).unwrap();
            (rel, bytes)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// A raw ZIP builder for hostile archives.

#[derive(Clone)]
struct Raw {
    name: Vec<u8>,
    /// Bytes as stored in the archive (already compressed for method 8).
    data: Vec<u8>,
    method: u16,
    crc: u32,
    size: u32,
    flags: u16,
    made_by: u16,
    external: u32,
    /// Name written in the local header, when it should differ.
    local_name: Option<Vec<u8>>,
    /// Central record only, pointing at entry k's local header.
    alias: Option<usize>,
    extra: Vec<u8>,
}

fn raw(name: &str, data: &[u8]) -> Raw {
    Raw {
        name: name.as_bytes().to_vec(),
        data: data.to_vec(),
        method: 0,
        crc: crc32fast::hash(data),
        size: data.len() as u32,
        flags: 1 << 11,
        made_by: (3 << 8) | 20,
        external: 0o100_644 << 16,
        local_name: None,
        alias: None,
        extra: Vec::new(),
    }
}

fn deflated(name: &str, plain: &[u8]) -> Raw {
    let mut entry = raw(name, plain);
    entry.data = miniz_oxide::deflate::compress_to_vec(plain, 6);
    entry.method = 8;
    entry
}

fn build(entries: &[Raw], trailing: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets = Vec::new();
    for e in entries {
        if e.alias.is_some() {
            offsets.push(0);
            continue;
        }
        offsets.push(out.len() as u32);
        let name = e.local_name.as_ref().unwrap_or(&e.name);
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend(20u16.to_le_bytes());
        out.extend(e.flags.to_le_bytes());
        out.extend(e.method.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(e.crc.to_le_bytes());
        out.extend((e.data.len() as u32).to_le_bytes());
        out.extend(e.size.to_le_bytes());
        out.extend((name.len() as u16).to_le_bytes());
        out.extend((e.extra.len() as u16).to_le_bytes());
        out.extend(name);
        out.extend(&e.extra);
        out.extend(&e.data);
    }
    let cd_offset = out.len() as u32;
    for (i, e) in entries.iter().enumerate() {
        let offset = match e.alias {
            Some(k) => offsets[k],
            None => offsets[i],
        };
        let compressed = match e.alias {
            Some(k) => entries[k].data.len(),
            None => e.data.len(),
        } as u32;
        out.extend(0x0201_4b50u32.to_le_bytes());
        out.extend(e.made_by.to_le_bytes());
        out.extend(20u16.to_le_bytes());
        out.extend(e.flags.to_le_bytes());
        out.extend(e.method.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(e.crc.to_le_bytes());
        out.extend(compressed.to_le_bytes());
        out.extend(e.size.to_le_bytes());
        out.extend((e.name.len() as u16).to_le_bytes());
        out.extend((e.extra.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(e.external.to_le_bytes());
        out.extend(offset.to_le_bytes());
        out.extend(&e.name);
        out.extend(&e.extra);
    }
    let cd_size = out.len() as u32 - cd_offset;
    out.extend(trailing);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend(cd_size.to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

fn open_raw(bytes: &[u8], limits: Limits) -> Result<Archive, ArchiveError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.zip");
    fs::write(&path, bytes).unwrap();
    Archive::open(File::open(&path).unwrap(), limits)
}

fn extract_all(bytes: &[u8], limits: Limits) -> Result<Vec<Vec<u8>>, ArchiveError> {
    let mut archive = open_raw(bytes, limits)?;
    (0..archive.entries().len())
        .map(|i| archive.read(i, &|| false).map(|(b, _)| b))
        .collect()
}

#[test]
fn safe_names_stay_inside_and_mean_the_same_everywhere() {
    for good in [
        "manifest.json",
        "images/Sunrise (2).png",
        "Schedules/Évening/Sunday.selaschedule",
        "Resources/Images/日本.jpg",
        "a/b/c/d/e/f/g/h",
    ] {
        assert!(safe_name(good), "{good}");
    }
    let long = "x".repeat(241);
    for bad in [
        "",
        "/etc/passwd",
        "../x",
        "a/../b",
        "a/./b",
        "a//b",
        "a/",
        "C:/x",
        "c:x",
        "a\\..\\b",
        "CON",
        "con.txt",
        "images/NUL.png",
        "Com1",
        "a.",
        "a ",
        " a",
        "x\u{0}y",
        "x\ny",
        "a*b",
        "a?b",
        "a<b",
        "a|b",
        "a\"b",
        "a/b/c/d/e/f/g/h/i",
        long.as_str(),
    ] {
        assert!(!safe_name(bad), "{bad:?}");
    }
}

#[test]
fn written_archives_round_trip_stored_and_deflated_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.zip");
    // Larger than one copy chunk and compressible, plus incompressible bytes.
    let text: Vec<u8> = (0..700_000u32)
        .map(|i| b"abcab"[(i % 5) as usize])
        .collect();
    let noise: Vec<u8> = (0..300_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let mut writer = Writer::new(File::create(&path).unwrap());
    let a = writer.add_bytes("text.txt", true, &text).unwrap();
    let b = writer.add_bytes("dir/noise.bin", false, &noise).unwrap();
    let c = writer.add_bytes("empty", true, &[]).unwrap();
    writer.finish().unwrap();
    assert_eq!(a.sha256, sha(&text));
    assert_eq!(b.size, noise.len() as u64);
    assert_eq!(c.size, 0);
    assert!(
        fs::metadata(&path).unwrap().len() < 400_000,
        "the text entry is compressed"
    );
    let mut archive = Archive::open(File::open(&path).unwrap(), LIMITS).unwrap();
    let names: Vec<_> = archive.entries().iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, ["text.txt", "dir/noise.bin", "empty"]);
    let (back, digest) = archive.read(0, &|| false).unwrap();
    assert_eq!(back, text);
    assert_eq!(digest.sha256, a.sha256);
    assert_eq!(archive.read(1, &|| false).unwrap().0, noise);
    assert!(archive.read(2, &|| false).unwrap().0.is_empty());
}

#[test]
fn the_writer_refuses_unsafe_or_colliding_names_and_oversized_sources() {
    let dir = tempfile::tempdir().unwrap();
    let mut writer = Writer::new(File::create(dir.path().join("a.zip")).unwrap());
    assert!(matches!(
        writer.add_bytes("../evil", false, b"x"),
        Err(ArchiveError::UnsafeName(_))
    ));
    writer.add_bytes("Images/A.png", false, b"x").unwrap();
    assert!(matches!(
        writer.add_bytes("images/a.PNG", false, b"x"),
        Err(ArchiveError::DuplicateName(_))
    ));
    assert!(matches!(
        writer.add_bytes("Images/A.png/inner", false, b"x"),
        Err(ArchiveError::DuplicateName(_))
    ));
    let mut big: &[u8] = &[0u8; 100];
    assert!(matches!(
        writer.add("big", true, &mut big, 99, &|| false),
        Err(ArchiveError::TooLarge(_))
    ));
}

#[test]
fn hostile_names_links_and_folders_are_refused() {
    for name in ["../escape.txt", "/abs.txt", "C:/win.txt", "a\\..\\b", "CON"] {
        let bytes = build(&[raw(name, b"x")], &[]);
        assert!(
            matches!(open_raw(&bytes, LIMITS), Err(ArchiveError::UnsafeName(_))),
            "{name}"
        );
    }
    let mut link = raw("link", b"/etc/passwd");
    link.external = 0o120_777 << 16;
    assert!(matches!(
        open_raw(&build(&[link], &[]), LIMITS),
        Err(ArchiveError::Unsupported(_))
    ));
    // Permission bits without a file type (Python's zipfile) are a file.
    let mut plain = raw("plain", b"x");
    plain.external = 0o600 << 16;
    assert!(open_raw(&build(&[plain], &[]), LIMITS).is_ok());
    let mut folder = raw("folder", b"");
    folder.external = 0x10;
    folder.made_by = 20;
    assert!(matches!(
        open_raw(&build(&[folder], &[]), LIMITS),
        Err(ArchiveError::Unsupported(_))
    ));
    // A CP437 (no UTF-8 flag) name with high bytes is ambiguous.
    let mut cp437 = raw("x", b"x");
    cp437.name = vec![b'a', 0x82];
    cp437.flags = 0;
    assert!(matches!(
        open_raw(&build(&[cp437], &[]), LIMITS),
        Err(ArchiveError::UnsafeName(_))
    ));
}

#[test]
fn duplicate_and_conflicting_names_are_refused() {
    let cases = [
        vec![raw("a.txt", b"1"), raw("a.txt", b"2")],
        vec![raw("Readme.TXT", b"1"), raw("readme.txt", b"2")],
        vec![raw("a", b"1"), raw("a/b", b"2")],
        vec![raw("A/b", b"1"), raw("a", b"2")],
    ];
    for entries in cases {
        assert!(matches!(
            open_raw(&build(&entries, &[]), LIMITS),
            Err(ArchiveError::DuplicateName(_))
        ));
    }
}

#[test]
fn unsupported_zip_features_are_refused() {
    let mut encrypted = raw("a", b"x");
    encrypted.flags |= 1;
    let mut descriptor = raw("a", b"x");
    descriptor.flags |= 1 << 3;
    let mut bzip2 = raw("a", b"x");
    bzip2.method = 12;
    let mut zip64 = raw("a", b"x");
    zip64.extra = vec![1, 0, 0, 0];
    for entry in [encrypted, descriptor, bzip2, zip64] {
        assert!(matches!(
            open_raw(&build(&[entry], &[]), LIMITS),
            Err(ArchiveError::Unsupported(_))
        ));
    }
    let mut cut = raw("a", b"x");
    cut.extra = vec![9, 9, 4, 0, 1];
    assert!(matches!(
        open_raw(&build(&[cut], &[]), LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
}

#[test]
fn structural_damage_is_refused() {
    assert_eq!(
        open_raw(b"just some text that is long enough", LIMITS).err(),
        Some(ArchiveError::NotArchive)
    );
    // Bytes between the central directory and the end record.
    let gap = build(&[raw("a", b"x")], b"junk");
    assert!(matches!(
        open_raw(&gap, LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
    // Truncated: the end record is gone.
    let whole = build(&[raw("a", b"hello")], &[]);
    assert_eq!(
        open_raw(&whole[..whole.len() - 3], LIMITS).err(),
        Some(ArchiveError::NotArchive)
    );
    // The local header names another file than the central record.
    let mut renamed = raw("a.txt", b"x");
    renamed.local_name = Some(b"b.txt".to_vec());
    assert!(matches!(
        open_raw(&build(&[renamed], &[]), LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
    // Stored entries cannot have two sizes.
    let mut sizes = raw("a", b"xyz");
    sizes.size = 2;
    assert!(matches!(
        open_raw(&build(&[sizes], &[]), LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
}

#[test]
fn overlapping_entries_cannot_reuse_one_payload() {
    let mut alias = raw("b.txt", b"x");
    alias.alias = Some(0);
    let bytes = build(&[raw("a.txt", b"x"), alias], &[]);
    // The local header names a.txt, so the alias is refused either way.
    assert!(matches!(
        open_raw(&bytes, LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
    // Same name in the alias: a duplicate as well as a shared payload.
    let mut same = raw("a.txt", b"x");
    same.alias = Some(0);
    let bytes = build(&[raw("a.txt", b"x"), same], &[]);
    assert!(open_raw(&bytes, LIMITS).is_err());
}

#[test]
fn declared_sizes_are_bounded_before_reading() {
    let entry = raw("big", &[7u8; 2000]);
    let small = Limits {
        entries: 4,
        entry_bytes: 1000,
        total_bytes: 10_000,
    };
    assert_eq!(
        open_raw(&build(std::slice::from_ref(&entry), &[]), small).err(),
        Some(ArchiveError::TooLarge("big".into()))
    );
    let total = Limits {
        entries: 4,
        entry_bytes: 5000,
        total_bytes: 3000,
    };
    let two = [entry, raw("big2", &[1u8; 2000])];
    assert_eq!(
        open_raw(&build(&two, &[]), total).err(),
        Some(ArchiveError::TooLarge(String::new()))
    );
    let few = Limits {
        entries: 1,
        ..LIMITS
    };
    assert_eq!(
        open_raw(&build(&two, &[]), few).err(),
        Some(ArchiveError::TooManyEntries)
    );
}

#[test]
fn a_deflate_bomb_stops_at_its_declared_size() {
    // 64 MiB of zeros compress to about 64 KiB; it declares 1000 bytes.
    let plain = vec![0u8; 64 * 1024 * 1024];
    let mut bomb = deflated("bomb", &plain);
    drop(plain);
    bomb.size = 1000;
    let bytes = build(&[bomb], &[]);
    let started = std::time::Instant::now();
    let mut archive = open_raw(&bytes, LIMITS).unwrap();
    let mut sink = Vec::new();
    assert_eq!(
        archive.extract(0, &mut sink, &|| false).err(),
        Some(ArchiveError::TooLarge("bomb".into()))
    );
    assert!(
        sink.len() <= 1000,
        "nothing past the declared size is written"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn checksums_and_compressed_data_are_verified() {
    let mut wrong_crc = raw("a", b"hello");
    wrong_crc.crc ^= 1;
    assert_eq!(
        extract_all(&build(&[wrong_crc], &[]), LIMITS).err(),
        Some(ArchiveError::Checksum("a".into()))
    );
    let mut short = deflated("a", b"hello hello hello");
    short.size += 5;
    assert!(matches!(
        extract_all(&build(&[short], &[]), LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
    let mut damaged = deflated("a", &[b'z'; 5000]);
    damaged.data.truncate(damaged.data.len() / 2);
    assert!(matches!(
        extract_all(&build(&[damaged], &[]), LIMITS),
        Err(ArchiveError::Malformed(_))
    ));
    let mut garbage = deflated("a", b"x");
    garbage.data = vec![0xff; 32];
    assert!(extract_all(&build(&[garbage], &[]), LIMITS).is_err());
    let good = [raw("a", b"stored"), deflated("b", &vec![b'q'; 300_000])];
    let out = extract_all(&build(&good, &[]), LIMITS).unwrap();
    assert_eq!(out[0], b"stored");
    assert_eq!(out[1], vec![b'q'; 300_000]);
}

// ---------------------------------------------------------------------------
// Manifest.

fn manifest_value(manifest: &Manifest) -> serde_json::Value {
    serde_json::from_slice(&manifest::encode(manifest)).unwrap()
}

fn sample_manifest() -> Manifest {
    let image = png([1, 2, 3]);
    Manifest {
        kind: Kind::Schedule,
        generator: "Sela test".into(),
        title: "Sunday".into(),
        files: vec![
            manifest::FileRecord {
                path: manifest::SCHEDULE_PATH.into(),
                size: 10,
                sha256: [1; 32],
                role: Role::Schedule,
            },
            manifest::FileRecord {
                path: "images/Sunrise.png".into(),
                size: image.len() as u64,
                sha256: sha(&image),
                role: Role::Image,
            },
        ],
        images: vec![
            manifest::ImageRecord {
                name: "Sunrise.png".into(),
                sha256: sha(&image),
                embedded: true,
            },
            manifest::ImageRecord {
                name: "Gone.png".into(),
                sha256: [3; 32],
                embedded: false,
            },
        ],
        fonts: vec![FontRecord {
            family: "Synthetic Serif".into(),
            installed: Some(false),
        }],
        omitted: vec![Omitted {
            title: "John 3:16 (RST)".into(),
            translation: "RST".into(),
        }],
        library_schema: None,
    }
}

#[test]
fn manifests_round_trip_and_unknown_fields_are_ignored() {
    let original = sample_manifest();
    assert_eq!(
        manifest::decode(&manifest::encode(&original)).unwrap(),
        original
    );
    let mut value = manifest_value(&original);
    value["future_field"] = serde_json::json!({"x": 1});
    assert_eq!(
        manifest::decode(&serde_json::to_vec(&value).unwrap()).unwrap(),
        original
    );
    let text = String::from_utf8(manifest::encode(&original)).unwrap();
    assert!(text.contains("\"format\": \"sela-bundle\""));
    assert!(
        text.contains("\"embedded\": false"),
        "fonts say they are not embedded"
    );
}

#[test]
fn malformed_or_newer_manifests_are_refused() {
    let original = sample_manifest();
    let decode = |value: serde_json::Value| manifest::decode(&serde_json::to_vec(&value).unwrap());
    let mut newer = manifest_value(&original);
    newer["version"] = 2.into();
    assert_eq!(decode(newer), Err(ManifestError::Newer(2)));
    let mut format = manifest_value(&original);
    format["format"] = "zip".into();
    assert!(matches!(decode(format), Err(ManifestError::Malformed(_))));
    let mut traversal = manifest_value(&original);
    traversal["files"][1]["path"] = "images/../../evil.png".into();
    assert!(matches!(decode(traversal), Err(ManifestError::Path(_))));
    let mut wrong_place = manifest_value(&original);
    wrong_place["files"][1]["path"] = "Resources/Images/Sunrise.png".into();
    assert!(matches!(decode(wrong_place), Err(ManifestError::Path(_))));
    let mut duplicate = manifest_value(&original);
    duplicate["files"][1]["path"] = "schedule.SELASCHEDULE".into();
    duplicate["files"][1]["role"] = "schedule".into();
    assert!(decode(duplicate).is_err());
    let mut bad_sha = manifest_value(&original);
    bad_sha["files"][0]["sha256"] = "abc".into();
    assert!(matches!(decode(bad_sha), Err(ManifestError::Malformed(_))));
    let mut unlisted = manifest_value(&original);
    unlisted["images"][0]["embedded"] = false.into();
    assert!(
        matches!(decode(unlisted), Err(ManifestError::Path(_))),
        "an image file no image record describes"
    );
    let mut font = manifest_value(&original);
    font["fonts"][0]["embedded"] = true.into();
    assert!(matches!(decode(font), Err(ManifestError::Malformed(_))));
    let mut no_schedule = manifest_value(&original);
    no_schedule["files"].as_array_mut().unwrap().remove(0);
    assert!(matches!(
        decode(no_schedule),
        Err(ManifestError::Malformed(_))
    ));
    assert!(manifest::decode(b"not json").is_err());
    assert!(manifest::decode(&vec![b' '; manifest::MAX_BYTES + 1]).is_err());
}

#[test]
fn manifest_paths_follow_the_profile_layout() {
    use manifest::allowed;
    assert!(allowed(
        Kind::Profile,
        Role::Image,
        "Resources/Images/A.png"
    ));
    assert!(!allowed(
        Kind::Profile,
        Role::Image,
        "Resources/Images/a/A.png"
    ));
    assert!(!allowed(
        Kind::Profile,
        Role::Image,
        "Resources/Images/A.gif"
    ));
    assert!(allowed(
        Kind::Profile,
        Role::Schedule,
        "Schedules/x/y.selaschedule"
    ));
    assert!(!allowed(Kind::Profile, Role::Schedule, "Schedules/y.txt"));
    assert!(!allowed(Kind::Profile, Role::Library, "Library.sqlite"));
    assert!(allowed(
        Kind::Profile,
        Role::Scripture,
        "Scripture/NIV.selabible"
    ));
    assert!(!allowed(
        Kind::Profile,
        Role::Scripture,
        "Scripture/.selabible"
    ));
    assert!(!allowed(Kind::Schedule, Role::Library, "library.sqlite"));
    assert!(!allowed(Kind::Schedule, Role::Image, "images/../x.png"));
}

// ---------------------------------------------------------------------------
// Standalone schedules.

struct Machine {
    _dir: tempfile::TempDir,
    profile: PathBuf,
}

/// A profile at a machine-specific place, as two machines would have.
fn machine(depth: &[&str]) -> Machine {
    let dir = tempfile::tempdir().unwrap();
    let profile = depth
        .iter()
        .fold(dir.path().to_path_buf(), |p, part| p.join(part));
    fs::create_dir_all(&profile).unwrap();
    Machine { _dir: dir, profile }
}

fn fonts_with(catalog: Option<&Path>) -> Fonts {
    let fonts = Fonts::new();
    if let Some(dir) = catalog {
        fonts.install_catalog(crate::fonts::Catalog::scan([dir.to_path_buf()]));
    }
    fonts
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn sunday(source: &Machine) -> (ScheduleFile, ImageRef, ImageRef) {
    let sunrise = put_image(&source.profile, "Sunrise.png", &png([200, 120, 40]));
    let missing = ImageRef {
        name: "Gone.png".into(),
        sha256: sha(&png([9, 9, 9])),
    };
    let mut themed = item(2, song("Harbor Light", None, None));
    let mut theme = crate::theme::Theme::new("Evening");
    theme.background = Some(Background::image(sunrise.clone()));
    theme.format.font = Some("Synthetic Missing Serif".into());
    themed.theme = Some(crate::schedule::ItemTheme {
        theme,
        explicit: true,
    });
    let file = ScheduleFile {
        title: "Sunday Service".into(),
        items: vec![
            item(
                1,
                song("Morning Song", Some(sunrise.clone()), Some("DejaVu Sans")),
            ),
            themed,
            item(3, song("Faded Hymn", Some(missing.clone()), None)),
        ],
        scripture: vec![
            verses("SYN", true, 1),
            verses("RESTRICTED", false, 3),
            verses("SYN", true, 5),
        ],
    };
    (file, sunrise, missing)
}

fn export(source: &Machine, file: &ScheduleFile, package: &Path) -> ExportSummary {
    export_schedule(
        &source.profile,
        file,
        package,
        &Fonts::new(),
        ExportOptions::default(),
        &no(),
    )
    .unwrap()
}

#[test]
fn a_standalone_schedule_round_trips_into_a_profile_on_another_machine() {
    let source = machine(&["home", "ana", "sela"]);
    let target = machine(&["D", "Church Data", "Profiles", "sela"]);
    let (file, sunrise, missing) = sunday(&source);
    let package = source.profile.join("Sunday.selapackage");
    let fonts = fonts_with(Some(&fixtures()));
    let summary = export_schedule(
        &source.profile,
        &file,
        &package,
        &fonts,
        ExportOptions::default(),
        &no(),
    )
    .unwrap();
    assert_eq!(summary.items, 5, "three songs and two shareable passages");
    assert_eq!(summary.embedded, ["Sunrise.png"]);
    assert_eq!(
        summary.not_embedded,
        [("Gone.png".to_owned(), Substitute::Missing)]
    );
    assert_eq!(
        summary.fonts,
        [
            FontRecord {
                family: "DejaVu Sans".into(),
                installed: Some(true)
            },
            FontRecord {
                family: "Synthetic Missing Serif".into(),
                installed: Some(false)
            },
        ]
    );
    assert_eq!(summary.omitted.len(), 1);
    assert_eq!(summary.omitted[0].translation, "RESTRICTED");
    assert_eq!(summary.bytes, fs::metadata(&package).unwrap().len());
    assert!(
        tree(&source.profile)
            .iter()
            .all(|p| !p.contains(".sela-export-")),
        "no temporary file is left"
    );

    assert!(snapshot(&target.profile).is_empty());
    let imported = import_schedule(&target.profile, &package, &no()).unwrap();
    assert_eq!(imported.added, ["Sunrise.png"]);
    assert!(imported.renamed.is_empty() && imported.reused.is_empty());
    assert_eq!(imported.not_included, ["Gone.png"]);
    assert_eq!(imported.fonts, summary.fonts);
    assert_eq!(imported.omitted, summary.omitted);
    assert_eq!(
        imported.schedule,
        target
            .profile
            .join("Schedules")
            .join("Sunday Service.selaschedule")
    );
    assert_eq!(
        fs::read(images::images_dir(&target.profile).join("Sunrise.png")).unwrap(),
        png([200, 120, 40]),
        "byte for byte, so the pin holds"
    );
    let opened = schedule_file::read(&imported.schedule).unwrap();
    // The restricted passage is gone and the order of the rest is kept.
    let mut expected = file.clone();
    expected.scripture.remove(1);
    expected.scripture[1].position = 4;
    assert_eq!(opened, expected);
    assert_eq!(image_refs(&opened), [sunrise, missing.clone()]);
    // Preflight on the new machine explains the missing image and font.
    let songs: Vec<Song> = opened.items.iter().map(|i| i.song.clone()).collect();
    let report = crate::preflight::check(
        &target.profile,
        &songs,
        &fonts_with(Some(&target.profile)),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.items.len(), 2);
    assert_eq!(report.items[0].fonts[0].family, "DejaVu Sans");
    assert_eq!(report.items[1].images[0].image, missing);
    assert_eq!(report.items[1].images[0].problem, Substitute::Missing);
    // Opening the same package again reuses the image and never replaces
    // the first schedule file.
    let again = import_schedule(&target.profile, &package, &no()).unwrap();
    assert_eq!(again.reused, ["Sunrise.png"]);
    assert_eq!(
        again.schedule,
        target
            .profile
            .join("Schedules")
            .join("Sunday Service (2).selaschedule")
    );
}

#[test]
fn an_image_name_taken_by_other_bytes_is_never_replaced() {
    let source = machine(&["a"]);
    let target = machine(&["b"]);
    let (file, sunrise, _) = sunday(&source);
    let package = source.profile.join("p.selapackage");
    export(&source, &file, &package);
    // Differs only in case: the same file on Windows and macOS volumes.
    let theirs = png([1, 1, 1]);
    put_image(&target.profile, "SUNRISE.png", &theirs);
    let imported = import_schedule(&target.profile, &package, &no()).unwrap();
    assert_eq!(
        imported.renamed,
        [("Sunrise.png".to_owned(), "Sunrise (2).png".to_owned())]
    );
    let dir = images::images_dir(&target.profile);
    assert_eq!(fs::read(dir.join("SUNRISE.png")).unwrap(), theirs);
    let opened = schedule_file::read(&imported.schedule).unwrap();
    let renamed = ImageRef {
        name: "Sunrise (2).png".into(),
        sha256: sunrise.sha256,
    };
    assert!(
        image_refs(&opened).contains(&renamed),
        "references follow the new name"
    );
    assert!(!image_refs(&opened).contains(&sunrise));
    assert_eq!(imported.not_included, ["Gone.png"]);
}

#[test]
fn exporting_without_images_lists_them_and_a_relink_repairs_after_import() {
    let source = machine(&["a"]);
    let target = machine(&["b"]);
    let (file, sunrise, _) = sunday(&source);
    let package = source.profile.join("p.selapackage");
    let summary = export_schedule(
        &source.profile,
        &file,
        &package,
        &Fonts::new(),
        ExportOptions {
            embed_images: false,
        },
        &no(),
    )
    .unwrap();
    assert!(summary.embedded.is_empty());
    assert_eq!(summary.listed_only, ["Sunrise.png", "Gone.png"]);
    assert!(
        summary.fonts.iter().all(|f| f.installed.is_none()),
        "fonts not scanned"
    );
    let imported = import_schedule(&target.profile, &package, &no()).unwrap();
    assert_eq!(imported.not_included, ["Sunrise.png", "Gone.png"]);
    assert!(
        !images::images_dir(&target.profile)
            .join("Sunrise.png")
            .exists()
    );
    // Locate… with the original file restores it under the pinned name.
    let original = source.profile.join("Resources/Images/Sunrise.png");
    assert_eq!(
        images::relink(&target.profile, &sunrise, &original, &no()),
        Ok(images::Relinked::Restored)
    );
    assert_eq!(
        images::content_hash(&target.profile, "Sunrise.png"),
        Ok(sunrise.sha256)
    );
}

/// Rebuilds `package` at `out` with `change` applied to each entry's bytes
/// (`None` drops the entry).
/// Maps an entry's name and bytes to its new bytes (`None` drops it).
type Change<'a> = &'a dyn Fn(&str, Vec<u8>) -> Option<Vec<u8>>;

fn rewrite(package: &Path, out: &Path, change: Change) {
    let mut archive = Archive::open(File::open(package).unwrap(), PROFILE_LIMITS).unwrap();
    let mut writer = Writer::new(File::create(out).unwrap());
    for i in 0..archive.entries().len() {
        let name = archive.entries()[i].name.clone();
        let (bytes, _) = archive.read(i, &|| false).unwrap();
        if let Some(bytes) = change(&name, bytes) {
            writer.add_bytes(&name, false, &bytes).unwrap();
        }
    }
    writer.finish().unwrap();
}

fn edit_manifest(bytes: Vec<u8>, edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    edit(&mut value);
    serde_json::to_vec(&value).unwrap()
}

fn exported() -> (Machine, PathBuf) {
    let source = machine(&["a"]);
    let (file, _, _) = sunday(&source);
    let package = source.profile.join("p.selapackage");
    export(&source, &file, &package);
    (source, package)
}

#[test]
fn tampered_or_inconsistent_packages_change_nothing() {
    let (source, package) = exported();
    let target = machine(&["b"]);
    put_image(&target.profile, "Mine.png", &png([5, 5, 5]));
    let before = snapshot(&target.profile);
    let bad = source.profile.join("bad.selapackage");
    let try_import = |change: Change| {
        rewrite(&package, &bad, change);
        let result = import_schedule(&target.profile, &bad, &no());
        assert_eq!(snapshot(&target.profile), before, "{result:?}");
        result
    };
    // Image bytes that do not match the manifest's SHA-256.
    let flipped = try_import(&|name, mut bytes| {
        if name.starts_with("images/") {
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
        }
        Some(bytes)
    });
    assert!(matches!(flipped, Err(Error::Damaged(_))), "{flipped:?}");
    // A file the manifest does not list.
    let extra = try_import(&|name, bytes| {
        Some(if name == manifest::NAME {
            edit_manifest(bytes, |v| {
                v["files"].as_array_mut().unwrap().pop();
                v["images"][0]["embedded"] = false.into();
            })
        } else {
            bytes
        })
    });
    assert!(matches!(extra, Err(Error::Damaged(_))), "{extra:?}");
    // A listed file that is missing.
    let missing = try_import(&|name, bytes| (!name.starts_with("images/")).then_some(bytes));
    assert!(matches!(missing, Err(Error::Damaged(_))), "{missing:?}");
    // A size the manifest disagrees with.
    let size = try_import(&|name, bytes| {
        Some(if name == manifest::NAME {
            edit_manifest(bytes, |v| v["files"][0]["size"] = 1.into())
        } else {
            bytes
        })
    });
    assert!(matches!(size, Err(Error::Damaged(_))), "{size:?}");
    // A manifest that is not JSON, or from a newer Sela.
    let junk = try_import(&|name, bytes| {
        Some(if name == manifest::NAME {
            b"{".to_vec()
        } else {
            bytes
        })
    });
    assert!(matches!(junk, Err(Error::Damaged(_))), "{junk:?}");
    let newer = try_import(&|name, bytes| {
        Some(if name == manifest::NAME {
            edit_manifest(bytes, |v| v["version"] = 7.into())
        } else {
            bytes
        })
    });
    assert_eq!(newer, Err(Error::NewerFormat(7)));
    // No manifest at all.
    let none = try_import(&|name, bytes| (name != manifest::NAME).then_some(bytes));
    assert_eq!(none, Err(Error::NotBundle));
    // A schedule changed after the manifest was written.
    let schedule = try_import(&|name, mut bytes| {
        if name == manifest::SCHEDULE_PATH {
            bytes.push(b' ');
        }
        Some(bytes)
    });
    assert!(matches!(schedule, Err(Error::Damaged(_))), "{schedule:?}");
    // An embedded "image" that is not an image, consistently hashed.
    let text = b"not an image".to_vec();
    let digest = hex_sha(&text);
    let fake = try_import(&|name, bytes| {
        Some(match name {
            manifest::NAME => edit_manifest(bytes, |v| {
                v["files"][1]["sha256"] = digest.clone().into();
                v["files"][1]["size"] = text.len().into();
                v["images"][0]["sha256"] = digest.clone().into();
            }),
            n if n.starts_with("images/") => text.clone(),
            _ => bytes,
        })
    });
    assert!(matches!(fake, Err(Error::Image(_, _))), "{fake:?}");
    // Not a bundle at all.
    fs::write(&bad, b"plain text, not a ZIP archive").unwrap();
    assert_eq!(
        import_schedule(&target.profile, &bad, &no()),
        Err(Error::NotBundle)
    );
    assert_eq!(snapshot(&target.profile), before);
}

#[test]
fn hostile_archives_never_write_outside_the_profile() {
    let target = machine(&["b"]);
    let outside = target.profile.parent().unwrap().join("evil.png");
    let image = png([3, 3, 3]);
    let digest = hex_sha(&image);
    let schedule = schedule_file::encode(&ScheduleFile {
        title: "x".into(),
        items: vec![item(1, song("A", None, None))],
        scripture: Vec::new(),
    })
    .unwrap();
    let manifest = |path: &str| {
        serde_json::json!({
            "format": "sela-bundle", "version": 1, "kind": "schedule",
            "generator": "x", "title": "x",
            "files": [
                {"path": "schedule.selaschedule", "size": schedule.len(),
                 "sha256": hex_sha(&schedule), "role": "schedule"},
                {"path": path, "size": image.len(), "sha256": digest, "role": "image"},
            ],
            "images": [{"name": "evil.png", "sha256": digest, "embedded": true}],
        })
        .to_string()
    };
    let bad = target.profile.parent().unwrap().join("bad.selapackage");
    for path in [
        "../evil.png",
        "images/../../evil.png",
        "/tmp/evil.png",
        "images\\..\\evil.png",
    ] {
        let bytes = build(
            &[
                raw("schedule.selaschedule", &schedule),
                raw(path, &image),
                raw("manifest.json", manifest(path).as_bytes()),
            ],
            &[],
        );
        fs::write(&bad, bytes).unwrap();
        let result = import_schedule(&target.profile, &bad, &no());
        assert!(
            matches!(result, Err(Error::Unsafe(_))),
            "{path}: {result:?}"
        );
        assert!(!outside.exists());
        assert!(tree(&target.profile).is_empty());
    }
    // A deflated schedule entry that inflates past its declared size.
    let mut bomb = deflated("schedule.selaschedule", &vec![b' '; 4 * 1024 * 1024]);
    bomb.size = schedule.len() as u32;
    let bytes = build(
        &[
            bomb,
            raw("images/evil.png", &image),
            raw("manifest.json", manifest("images/evil.png").as_bytes()),
        ],
        &[],
    );
    fs::write(&bad, bytes).unwrap();
    let result = import_schedule(&target.profile, &bad, &no());
    assert_eq!(result, Err(Error::TooLarge("schedule.selaschedule".into())));
    assert!(tree(&target.profile).is_empty());
}

#[test]
fn exports_replace_atomically_and_a_cancelled_export_leaves_the_old_file() {
    let source = machine(&["a"]);
    let (file, _, _) = sunday(&source);
    let package = source.profile.join("p.selapackage");
    fs::write(&package, b"previous").unwrap();
    let cancel = AtomicBool::new(true);
    assert_eq!(
        export_schedule(
            &source.profile,
            &file,
            &package,
            &Fonts::new(),
            ExportOptions::default(),
            &cancel
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(fs::read(&package).unwrap(), b"previous");
    assert!(!tree(&source.profile).iter().any(|p| p.contains(".partial")));
    export(&source, &file, &package);
    assert!(inspect(&package).is_ok());
    let empty = ScheduleFile {
        title: "Empty".into(),
        items: Vec::new(),
        scripture: vec![verses("RESTRICTED", false, 0)],
    };
    assert_eq!(
        export_schedule(
            &source.profile,
            &empty,
            &package,
            &Fonts::new(),
            ExportOptions::default(),
            &no()
        ),
        Err(Error::Empty)
    );
    assert!(inspect(&package).is_ok(), "the earlier export is kept");
}

// ---------------------------------------------------------------------------
// Profile backups.

fn profile_with_library(machine: &Machine) -> (Version, Song) {
    let mut repository = Repository::open(&machine.profile.join("library.sqlite")).unwrap();
    let song = song("Library Song", None, Some("Synthetic Serif"));
    let version = repository.save_song(None, song.clone()).unwrap();
    drop(repository);
    (version, song)
}

fn full_profile(machine: &Machine) -> (Version, Song) {
    let saved = profile_with_library(machine);
    let profile = &machine.profile;
    put_image(profile, "Logo.png", &png([10, 20, 30]));
    put_image(profile, "Back.jpg.png", &png([40, 50, 60]));
    fs::write(profile.join(images::LOGO_FILE), "Logo.png\n").unwrap();
    let schedules = profile.join("Schedules").join("Evening");
    fs::create_dir_all(&schedules).unwrap();
    let doc = schedule_file::encode(&ScheduleFile {
        title: "Vespers".into(),
        items: vec![item(1, song("A", None, None))],
        scripture: Vec::new(),
    })
    .unwrap();
    fs::write(schedules.join("Vespers.selaschedule"), &doc).unwrap();
    fs::write(
        profile.join("Schedules").join("notes.txt"),
        b"not a schedule",
    )
    .unwrap();
    let scripture = profile.join("Scripture");
    fs::create_dir_all(&scripture).unwrap();
    fs::write(scripture.join("KJV.selabible"), b"bundled, rebuilt by Sela").unwrap();
    fs::write(
        scripture.join("USER.selabible"),
        b"imported translation bytes",
    )
    .unwrap();
    let recovery = profile.join("Recovery");
    fs::create_dir_all(&recovery).unwrap();
    fs::write(recovery.join("autosave-1.json"), b"transient").unwrap();
    fs::write(profile.join("sela.lock"), b"").unwrap();
    let mut settings = settings::Settings {
        language: crate::i18n::Language::Indonesian,
        ..settings::Settings::default()
    };
    settings.remember_recent(PathBuf::from("/elsewhere/Other.selaschedule"));
    settings.remember_recent(schedules.join("Vespers.selaschedule"));
    fs::write(settings::path_in(profile), settings::render(&settings)).unwrap();
    saved
}

#[test]
fn a_profile_backup_restores_into_a_new_folder_on_another_machine() {
    let source = machine(&["home", "ana", ".local", "share", "sela"]);
    let (version, song) = full_profile(&source);
    let backup = source.profile.parent().unwrap().join("Ana.selabackup");
    let summary = backup_profile(&source.profile, &backup, &no()).unwrap();
    assert_eq!(summary.library_schema, storage::newest_schema());
    assert_eq!(
        summary.recent_dropped, 1,
        "the schedule outside the profile"
    );
    assert_eq!(
        summary.skipped,
        [("Schedules/notes.txt".to_owned(), Skip::Unusable)]
    );
    let listed: Vec<String> = inspect(&backup)
        .unwrap()
        .files
        .iter()
        .map(|f| f.path.clone())
        .collect();
    assert_eq!(
        listed,
        [
            "library.sqlite",
            "settings.txt",
            "logo.txt",
            "Resources/Images/Back.jpg.png",
            "Resources/Images/Logo.png",
            "Schedules/Evening/Vespers.selaschedule",
            "Scripture/USER.selabible",
        ],
        "no lock, autosave or bundled translation"
    );
    assert_eq!(summary.files, listed.len());

    let target = machine(&["E", "Restored"]);
    let destination = target.profile.join("Ana profile");
    let restored = restore_profile(&backup, &destination, &no()).unwrap();
    assert_eq!(restored.files, listed.len());
    assert!(!restored.settings_skipped);
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(tree(&destination), sorted);
    let repository = Repository::open(&library_in(&destination)).unwrap();
    assert_eq!(repository.song(version).unwrap(), song);
    drop(repository);
    for rel in [
        "Resources/Images/Logo.png",
        "logo.txt",
        "Scripture/USER.selabible",
        "Schedules/Evening/Vespers.selaschedule",
    ] {
        assert_eq!(
            fs::read(under(&destination, rel)).unwrap(),
            fs::read(under(&source.profile, rel)).unwrap(),
            "{rel}"
        );
    }
    let loaded =
        settings::parse(&fs::read_to_string(settings::path_in(&destination)).unwrap()).unwrap();
    assert_eq!(loaded.language, crate::i18n::Language::Indonesian);
    assert_eq!(
        loaded.recent_schedules,
        [destination
            .join("Schedules")
            .join("Evening")
            .join("Vespers.selaschedule")],
        "recent schedules follow the profile to its new place"
    );
    assert!(
        fs::read_dir(&target.profile).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sela-")),
        "no staging folder is left"
    );
}

#[cfg(unix)]
#[test]
fn backups_never_follow_links_out_of_the_profile() {
    let source = machine(&["p"]);
    profile_with_library(&source);
    let secret = source.profile.parent().unwrap().join("secret.png");
    fs::write(&secret, png([1, 2, 3])).unwrap();
    let images = images::images_dir(&source.profile);
    fs::create_dir_all(&images).unwrap();
    std::os::unix::fs::symlink(&secret, images.join("Link.png")).unwrap();
    let outside = source.profile.parent().unwrap().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("Private.selaschedule"), b"{}").unwrap();
    fs::create_dir_all(source.profile.join("Schedules")).unwrap();
    std::os::unix::fs::symlink(&outside, source.profile.join("Schedules").join("Linked")).unwrap();
    let backup = source.profile.parent().unwrap().join("b.selabackup");
    let summary = backup_profile(&source.profile, &backup, &no()).unwrap();
    assert_eq!(
        summary.skipped,
        [
            ("Schedules/Linked".to_owned(), Skip::Link),
            ("Resources/Images/Link.png".to_owned(), Skip::Link),
        ]
    );
    let paths: Vec<String> = inspect(&backup)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(paths, ["library.sqlite"]);
}

#[test]
fn restore_never_writes_into_an_existing_place() {
    let source = machine(&["p"]);
    full_profile(&source);
    let backup = source.profile.parent().unwrap().join("b.selabackup");
    backup_profile(&source.profile, &backup, &no()).unwrap();
    let target = machine(&["t"]);
    let existing = target.profile.join("Existing");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("library.sqlite"), b"their data").unwrap();
    assert_eq!(
        restore_profile(&backup, &existing, &no()),
        Err(Error::DestinationExists)
    );
    let empty = target.profile.join("Empty");
    fs::create_dir_all(&empty).unwrap();
    assert_eq!(
        restore_profile(&backup, &empty, &no()),
        Err(Error::DestinationExists)
    );
    let file = target.profile.join("file");
    fs::write(&file, b"keep").unwrap();
    assert_eq!(
        restore_profile(&backup, &file, &no()),
        Err(Error::DestinationExists)
    );
    assert_eq!(
        fs::read(existing.join("library.sqlite")).unwrap(),
        b"their data"
    );
    assert_eq!(fs::read(&file).unwrap(), b"keep");
    assert_eq!(
        tree(&target.profile),
        ["Existing/library.sqlite", "file"],
        "nothing else appeared"
    );
    // Wrong kind both ways.
    let (_package_source, package) = exported();
    assert_eq!(
        restore_profile(&package, &target.profile.join("New"), &no()),
        Err(Error::WrongKind(Kind::Schedule))
    );
    assert_eq!(
        import_schedule(&target.profile, &backup, &no()),
        Err(Error::WrongKind(Kind::Profile))
    );
    assert!(!target.profile.join("New").exists());
}

#[test]
fn a_damaged_or_newer_backup_restores_nothing() {
    let source = machine(&["p"]);
    full_profile(&source);
    let backup = source.profile.parent().unwrap().join("b.selabackup");
    backup_profile(&source.profile, &backup, &no()).unwrap();
    let target = machine(&["t"]);
    let bad = source.profile.parent().unwrap().join("bad.selabackup");
    let attempt = |change: Change| {
        rewrite(&backup, &bad, change);
        let destination = target.profile.join("R");
        let result = restore_profile(&bad, &destination, &no());
        assert!(!destination.exists(), "{result:?}");
        assert!(
            tree(&target.profile).is_empty(),
            "no staging left: {result:?}"
        );
        result
    };
    // A library that is not a Sela library, with a matching SHA-256.
    let junk = b"SQLite format 3\0 but not really a database".to_vec();
    let junk_sha = hex_sha(&junk);
    let library = attempt(&|name, bytes| match name {
        manifest::LIBRARY_PATH => Some(junk.clone()),
        manifest::NAME => Some(edit_manifest(bytes, |v| {
            v["files"][0]["sha256"] = junk_sha.clone().into();
            v["files"][0]["size"] = junk.len().into();
        })),
        _ => Some(bytes),
    });
    assert!(matches!(library, Err(Error::Library(_))), "{library:?}");
    // A schedule file whose bytes changed after the manifest.
    let changed = attempt(&|name, mut bytes| {
        if name.starts_with("Schedules/") {
            bytes.push(b' ');
        }
        Some(bytes)
    });
    assert!(matches!(changed, Err(Error::Damaged(_))), "{changed:?}");
    // A library schema this Sela does not know yet.
    let newest = storage::newest_schema();
    let newer = attempt(&|name, bytes| {
        Some(if name == manifest::NAME {
            edit_manifest(bytes, |v| v["library_schema"] = (newest + 1).into())
        } else {
            bytes
        })
    });
    assert_eq!(newer, Err(Error::NewerLibrary(newest + 1)));
    // A path outside the profile layout.
    let stray = attempt(&|name, bytes| {
        Some(if name == manifest::NAME {
            edit_manifest(bytes, |v| v["files"][2]["path"] = "../logo.txt".into())
        } else {
            bytes
        })
    });
    assert!(matches!(stray, Err(Error::Unsafe(_))), "{stray:?}");
    // Cancelled.
    let cancelled = restore_profile(&backup, &target.profile.join("C"), &AtomicBool::new(true));
    assert_eq!(cancelled, Err(Error::Cancelled));
    assert!(tree(&target.profile).is_empty());
}

/// Spawns this test binary to abort the restore part-way: no destructor
/// runs, as after a crash or power cut during a restore.
#[test]
fn an_interrupted_restore_never_damages_existing_data() {
    const CHILD: &str = "SELA_ABORT_RESTORE_DIR";
    if let Some(dir) = std::env::var_os(CHILD) {
        let dir = Path::new(&dir);
        restore_profile(
            &dir.join("b.selabackup"),
            &dir.join("target").join("R"),
            &no(),
        )
        .unwrap();
        panic!("the abort hook did not run");
    }
    let source = machine(&["p"]);
    full_profile(&source);
    let root = source.profile.parent().unwrap();
    let backup = root.join("b.selabackup");
    backup_profile(&source.profile, &backup, &no()).unwrap();
    let target = root.join("target");
    fs::create_dir_all(target.join("Other profile")).unwrap();
    fs::write(
        target.join("Other profile").join("library.sqlite"),
        b"keep me",
    )
    .unwrap();
    let backup_before = fs::read(&backup).unwrap();
    let source_before = snapshot(&source.profile);
    for after in ["1", "4"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bundle::tests::an_interrupted_restore_never_damages_existing_data",
            ])
            .env(CHILD, root)
            .env("SELA_ABORT_RESTORE_AFTER", after)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(6), "aborted, not failed");
        }
        assert!(!target.join("R").exists(), "a half restore never appears");
        assert_eq!(
            fs::read(target.join("Other profile").join("library.sqlite")).unwrap(),
            b"keep me"
        );
    }
    assert_eq!(fs::read(&backup).unwrap(), backup_before);
    assert_eq!(snapshot(&source.profile), source_before);
    let leftovers: Vec<String> = fs::read_dir(&target)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "Other profile")
        .collect();
    assert_eq!(leftovers.len(), 2, "{leftovers:?}");
    assert!(leftovers.iter().all(|n| n.starts_with(".sela-restore-")));
    // The next restore works beside the leftovers.
    restore_profile(&backup, &target.join("R"), &no()).unwrap();
    assert!(library_in(&target.join("R")).is_file());
}

#[test]
fn a_backup_needs_a_library_and_a_cancelled_backup_publishes_nothing() {
    let empty = machine(&["p"]);
    let destination = empty.profile.join("b.selabackup");
    assert_eq!(
        backup_profile(&empty.profile, &destination, &no()),
        Err(Error::Library(storage::Error::Missing))
    );
    let source = machine(&["q"]);
    full_profile(&source);
    let destination = source.profile.parent().unwrap().join("b.selabackup");
    fs::write(&destination, b"older backup").unwrap();
    assert_eq!(
        backup_profile(&source.profile, &destination, &AtomicBool::new(true)),
        Err(Error::Cancelled)
    );
    assert_eq!(fs::read(&destination).unwrap(), b"older backup");
    let parent = source.profile.parent().unwrap();
    assert!(
        fs::read_dir(parent).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sela-")),
        "no temporary file or staging folder is left"
    );
}

#[test]
fn the_worker_runs_one_job_at_a_time_and_replies() {
    let source = machine(&["p"]);
    let (file, _, _) = sunday(&source);
    static FONTS: std::sync::OnceLock<Fonts> = std::sync::OnceLock::new();
    let mut worker =
        Worker::start_with(source.profile.clone(), FONTS.get_or_init(Fonts::new)).unwrap();
    let package = source.profile.join("w.selapackage");
    assert!(worker.submit(Job::ExportSchedule {
        file: Box::new(file),
        destination: package.clone(),
        options: ExportOptions::default(),
    }));
    let busy = worker.submit(Job::ImportSchedule {
        package: package.clone(),
    });
    let wait = |worker: &mut Worker| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if let Some(done) = worker.poll() {
                return done;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    match wait(&mut worker) {
        Done::Exported(path, Ok(summary)) => {
            assert_eq!(path, package);
            assert_eq!(summary.embedded, ["Sunrise.png"]);
        }
        other => panic!("{other:?}"),
    }
    if busy {
        // Accepted only when the export had already finished.
        assert!(matches!(wait(&mut worker), Done::Imported(_, Ok(_))));
    }
    assert!(!worker.busy());
}

#[test]
fn file_stems_are_portable() {
    assert_eq!(file_stem("Sunday: 9am / Main?"), "Sunday_ 9am _ Main_");
    assert_eq!(file_stem("  ...  "), "Schedule");
    assert_eq!(file_stem("CON"), "Schedule");
    assert_eq!(file_stem(""), "Schedule");
    assert_eq!(
        with_extension(Path::new("a/b"), "selapackage"),
        Path::new("a/b.selapackage")
    );
    assert_eq!(
        with_extension(Path::new("a/b.SELAPACKAGE"), "selapackage"),
        Path::new("a/b.SELAPACKAGE")
    );
    assert!(is_bundle(Path::new("x.SelaBackup")) && !is_bundle(Path::new("x.selaschedule")));
}
