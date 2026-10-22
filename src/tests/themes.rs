//! M1-08 themes: the Themes tab, applying themes to songs and schedule
//! items, the theme editor, snapshots in schedule files, and Live staying
//! unchanged until a theme reaches it through Go Live.
use super::*;
use sela::{
    background::Background,
    format::{Align, Margins, SlideFormat},
    theme::{Theme, ThemeId},
};

fn navy() -> Theme {
    Theme {
        format: SlideFormat {
            bold: Some(true),
            color: Some([255, 214, 90]),
            align: Some(Align::Left),
            margins: Some(Margins {
                left: 160,
                right: 160,
                top: 90,
                bottom: 90,
            }),
            ..Default::default()
        },
        background: Some(Background::color([20, 30, 90])),
        show_copyright: false,
        id: ThemeId([0x11; 16]),
        ..Theme::new("Evening Navy")
    }
}

fn pale() -> Theme {
    Theme {
        format: SlideFormat {
            color: Some([255, 255, 255]),
            ..Default::default()
        },
        background: Some(Background::color([240, 240, 236])),
        id: ThemeId([0x22; 16]),
        ..Theme::new("Low Contrast")
    }
}

fn missing_face() -> Theme {
    Theme {
        format: SlideFormat {
            font: Some("Sela Missing Face".into()),
            ..Default::default()
        },
        id: ThemeId([0x33; 16]),
        ..Theme::new("Missing Face")
    }
}

/// The two-song library plus three themes.
fn library_with_themes(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = library_with_songs(dir);
    let mut repository = Repository::open(&path).unwrap();
    for theme in [navy(), pale(), missing_face()] {
        repository.save_theme(&theme).unwrap();
    }
    path
}

/// Row selector of a theme by name (row 0 is the built-in theme).
fn theme_row(o: &Operator, name: &str) -> &'static str {
    let index = o
        .themes
        .rows()
        .iter()
        .position(|t| t.name == name)
        .unwrap_or_else(|| panic!("{name} is listed"));
    [
        "theme-0", "theme-1", "theme-2", "theme-3", "theme-4", "theme-5",
    ][index]
}

fn open_themes(cx: &mut VisualTestContext, operator: &Entity<Operator>, count: usize) {
    click(cx, "Themes");
    settle(cx, operator, "themes listed", |o| {
        o.tab == 4 && o.themes.loaded && o.themes.set.themes.len() == count
    });
}

fn select_theme(cx: &mut VisualTestContext, operator: &Entity<Operator>, name: &str) {
    let row = operator.read_with(cx, |o, _| theme_row(o, name));
    click(cx, row);
    assert_eq!(
        operator.read_with(cx, |o, _| o.themes.selected_theme().map(|t| t.name)),
        Some(name.to_owned())
    );
}

fn preview_key(o: &Operator) -> Option<u64> {
    o.preview.as_ref().map(|p| p.theme_key)
}

fn live_key(o: &Operator) -> Option<u64> {
    o.live.as_ref().map(|p| p.theme_key)
}

