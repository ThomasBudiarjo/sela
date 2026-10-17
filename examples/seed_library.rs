//! Creates a new song library with original fixture songs for native checks
//! (`scripts/live-output-windows.py`, `scripts/schedule-windows.py`). Refuses
//! to touch an existing file. `--second-song` adds a one-slide song;
//! `--extra-songs N` adds N one-slide songs "Study Hymn k"\n//! (`scripts/schedule-remainder-windows.py`); `--formatted-song` seeds a single-song library whose three slides
//! exercise the M1-05g2 rendering path (bold bundled face, synth italic,
//! underline, color, right/bottom alignment, Outer outline, shadow, an
//! installed family); `--background-song` seeds a single-song library whose
//! four slides exercise M1-05h backgrounds: a 4:1 profile image (red, green,
//! blue bands) with Zoom, the same image with Maintain, a missing image, and
//! the song master's color; `--metadata-songs` seeds "Metadata Hymn" (author,
//! copyright, administrator, song number) and "Plain Hymn" (none); --heavy-song seeds one 100-slide song of\n//! 28-line slides (scripts/prep-windows.py).
use image::ImageEncoder;
use sela::{
    arrangement::{Occurrence, OccurrenceId, SectionId, Variant, VariantId},
    background::{Aspect, Background, ImageRef},
    format::{Align, Outline, Shadow, Size, SlideFormat, VAlign},
    storage::{Repository, Section, Song},
};
use sha2::Digest;

const USAGE: &str = "usage: seed_library <new-database-path> [--second-song] [--extra-songs N] [--formatted-song] [--background-song] [--heavy-song] [--vanished-schedule] [--arranged-song] [--metadata-songs]";

/// `scripts/live-output-windows.py` samples these.
const BANDS: [[u8; 3]; 3] = [[200, 40, 40], [40, 160, 60], [40, 60, 200]];
const MASTER: [u8; 3] = [20, 30, 90];

