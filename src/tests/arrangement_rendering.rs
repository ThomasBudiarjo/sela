//! sunday-core fix: arrangement rendering (SC-ARR-01, SC-ARR-02, SC-SONG-02).
use super::*;
use sela::{
    arrangement::{Occurrence, OccurrenceId, Variant, VariantId},
    format::SlideFormat,
    scene::ContentVersion,
};

fn section(label: &str, lyrics: &str, format: SlideFormat) -> Section {
    Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
        format,
        background: None,
    }
}

fn save(path: &std::path::Path, title: &str, sections: Vec<Section>, variants: Vec<Variant>) {
    Repository::open(path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: title.into(),
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
        .unwrap();
}

/// "Arranged": a bold Verse, a Chorus and a Bridge, arranged as Full
/// (V C B) and Short (C C).
fn library_with_arrangements(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let bold = SlideFormat {
        bold: Some(true),
        ..Default::default()
    };
    let sections = vec![
        section("Verse", "verse words", bold),
        section("Chorus", "chorus words", Default::default()),
        section("Bridge", "bridge words", Default::default()),
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
    let variants = vec![variant("Full", &[0, 1, 2]), variant("Short", &[1, 1])];
    save(&path, "Arranged", sections, variants);
    path
}

/// Per Live slide: the face and synthetic styles its cues are built with.
fn live_faces(o: &Operator) -> Option<Vec<(ContentVersion, bool, bool)>> {
    let resolved = o.live.as_ref()?.resolved.as_ref()?;
    Some(
        resolved
            .iter()
            .map(|r| (r.face().version(), r.synth_bold(), r.synth_italic()))
            .collect(),
    )
}

#[gpui::test]
fn previewing_another_arrangement_keeps_live_faces_and_every_live_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_arrangements(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 1);
    add_song(&mut cx, &operator, "Arranged");
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "Full preview resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry.is_some() && p.slides.len() == 3 && p.resolved.is_some())
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Arranged · Verse")
    });
    let full = operator.read_with(&cx, |o, _| live_faces(o)).unwrap();
    assert_eq!(full.len(), 3);
    assert_ne!(full[0], full[1], "the bold Verse has its own face");

    click(&mut cx, "preview-arrangement");
    settle(&mut cx, &operator, "Short preview resolved", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.slides.len() == 2 && p.resolved.is_some())
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            o.live.as_ref().unwrap().slides.len(),
            3,
            "Live keeps the Full arrangement"
        );
        assert_eq!(live_faces(o), Some(full.clone()), "Live keeps its faces");
        let short = o.preview.as_ref().unwrap().resolved.as_ref().unwrap();
        assert_eq!(short.len(), 2);
        assert!(
            short
                .iter()
                .all(|r| (r.face().version(), r.synth_bold(), r.synth_italic()) == full[1]),
            "Short gets the Chorus face twice"
        );
    });

    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "chorus on screen", |o| {
        o.on_screen() == Some("Arranged · Chorus")
    });
    click(&mut cx, "live-next");
    settle(&mut cx, &operator, "bridge on screen", |o| {
        o.on_screen() == Some("Arranged · Bridge")
    });
    operator.read_with(&cx, |o, _| {
        assert!(!o.cue_failed, "every Live slide prepares");
        assert_eq!(o.live_slide, Some(2));
    });

    let version = operator.read_with(&cx, |o, _| o.live.as_ref().unwrap().version);
    cx.update(|_, cx| {
        operator.update(cx, |o, cx| {
            let one = vec![fonts::Resolved::bundled(&SlideFormat::default())];
            o.fonts_resolved(version, one, cx);
        })
    });
    assert_eq!(
        operator.read_with(&cx, |o, _| live_faces(o)),
        Some(full),
        "a resolution that is not one face per Live slide is not attached"
    );
}

/// Sixteen and twenty numbered lines, typed with blank lines around them,
/// trailing spaces and a final Enter.
fn library_with_padded_verses(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("library.sqlite");
    let padded = |count: usize| {
        let lines: Vec<String> = (1..=count).map(|n| format!("Line {n}  ")).collect();
        format!("\n \n{}\n\n", lines.join("\n"))
    };
    for (title, count) in [("Sixteen", 16), ("Twenty", 20)] {
        save(
            &path,
            title,
            vec![section("Verse", &padded(count), Default::default())],
            Vec::new(),
        );
    }
    path
}

fn numbered(lines: std::ops::RangeInclusive<usize>) -> String {
    lines
        .map(|n| format!("Line {n}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[gpui::test]
fn the_editor_pages_a_section_exactly_like_the_audience(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_padded_verses(&dir)), "apply");
    cx.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    // At most 16 lines per page, balanced (docs/arrangement.md).
    let expected = [
        ("Sixteen", vec![numbered(1..=16)]),
        ("Twenty", vec![numbered(1..=10), numbered(11..=20)]),
    ];
    for (title, pages) in expected {
        let row = song_row(&mut cx, &operator, title);
        let before = editor_windows(&cx);
        right_click(&mut cx, row);
        click(&mut cx, "song-menu-edit");
        let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(version, _, busy)| {
            version.is_some() && !busy
        });
        editor_cx.simulate_resize(size(px(1400.), px(900.)));
        editor_cx.run_until_parked();
        settle(&mut cx, &operator, "audience projection", |o| {
            o.preview.as_ref().is_some_and(|p| p.title == title)
        });
        let audience: Vec<String> = operator.read_with(&cx, |o, _| {
            let preview = o.preview.as_ref().unwrap();
            preview.slides.iter().map(|s| s.text.clone()).collect()
        });
        assert_eq!(audience, pages, "{title}: audience slides");
        let (editor_pages, shown) = editor.read_with(&editor_cx, |e, cx| {
            let pages: Vec<String> = e.audience_pages(cx).into_iter().map(|s| s.text).collect();
            (pages, e.preview_slide(cx).map(|s| s.text))
        });
        assert_eq!(editor_pages, audience, "{title}: editor pages");
        assert_eq!(shown.as_ref(), audience.first(), "{title}: editor preview");
        assert_eq!(
            editor_cx.debug_bounds("split-badge").is_some(),
            audience.len() > 1,
            "{title}: split badge"
        );
    }
}
