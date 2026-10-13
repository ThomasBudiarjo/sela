//! Saves a new head revision of a library song for native checks
//! (`scripts/schedule-remainder-windows.py`): replaces the first slide's lyrics.
//! usage: edit_song <library-path> <song-title> <new-first-slide-lyrics>
use sela::storage::Repository;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [path, title, lyrics] = args.as_slice() else {
        return Err("usage: edit_song <library-path> <song-title> <new-first-slide-lyrics>".into());
    };
    let mut repository =
        Repository::open(std::path::Path::new(path)).map_err(|e| format!("{e:?}"))?;
    let version = repository
        .catalog(None)
        .map_err(|e| format!("{e:?}"))?
        .into_iter()
        .find(|(_, name)| name == title)
        .map(|(version, _)| version)
        .ok_or("no such song")?;
    let mut song = repository.song(version).map_err(|e| format!("{e:?}"))?;
    song.sections[0].lyrics = lyrics.clone();
    repository
        .save_song(Some(version), song)
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}
