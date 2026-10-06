//! Creates a new song library with original fixture songs for native checks
//! (`scripts/live-output-windows.py`, `scripts/schedule-windows.py`). Refuses
//! to touch an existing file. `--second-song` adds a one-slide song;
//! `--formatted-song` seeds a single-song library whose three slides
//! exercise the M1-05g2 rendering path (bold bundled face, synth italic,
//! underline, color, right/bottom alignment, Outer outline, shadow, an
//! installed family).
use sela::{
    arrangement::SectionId,
    format::{Align, Outline, Shadow, Size, SlideFormat, VAlign},
    storage::{Repository, Section, Song},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut path = None;
    let (mut second, mut formatted) = (false, false);
    for arg in args {
        match arg {
            flag if flag == "--second-song" => second = true,
            flag if flag == "--formatted-song" => formatted = true,
            arg if path.is_none() && !arg.to_string_lossy().starts_with("--") => path = Some(arg),
            _ => {
                return Err(
                    "usage: seed_library <new-database-path> [--second-song] [--formatted-song]"
                        .into(),
                );
            }
        }
    }
    let Some(path) = path else {
        return Err(
            "usage: seed_library <new-database-path> [--second-song] [--formatted-song]".into(),
        );
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
    };
    let formatted_section = |label: &str, lyrics: &str, format: SlideFormat| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
        format,
    };
    if formatted {
        repository
            .save_song(
                None,
                Song {
                    title: "Formatted Hymn".into(),
                    authors: String::new(),
                    copyright: String::new(),
                    license: String::new(),
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
                variants: Vec::new(),
                sections: vec![
                    section("Verse 1", "First original line\nSecond original line"),
                    section("Chorus", "Original refrain"),
                    section("Bridge", "Original bridge\nthat ends the song"),
                ],
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
                    variants: Vec::new(),
                    sections: vec![section("Verse 1", "A quiet original canticle")],
                },
            )
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
