//! Creates a new song library with original fixture songs for native checks
//! (`scripts/live-output-windows.py`, `scripts/schedule-windows.py`). Refuses
//! to touch an existing file. `--second-song` adds a one-slide song.
use sela::{
    arrangement::SectionId,
    storage::{Repository, Section, Song},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (path, second) = match args.as_slice() {
        [path] => (path, false),
        [path, flag] if flag == "--second-song" => (path, true),
        _ => return Err("usage: seed_library <new-database-path> [--second-song]".into()),
    };
    let path = std::path::Path::new(path);
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