#[gpui::test]
fn themes_tab_lists_themes_with_thumbnails_badges_and_warnings(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut cx, operator, _) = with_library(cx, Some(library_with_themes(&dir)), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    open_themes(&mut cx, &operator, 3);
    // Thumbnails render one at a time off the UI thread.
    settle(&mut cx, &operator, "every thumbnail", |o| {
        o.themes.thumbs.len() == 4 && o.themes.thumbs.values().all(|t| t.image.is_some())
    });
    operator.read_with(&cx, |o, _| {
        let names: Vec<_> = o.themes.rows().iter().map(|t| t.name.clone()).collect();
        assert_eq!(
            names,
            ["Default", "Evening Navy", "Low Contrast", "Missing Face"],
            "built-in first, then by name"
        );
        let warnings = |name: &str| {
            let theme = o
                .themes
                .rows()
                .into_iter()
                .find(|t| t.name == name)
                .unwrap();
            o.themes
                .thumbs
                .iter()
                .find(|((stamp, _, _), _)| *stamp == theme.stamp())
                .map(|(_, thumb)| thumb.warnings.clone())
                .unwrap()
        };
        assert!(warnings("Default").is_empty());
        assert!(
            warnings("Evening Navy").is_empty(),
            "gold on navy is legible"
        );
        let pale = warnings("Low Contrast");
        assert!(
            pale.iter().any(|w| w.starts_with("Low contrast 1.")),
            "{pale:?}"
        );
        let face = warnings("Missing Face");
        assert_eq!(
            face,
            ["Font “Sela Missing Face” is not installed · showing DejaVu Sans"]
        );
    });
    // The built-in theme is the default until another is chosen.
    assert_eq!(operator.read_with(&cx, |o, _| o.themes.set.default), None);
    select_theme(&mut cx, &operator, "Evening Navy");
    click(&mut cx, "theme-set-default");
    settle(&mut cx, &operator, "default saved", |o| {
        o.themes.set.default == Some(navy().id)
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(o.themes.set.default_theme().name, "Evening Navy");
        assert_eq!(
            o.themes.message.as_ref().map(|m| m.0.as_str()),
            Some("“Evening Navy” is now the default song theme.")
        );
    });
    // The built-in theme cannot be edited or deleted.
    select_theme(&mut cx, &operator, "Default");
    click(&mut cx, "theme-delete");
    operator.read_with(&cx, |o, _| {
        assert!(o.themes.message.as_ref().unwrap().1, "refused");
        assert_eq!(o.themes.set.themes.len(), 3);
    });
}

#[gpui::test]
fn applying_a_theme_to_a_song_restyles_preview_but_live_waits_for_go_live(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_themes(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let row = song_row(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, row);
    settle(&mut cx, &operator, "built-in preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.resolved.is_some())
    });
    assert_eq!(operator.read_with(&cx, |o, _| preview_key(o)), Some(0));
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "verse on screen", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    open_themes(&mut cx, &operator, 3);
    select_theme(&mut cx, &operator, "Evening Navy");
    click(&mut cx, "theme-apply-song");
    settle(&mut cx, &operator, "preview restyled", |o| {
        preview_key(o) == Some(navy().key()) && o.preview.as_ref().unwrap().resolved.is_some()
    });
    operator.read_with(&cx, |o, _| {
        let preview = o.preview.as_ref().unwrap();
        // Every slide takes the theme: format, background, no copyright.
        for slide in preview.slides.iter() {
            assert_eq!(slide.format, navy().format);
            assert_eq!(slide.background, navy().background);
        }
        assert_eq!(preview.copyright, None);
        assert_eq!(o.theme_label().as_deref(), Some("Theme: Evening Navy"));
        // Live keeps its scene: no cue was sent and the item is unchanged.
        assert_eq!(live_key(o), Some(0));
        assert_eq!(submitted(o), sent, "applying a theme sends nothing");
        assert_eq!(o.on_screen(), Some("Signal Hymn · Verse 1"));
        assert!(
            o.live
                .as_ref()
                .unwrap()
                .slides
                .iter()
                .all(|s| s.format == SlideFormat::default() && s.background.is_none())
        );
    });
    assert_eq!(
        Repository::open(&path)
            .unwrap()
            .song_theme(
                operator
                    .read_with(&cx, |o, _| version_of(o, "Signal Hymn"))
                    .id
            )
            .unwrap(),
        Some(navy().id)
    );
    // Go Live is what puts the theme on the audience, acknowledged.
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "themed verse acknowledged", |o| {
        live_key(o) == Some(navy().key())
            && submitted(o) > sent
            && o.on_screen() == Some("Signal Hymn · Verse 1")
    });
}