/// Writes the 400x100 band image into the profile beside the database.
fn backdrop(path: &std::path::Path) -> Result<ImageRef, Box<dyn std::error::Error>> {
    let images = sela::images::images_dir(path.parent().ok_or("database has no folder")?);
    std::fs::create_dir_all(&images)?;
    let (width, height) = (400u32, 100u32);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..height {
        for x in 0..width {
            let band = match x {
                0..100 => 0,
                100..300 => 1,
                _ => 2,
            };
            pixels.extend(BANDS[band]);
            pixels.push(255);
        }
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &pixels,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;
    let name = "sela-backdrop.png";
    let file = images.join(name);
    if file.exists() {
        return Err(format!("{} already exists", file.display()).into());
    }
    std::fs::write(file, &png)?;
    Ok(ImageRef {
        name: name.into(),
        sha256: sha2::Sha256::digest(&png).into(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut path = None;
    let (mut second, mut formatted, mut background, mut heavy, mut vanished, mut arranged) =
        (false, false, false, false, false, false);
    let mut metadata = false;
    let mut extra = 0usize;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg {
            flag if flag == "--second-song" => second = true,
            flag if flag == "--formatted-song" => formatted = true,
            flag if flag == "--background-song" => background = true,
            flag if flag == "--heavy-song" => heavy = true,
            flag if flag == "--extra-songs" => {
                extra = args
                    .next()
                    .and_then(|n| n.to_string_lossy().parse().ok())
                    .ok_or(USAGE)?;
            }
            flag if flag == "--vanished-schedule" => vanished = true,
            flag if flag == "--arranged-song" => arranged = true,
            flag if flag == "--metadata-songs" => metadata = true,
            arg if path.is_none() && !arg.to_string_lossy().starts_with("--") => path = Some(arg),
            _ => return Err(USAGE.into()),
        }
    }
    let Some(path) = path else {
        return Err(USAGE.into());
    };
    let path = std::path::Path::new(&path);
    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }
    let mut repository = Repository::open(path).map_err(|e| format!("{e:?}"))?;
    let section = |label: &str, lyrics: &str| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
        format: Default::default(),
        background: None,
    };
    let formatted_section = |label: &str, lyrics: &str, format: SlideFormat| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
        format,
        background: None,
    };
    if metadata {
        // "Metadata Hymn" carries every song property; "Plain Hymn" has no
        // copyright text, so it must show no strip (VAL-SONG-002).
        let song = |title: &str, authors: &str, copyright: &str, admin: &str, number: &str| Song {
            title: title.into(),
            authors: authors.into(),
            copyright: copyright.into(),
            license: String::new(),
            administrator: admin.into(),
            song_number: number.into(),
            variants: Vec::new(),
            sections: vec![
                section("Verse 1", &format!("{title} first original line")),
                section("Chorus", &format!("{title} original refrain")),
                section("Bridge", &format!("{title} last original line")),
            ],
            master: None,
        };
        for (title, authors, copyright, admin, number) in [
            (
                "Metadata Hymn",
                "Quill Author",
                "Synthetic Copyright 2026",
                "Acme Administration",
                "7654321",
            ),
            ("Plain Hymn", "", "", "", ""),
        ] {
            repository
                .save_song(None, song(title, authors, copyright, admin, number))
                .map_err(|e| format!("{e:?}"))?;
        }
        return Ok(());
    }
    if heavy {
        let line = "Original heavy line of lyrics for the fitting measurement";
        let sections = (0..100)
            .map(|i| {
                section(
                    &format!("Verse {i}"),
                    &vec![format!("{line} {i}"); 28].join("\n"),
                )
            })
            .collect();
        repository
            .save_song(
                None,
                Song {
                    title: "Heavy Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections,
                    master: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(());
    }
    if arranged {
        let sections = vec![
            section("Verse 1", "Arranged verse line"),
            section("Chorus", "Arranged chorus line"),
            section("Bridge", "Arranged bridge line"),
            section(
                "Long Verse",
                &(1..=40)
                    .map(|n| format!("Overflowing line {n}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ];
        let variant = |name: &str, order: &[usize]| Variant {
            id: VariantId(SectionId::allocate().0),
            name: name.into(),
            occurrences: order
                .iter()
                .map(|n| Occurrence {
                    id: OccurrenceId(SectionId::allocate().0),
                    section: sections[*n].id,
                })
                .collect(),
        };
        let variants = vec![
            variant("Full", &[0, 1, 2]),
            variant("Chorus Twice", &[1, 1]),
            variant("Long", &[3]),
        ];
        repository
            .save_song(
                None,
                Song {
                    title: "Arranged Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants,
                    sections,
                    master: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(());
    }
    if vanished {
        let image = backdrop(path)?;
        let folder = path.parent().ok_or("database has no folder")?;
        let managed = sela::images::images_dir(folder).join(&image.name);
        std::fs::rename(&managed, folder.join("original-backdrop.png"))?;
        let version = repository
            .save_song(
                None,
                Song {
                    title: "Vanished Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![Section {
                        background: Some(Background::image(image)),
                        ..section("Verse 1", "Verse 1 original line")
                    }],
                    master: Some(Background::color(MASTER)),
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let song = repository.song(version).map_err(|e| format!("{e:?}"))?;
        let document = sela::schedule_file::ScheduleFile {
            title: "Sunday".into(),
            items: vec![sela::schedule_file::Item {
                source: version,
                song,
                arrangement: None,
            }],
        };
        let bytes = sela::schedule_file::encode(&document).map_err(|e| format!("{e:?}"))?;
        std::fs::write(folder.join("Sunday.selaschedule"), bytes)?;
        return Ok(());
    }
    if background {
        let image = backdrop(path)?;
        let with = |label: &str, background: Option<Background>| Section {
            background,
            ..section(label, &format!("{label} original line"))
        };
        repository
            .save_song(
                None,
                Song {
                    title: "Backdrop Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![
                        with("Verse 1", Some(Background::image(image.clone()))),
                        with(
                            "Chorus",
                            Some(Background {
                                aspect: Aspect::Maintain,
                                ..Background::image(image.clone())
                            }),
                        ),
                        with(
                            "Bridge",
                            Some(Background::image(ImageRef {
                                name: "sela-missing.png".into(),
                                ..image
                            })),
                        ),
                        with("Tag", None),
                    ],
                    master: Some(Background::color(MASTER)),
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(());
    }
    if formatted {
        repository
            .save_song(
                None,
                Song {
                    title: "Formatted Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![
                        formatted_section(
                            "Verse 1",
                            "Golden bold refrain",
                            SlideFormat {
                                bold: Some(true),
                                // Fixed so Right/Bottom placement is visible:
                                // auto-fit fills the area width.
                                size: Some(Size::Fixed(120)),
                                color: Some([255, 210, 0]),
                                align: Some(Align::Right),
                                valign: Some(VAlign::Bottom),
                                outline: Some(Outline {
                                    enabled: true,
                                    color: [0, 0, 0],
                                    size: 7,
                                    opacity: 100,
                                }),
                                shadow: Some(Shadow {
                                    enabled: true,
                                    color: [0, 0, 0],
                                    angle: 135,
                                    offset: 20,
                                    blur: 8,
                                    opacity: 70,
                                }),
                                ..Default::default()
                            },
                        ),
                        formatted_section(
                            "Chorus",
                            "Italic underlined chorus line",
                            SlideFormat {
                                italic: Some(true),
                                underline: Some(true),
                                ..Default::default()
                            },
                        ),
                        formatted_section(
                            "Bridge",
                            "Arial installed line",
                            SlideFormat {
                                font: Some("Arial".into()),
                                bold: Some(true),
                                italic: Some(true),
                                ..Default::default()
                            },
                        ),
                    ],
                    master: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(());
    }
    repository
        .save_song(
            None,
            Song {
                title: "Signal Hymn".into(),
                authors: String::new(),
                copyright: String::new(),
                license: String::new(),
                administrator: String::new(),
                song_number: String::new(),
                variants: Vec::new(),
                sections: vec![
                    section("Verse 1", "First original line\nSecond original line"),
                    section("Chorus", "Original refrain"),
                    section("Bridge", "Original bridge\nthat ends the song"),
                ],
                master: None,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
    if second {
        repository
            .save_song(
                None,
                Song {
                    title: "Quiet Canticle".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![section("Verse 1", "A quiet original canticle")],
                    master: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    for k in 1..=extra {
        repository
            .save_song(
                None,
                Song {
                    title: format!("Study Hymn {k}"),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
                    administrator: String::new(),
                    song_number: String::new(),
                    variants: Vec::new(),
                    sections: vec![section("Verse 1", &format!("Study original line {k}"))],
                    master: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