#[gpui::test]
fn a_theme_edit_while_live_reaches_live_only_when_recued(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_themes(&dir);
    {
        let mut repository = Repository::open(&path).unwrap();
        let song = repository
            .catalog(None)
            .unwrap()
            .into_iter()
            .find(|(_, t)| t == "Signal Hymn")
            .unwrap()
            .0;
        repository.set_song_theme(song.id, Some(navy().id)).unwrap();
    }
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let row = song_row(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, row);
    settle(&mut cx, &operator, "navy preview", |o| {
        preview_key(o) == Some(navy().key()) && o.preview.as_ref().unwrap().resolved.is_some()
    });
    click(&mut cx, "live-output");
    settle(&mut cx, &operator, "output connection", connected);
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "navy live", |o| {
        live_key(o) == Some(navy().key()) && o.on_screen().is_some()
    });
    let sent = operator.read_with(&cx, |o, _| submitted(o));

    // Edit the theme in the theme editor: Reset Look, then OK.
    open_themes(&mut cx, &operator, 3);
    select_theme(&mut cx, &operator, "Evening Navy");
    let before = editor_windows(&cx);
    click(&mut cx, "theme-edit");
    let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
    assert_eq!(
        editor_cx.window_title().as_deref(),
        Some("Theme Editor - Evening Navy")
    );
    assert_eq!(
        editor.read_with(&editor_cx, |e, cx| e.probe_theme(cx)),
        Some(navy())
    );
    editor_cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.probe_action(crate::song_library::THEME_RESET, window, cx)
        })
    });
    let reset = editor
        .read_with(&editor_cx, |e, cx| e.probe_theme(cx))
        .unwrap();
    assert!(reset.is_neutral(), "{reset:?}");
    assert_eq!(reset.name, "Evening Navy");
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(15, window, cx)));
    settle_editor(&mut editor_cx, &editor, |(_, _, busy)| !busy);
    settle(&mut cx, &operator, "library theme reloaded", |o| {
        o.themes.set.get(navy().id).is_some_and(|t| t.is_neutral())
    });
    // The library preview follows; Live does not, and nothing was sent.
    settle(&mut cx, &operator, "preview follows the edit", |o| {
        preview_key(o) == Some(0)
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(live_key(o), Some(navy().key()), "Live keeps the old look");
        assert_eq!(submitted(o), sent);
    });
    click(&mut cx, "go-live");
    settle(&mut cx, &operator, "re-cued with the edited theme", |o| {
        live_key(o) == Some(0) && submitted(o) > sent
    });
}

#[gpui::test]
fn schedule_items_snapshot_their_theme_and_files_keep_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_themes(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    add_song(&mut cx, &operator, "Signal Hymn");
    add_song(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, "schedule-item-0");
    settle(&mut cx, &operator, "entry preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.entry.is_some())
    });
    // First shown: the entry captured the library theme (built-in).
    operator.read_with(&cx, |o, _| {
        let entry = &o.schedule.entries()[0];
        let snapshot = entry.theme.as_ref().unwrap();
        assert!(snapshot.theme.is_built_in() && !snapshot.explicit);
    });

    // Apply Evening Navy to the item: an explicit snapshot, a schedule edit.
    open_themes(&mut cx, &operator, 3);
    select_theme(&mut cx, &operator, "Evening Navy");
    click(&mut cx, "theme-apply-item");
    settle(&mut cx, &operator, "item restyled", |o| {
        preview_key(o) == Some(navy().key())
    });
    operator.read_with(&cx, |o, _| {
        let snapshot = o.schedule.entries()[0].theme.clone().unwrap();
        assert!(snapshot.explicit);
        assert_eq!(snapshot.theme, navy());
        assert!(o.schedule.entries()[1].theme.is_none(), "never shown yet");
    });

    // A later library edit never reaches the snapshot.
    let edited = Theme {
        background: Some(Background::color([90, 0, 0])),
        ..navy()
    };
    Repository::open(&path)
        .unwrap()
        .save_theme(&edited)
        .unwrap();
    operator.update(&mut cx, |o, _| o.themes.stale = true);
    settle(&mut cx, &operator, "library reloaded", |o| {
        o.themes.set.get(navy().id) == Some(&edited)
    });
    operator.read_with(&cx, |o, _| {
        assert_eq!(
            preview_key(o),
            Some(navy().key()),
            "the entry keeps its snapshot"
        );
        assert_eq!(
            o.schedule.entries()[0].theme.as_ref().unwrap().theme,
            navy()
        );
    });

    // Save: every item carries a snapshot, the unshown one too.
    cx.simulate_keystrokes("ctrl-s");
    let file = dir.path().join("Themed.selaschedule");
    let typed = file.clone();
    cx.simulate_new_path_selection(move |_| Some(typed));
    settle(&mut cx, &operator, "saved", |o| {
        o.schedule.path().is_some() && !o.schedule.is_dirty()
    });
    let stored = sela::schedule_file::read(&file).unwrap();
    let snapshots: Vec<_> = stored
        .items
        .iter()
        .map(|i| i.theme.clone().map(|t| (t.theme.name, t.explicit)))
        .collect();
    assert_eq!(
        snapshots,
        [
            Some(("Evening Navy".to_owned(), true)),
            Some(("Default".to_owned(), false)),
        ]
    );
    assert_eq!(stored.items[0].theme.as_ref().unwrap().theme, navy());

    // Clear the item's theme, then reopen the file: the snapshot is back
    // and the schedule is clean.
    click(&mut cx, "theme-item-song-theme");
    assert!(operator.read_with(&cx, |o, _| o.schedule.is_dirty()));
    click(&mut cx, "save-schedule");
    settle(&mut cx, &operator, "saved again", |o| {
        !o.schedule.is_dirty()
    });
    open_file(&mut cx, &operator, &file);
    settle(&mut cx, &operator, "reopened", |o| {
        o.schedule_message.as_deref() == Some("Opened “Themed.selaschedule”")
    });
    operator.read_with(&cx, |o, _| {
        assert!(!o.schedule.is_dirty());
        // Cleared and saved again, the item captured the library theme
        // (now the edited Evening Navy is not the song's theme: built-in).
        let first = o.schedule.entries()[0].theme.clone().unwrap();
        assert!(first.theme.is_built_in() && !first.explicit);
    });
}

#[gpui::test]
fn the_theme_editor_previews_undoes_resets_and_cancel_discards(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_themes(&dir);
    let (mut cx, operator, _) = with_library(cx, Some(path.clone()), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    open_themes(&mut cx, &operator, 3);

    // New Theme opens an unsaved theme editor with the built-in look.
    let before = editor_windows(&cx);
    click(&mut cx, "theme-new");
    let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
    assert_eq!(
        editor_cx.window_title().as_deref(),
        Some("Theme Editor - New Theme")
    );
    let theme = |cx: &VisualTestContext| editor.read_with(cx, |e, cx| e.probe_theme(cx)).unwrap();
    let fresh = theme(&editor_cx);
    assert!(fresh.is_neutral() && fresh.name == "New Theme");
    assert!(editor_cx.debug_bounds("theme-margin-left").is_some());
    // The draft's warnings come from the same off-thread check as the tab.
    let end = Instant::now() + Duration::from_secs(10);
    while editor
        .read_with(&editor_cx, |e, _| e.probe_theme_warnings())
        .is_none()
    {
        editor_cx.run_until_parked();
        assert!(Instant::now() < end, "bounded wait for the theme check");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        editor.read_with(&editor_cx, |e, _| e.probe_theme_warnings()),
        Some(Vec::new())
    );
    assert!(editor_cx.debug_bounds("format-EditLayouts").is_none());

    // Bold through the shared Format pane, margins, copyright: undoable.
    let bounds = editor_cx.debug_bounds("format-Bold").unwrap();
    editor_cx.simulate_click(bounds.center(), Default::default());
    assert_eq!(theme(&editor_cx).format.bold, Some(true));
    editor_cx.update(|_, cx| editor.update(cx, |e, cx| e.probe_margin(0, "200", cx)));
    assert_eq!(theme(&editor_cx).format.margins.map(|m| m.left), Some(200));
    editor_cx.update(|_, cx| editor.update(cx, |e, cx| e.probe_margin(1, "401", cx)));
    assert_eq!(
        theme(&editor_cx).format.margins.map(|m| m.right),
        Some(32),
        "out of range is refused"
    );
    assert!(
        editor
            .read_with(&editor_cx, |e, _| e.probe_status())
            .starts_with("Margins are whole numbers")
    );
    editor_cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.probe_action(crate::song_library::THEME_COPYRIGHT, window, cx)
        })
    });
    assert!(!theme(&editor_cx).show_copyright);
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(5, window, cx)));
    assert!(
        theme(&editor_cx).show_copyright,
        "undo restores the element"
    );
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(5, window, cx)));
    assert_eq!(theme(&editor_cx).format.margins, None, "undo the margin");
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(6, window, cx)));
    assert_eq!(theme(&editor_cx).format.margins.map(|m| m.left), Some(200));
    // Reset is one undo step back to the built-in look.
    editor_cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.probe_action(crate::song_library::THEME_RESET, window, cx)
        })
    });
    assert!(theme(&editor_cx).is_neutral());
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(5, window, cx)));
    assert_eq!(theme(&editor_cx).format.bold, Some(true));

    // Cancel with changes asks; No discards and nothing is stored.
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(16, window, cx)));
    assert!(editor.read_with(&editor_cx, |e, _| e.probe_close_prompt()));
    editor_cx.update(|window, cx| editor.update(cx, |e, cx| e.probe_action(10, window, cx)));
    cx.run_until_parked();
    assert!(editor_windows(&cx).len() == before.len(), "closed");
    assert_eq!(
        Repository::open(&path)
            .unwrap()
            .themes()
            .unwrap()
            .themes
            .len(),
        3
    );
    // The live scene and output were never touched.
    operator.read_with(&cx, |o, _| assert!(o.output.is_none() && o.live.is_none()));

    // A named new theme saves with OK and is listed.
    let before = editor_windows(&cx);
    click(&mut cx, "theme-new");
    let (mut editor_cx, editor) = opened_editor(&mut cx, &before, |(_, _, busy)| !busy);
    let bounds = editor_cx.debug_bounds("format-Italic").unwrap();
    editor_cx.simulate_click(bounds.center(), Default::default());
    let title = editor.read_with(&editor_cx, |e, _| e.probe_title_field());
    editor_cx.update(|window, cx| {
        use gpui::EntityInputHandler;
        title.update(cx, |field, cx| {
            let end = field.text().encode_utf16().count();
            field.replace_text_in_range(Some(0..end), "Italic Morning", window, cx);
        });
        editor.update(cx, |e, cx| e.probe_action(15, window, cx));
    });
    settle_editor(&mut editor_cx, &editor, |(_, _, busy)| !busy);
    settle(&mut cx, &operator, "new theme listed", |o| {
        o.themes
            .set
            .themes
            .iter()
            .any(|t| t.name == "Italic Morning")
    });
    let stored = Repository::open(&path).unwrap().themes().unwrap();
    let saved = stored
        .themes
        .iter()
        .find(|t| t.name == "Italic Morning")
        .unwrap();
    assert_eq!(saved.format.italic, Some(true));
    assert!(saved.id != ThemeId::BUILT_IN);
}

#[gpui::test]
fn missing_fonts_and_themes_are_reported_in_preview_not_hidden(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = library_with_themes(&dir);
    let gone = Theme::new("Gone");
    {
        let mut repository = Repository::open(&path).unwrap();
        let catalog = repository.catalog(None).unwrap();
        let id = |title: &str| catalog.iter().find(|(_, t)| t == title).unwrap().0.id;
        repository
            .set_song_theme(id("Signal Hymn"), Some(missing_face().id))
            .unwrap();
        repository.save_theme(&gone).unwrap();
        repository
            .set_song_theme(id(SECOND_SONG), Some(gone.id))
            .unwrap();
        repository.delete_theme(gone.id).unwrap();
    }
    let (mut cx, operator, _) = with_library(cx, Some(path), "apply");
    cx.simulate_resize(size(px(1400.), px(900.)));
    settle(&mut cx, &operator, "catalog", |o| o.catalog.len() == 2);
    let row = song_row(&mut cx, &operator, "Signal Hymn");
    click(&mut cx, row);
    settle(&mut cx, &operator, "font fallback reported", |o| {
        o.theme_notes() == ["Font “Sela Missing Face” is not installed · showing DejaVu Sans"]
    });
    let row = song_row(&mut cx, &operator, SECOND_SONG);
    click(&mut cx, row);
    settle(&mut cx, &operator, "missing theme reported", |o| {
        o.preview.as_ref().is_some_and(|p| p.title == SECOND_SONG)
            && o.theme_notes() == ["This song's theme is missing · showing the default theme"]
    });
    assert_eq!(operator.read_with(&cx, |o, _| preview_key(o)), Some(0));
}
