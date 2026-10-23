//! Stage (Foldback) output E2E with fake renderer children (M3-01/M3-02):
//! the real Operator, its two supervisors and the real wire, with children
//! that mirror the renderers' routing rules and log every scene they get.
use super::*;
use crate::operator::{Dialog, StageLauncher};
use sela::{output::Loss, scene::ContentVersion, settings};

/// `tests::settle` with a longer bound: every rig starts two renderer
/// children (each a fresh test-binary process), which takes several seconds
/// when the whole suite runs on a loaded machine. Shadows the parent's.
fn settle(
    cx: &mut VisualTestContext,
    operator: &Entity<Operator>,
    what: &str,
    done: impl Fn(&Operator) -> bool,
) {
    settle_or_report(cx, operator, what, done, String::new);
}

/// `settle` whose timeout message also carries `extra` (the children's
/// logs) after the operator's view of both outputs, so a failure on a CI
/// machine shows where a cue stopped.
fn settle_or_report(
    cx: &mut VisualTestContext,
    operator: &Entity<Operator>,
    what: &str,
    done: impl Fn(&Operator) -> bool,
    extra: impl Fn() -> String,
) {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        operator.update(cx, |o, cx| o.poll(cx));
        cx.run_until_parked();
        if operator.read_with(cx, |o, _| done(o)) {
            return;
        }
        if Instant::now() >= end {
            let report = operator.read_with(cx, |o, _| outputs_report(o));
            panic!("bounded wait for {what}\n{report}{}", extra());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What the operator knows about both outputs: status, receipts, counters,
/// the panel it wants on the stage, and the Live pane's stage lines.
fn outputs_report(o: &Operator) -> String {
    let supervisor = |name: &str, output: Option<&sela::output::Supervisor>| match output {
        None => format!("{name}: off\n"),
        Some(s) => format!(
            "{name}: {:?}; live {:?}; in flight {:?}; wanted {:?}; rejected {:?}; {:?}\n",
            s.status(),
            s.live(),
            s.in_flight(),
            s.wanted(),
            s.rejected(),
            s.counters(),
        ),
    };
    format!(
        "{}{}on screen: {:?}\nstage panel wanted: {:?}\nstage lines: {:?}\n",
        supervisor("audience", o.output.as_ref()),
        supervisor("stage", o.stage.output.as_ref()),
        o.on_screen(),
        o.stage_panel(),
        o.stage_lines(),
    )
}

/// Renderer stand-in for either output. `SELA_FAKE_ROLE` is `audience` or
/// `stage`; like the real renderers, an audience refuses stage cues and a
/// stage refuses anything else. Every scene is appended to `SELA_FAKE_LOG`
/// (one file per role, see `log_of`) with a single write per line: role, kind (`stage`/`scene`), the stage fields (or the
/// audience text) with newlines shown as `|`, then the face it carries
/// (`face_field`). `SELA_FAKE_MODE`: `apply`, `exit-after:<n>` (exit
/// after the nth applied scene), `reject-at:<n>` (fail the nth scene), or
/// `exit` (exit right after reporting the surface).
#[test]
#[ignore = "subprocess fixture for stage output tests"]
fn fake_output_child() {
    use std::io::Write;
    let role = std::env::var("SELA_FAKE_ROLE").unwrap();
    let mode = std::env::var("SELA_FAKE_MODE").unwrap();
    let log = std::env::var("SELA_FAKE_LOG").unwrap();
    let epoch =
        Epoch(u128::from_str_radix(&std::env::var("SELA_FAKE_EPOCH").unwrap(), 16).unwrap());
    let number = |prefix: &str| mode.strip_prefix(prefix).map(|n| n.parse::<u64>().unwrap());
    let (exit_after, reject_at) = (number("exit-after:"), number("reject-at:"));
    let caps = RendererCapabilities {
        max_texture_dimension: 2048,
    };
    let out = || std::io::stderr();
    Frame::ready(epoch, 2048).write(out()).unwrap();
    Frame::surface(epoch, FAKE_EXTENT).write(out()).unwrap();
    if mode == "exit" {
        std::process::exit(0);
    }
    let stage_role = role == "stage";
    let mut session = RendererSession::new(epoch);
    let mut scenes = 0u64;
    while let Ok(frame) = Frame::read(std::io::stdin()) {
        let now = Instant::now();
        let stamp = frame.command_stamp().unwrap();
        let lane = frame.command_lane().unwrap();
        let ack = match frame.into_command(now, caps) {
            Ok(command) => {
                if let Payload::Scene(cue) = command.payload() {
                    let line = match cue.stage() {
                        Some(stage) => {
                            let p = stage.panel();
                            format!(
                                "{role}\tstage\t{}\t{}\t{}\t{}\t{}\t{}{}\t{}",
                                p.current_label,
                                p.current,
                                p.next_label,
                                p.next,
                                p.message,
                                u8::from(p.show_clock),
                                u8::from(p.show_next),
                                face_field(stage.font_version(), stage.face_index()),
                            )
                        }
                        None => format!(
                            "{role}\tscene\t{}\t{}",
                            cue.text().map(|t| t.content()).unwrap_or_default(),
                            cue.text()
                                .map(|t| face_field(t.font_version(), t.face_index()))
                                .unwrap_or_default()
                        ),
                    };
                    // One append per line: `writeln!` issues the text and the
                    // newline as two writes, and a stage child restarted
                    // while the old one is being killed shares this file.
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&log)
                        .unwrap();
                    let line = format!("{}\n", line.replace('\n', "|"));
                    file.write_all(line.as_bytes()).unwrap();
                }
                session.accept(command, now)
            }
            Err(_) => session.reject_preparation(stamp, lane),
        };
        Frame::acknowledgment(ack).write(out()).unwrap();
        if ack.outcome == Outcome::Accepted {
            let mut applied_scene = false;
            let ack = session
                .present(Instant::now(), |payload| match payload {
                    Payload::Mask(_) | Payload::Logo(_) if stage_role => {
                        Err(DeliveryError::RenderFailed)
                    }
                    Payload::Mask(Layer::Logo) if session_has_no_logo() => {
                        Err(DeliveryError::RenderFailed)
                    }
                    Payload::Mask(_) | Payload::Logo(_) => Ok(()),
                    // No video plays on either fake output.
                    Payload::Media(_) => Err(DeliveryError::RenderFailed),
                    Payload::Scene(cue) => {
                        if cue.stage().is_some() != stage_role {
                            return Err(DeliveryError::RenderFailed);
                        }
                        scenes += 1;
                        if reject_at == Some(scenes) {
                            return Err(DeliveryError::RenderFailed);
                        }
                        applied_scene = true;
                        Ok(())
                    }
                })
                .unwrap();
            Frame::acknowledgment(ack).write(out()).unwrap();
            if applied_scene && exit_after == Some(scenes) {
                std::process::exit(0);
            }
        }
    }
    std::process::exit(0);
}

/// A face as the fake children log it: font version id and revision, and
/// the face index.
fn face_field(version: ContentVersion, index: u32) -> String {
    format!("{:x}:{}:{index}", version.id, version.revision)
}

/// The audience tests here never send a logo, so a Logo mask always fails
/// like the renderer without one.
fn session_has_no_logo() -> bool {
    true
}

/// Each role's log in the rig's log folder. Separate files: the audience and
/// stage children get their cues at the same moment, and two processes
/// appending to one file can interleave a line's text and its newline,
/// hiding the stage line at the end of an audience line (the Windows CI
/// symptom: a panel the stage acknowledged never showed in the log).
fn log_of(logs: &std::path::Path, role: &str) -> std::path::PathBuf {
    logs.join(format!("{role}.log"))
}

fn output_command(role: &'static str, mode: &'static str, logs: &std::path::Path) -> Launch {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "tests::stage_display::fake_output_child",
            "--ignored",
            "--nocapture",
        ])
        .env("SELA_FAKE_ROLE", role)
        .env("SELA_FAKE_MODE", mode)
        .env("SELA_FAKE_LOG", log_of(logs, role));
    Launch {
        command,
        frames: Stream::Stderr,
    }
}

fn audience_launcher(mode: &'static str, log: std::path::PathBuf) -> Launcher {
    Arc::new(move |epoch| {
        let mut launch = output_command("audience", mode, &log);
        launch
            .command
            .env("SELA_FAKE_EPOCH", format!("{:x}", epoch.0));
        launch
    })
}

/// Records each stage spawn's monitor argument.
fn stage_launcher(
    modes: Vec<&'static str>,
    log: std::path::PathBuf,
    monitors: Arc<std::sync::Mutex<Vec<String>>>,
) -> StageLauncher {
    let spawned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    Arc::new(move |epoch, monitor| {
        monitors.lock().unwrap().push(monitor);
        let n = spawned.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mode = modes[n.min(modes.len() - 1)];
        let mut launch = output_command("stage", mode, &log);
        launch
            .command
            .env("SELA_FAKE_EPOCH", format!("{:x}", epoch.0));
        launch
    })
}

struct Rig {
    cx: VisualTestContext,
    operator: Entity<Operator>,
    /// Folder of the children's logs (`log_of`), inside the profile folder.
    log: std::path::PathBuf,
    monitors: Arc<std::sync::Mutex<Vec<String>>>,
    _dir: tempfile::TempDir,
}

/// Operator over `library` with Foldback enabled in the stored settings and
/// fake children for both outputs. `override_placement` mirrors
/// `SELA_STAGE_MONITOR` (no monitor list needed).
fn rig(
    cx: &mut TestAppContext,
    songs: fn(&tempfile::TempDir) -> std::path::PathBuf,
    audience: &'static str,
    stage: Vec<&'static str>,
    stored: &str,
    override_placement: bool,
    monitors: Vec<settings::MonitorInfo>,
) -> Rig {
    rig_with(
        cx,
        songs,
        (audience, stage),
        stored,
        override_placement,
        monitors,
        None,
    )
}

/// `rig` with a settings loader of the test's own (the settle then waits
/// for the catalog only).
fn rig_with(
    cx: &mut TestAppContext,
    songs: fn(&tempfile::TempDir) -> std::path::PathBuf,
    (audience, stage): (&'static str, Vec<&'static str>),
    stored: &str,
    override_placement: bool,
    monitors: Vec<settings::MonitorInfo>,
    loader: Option<settings::Loader>,
) -> Rig {
    let wait_loaded = loader.is_none();
    let dir = tempfile::tempdir().unwrap();
    let library = songs(&dir);
    std::fs::write(
        settings::path_in(dir.path()),
        format!("sela-settings 1\n{stored}"),
    )
    .unwrap();
    let log = dir.path().join("outputs");
    std::fs::create_dir(&log).unwrap();
    let spawned = Arc::new(std::sync::Mutex::new(Vec::new()));
    let launcher = stage_launcher(stage, log.clone(), spawned.clone());
    let mut config = crate::operator::SettingsConfig {
        list_monitors: Arc::new(move || monitors.clone()),
        ..Default::default()
    };
    if let Some(loader) = loader {
        config.loader = loader;
    }
    let audience = audience_launcher(audience, log.clone());
    let window = cx.update(|cx| {
        bind_operator_keys(cx);
        cx.open_window(Default::default(), |window, cx| {
            let operator = cx.new(|cx| {
                let mut operator = Operator::with_settings(Some(library), audience, config, cx);
                operator.set_stage_launcher(launcher, override_placement);
                operator
            });
            operator.read(cx).focus.clone().focus(window, cx);
            cx.new(|_| DispatchProbe {
                operator,
                seen: Rc::new(Cell::new(0)),
            })
        })
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let operator = window
        .root(&mut visual)
        .unwrap()
        .read_with(&visual, |p, _| p.operator.clone());
    visual.simulate_resize(size(px(1280.), px(800.)));
    settle(&mut visual, &operator, "settings and catalog", |o| {
        (o.settings_loaded_for_options() || !wait_loaded) && !o.catalog.is_empty()
    });
    Rig {
        cx: visual,
        operator,
        log,
        monitors: spawned,
        _dir: dir,
    }
}

const ENABLED: &str = "stage_enabled=1\n";

fn lines(log: &std::path::Path, role: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(log_of(log, role))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with(&format!("{role}\t")))
        .map(|line| line.split('\t').skip(1).map(str::to_owned).collect())
        .collect()
}

/// The newest panel the stage child received: label, current, next label,
/// next, message, flags.
fn last_panel(log: &std::path::Path) -> Option<Vec<String>> {
    lines(log, "stage").pop().map(|mut fields| {
        assert_eq!(
            fields.remove(0),
            "stage",
            "stage child got a non-stage scene"
        );
        fields
    })
}

fn stage_connected(o: &Operator) -> bool {
    matches!(
        o.stage.output.as_ref().map(|s| s.status()),
        Some(Status::Connected { .. })
    )
}

fn stage_shows(o: &Operator, label: &str) -> bool {
    o.stage_lines()
        .iter()
        .any(|(line, _)| line.ends_with(&format!("showing {label}")))
}

fn wait_for_panel(rig: &mut Rig, what: &str, done: impl Fn(&[String]) -> bool) -> Vec<String> {
    let log = rig.log.clone();
    settle_or_report(
        &mut rig.cx,
        &rig.operator,
        what,
        |_| last_panel(&log).is_some_and(|panel| done(&panel)),
        || log_tails(&log),
    );
    last_panel(&log).unwrap()
}

/// The last lines each child logged, for a timeout message.
fn log_tails(log: &std::path::Path) -> String {
    let mut report = String::new();
    for role in ["audience", "stage"] {
        let path = log_of(log, role);
        let text = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => format!("(unreadable: {error})"),
        };
        let all: Vec<&str> = text.lines().collect();
        report.push_str(&format!("{role} log, last of {} lines:\n", all.len()));
        for line in &all[all.len().saturating_sub(10)..] {
            report.push_str(&format!("  {line}\n"));
        }
    }
    report
}

/// No audience scene ever carried stage content.
fn assert_audience_private(log: &std::path::Path) {
    for fields in lines(log, "audience") {
        assert_eq!(fields[0], "scene", "audience child received {fields:?}");
        for private in ["Two minutes", "Next ·", "End of schedule"] {
            assert!(
                !fields.iter().any(|field| field.contains(private)),
                "{private} reached the audience: {fields:?}"
            );
        }
    }
}

#[gpui::test]
fn stage_follows_go_live_and_next_with_its_own_receipts(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    // Before anything is live the stage shows its idle panel with the clock.
    let idle = wait_for_panel(&mut rig, "idle panel", |p| p[0] == "Nothing live");
    assert_eq!(idle[5], "11", "clock and next area on by default");
    settle(&mut rig.cx, &operator, "idle panel acknowledged", |o| {
        stage_shows(o, "Nothing live")
    });

    click(&mut rig.cx, "song-0");
    settle(&mut rig.cx, &operator, "preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    });
    let before = rig
        .operator
        .read_with(&rig.cx, |o, _| o.stage.output.as_ref().unwrap().counters());
    click(&mut rig.cx, "go-live");
    let panel = wait_for_panel(&mut rig, "verse on stage", |p| {
        p[0] == "Signal Hymn · Verse 1"
    });
    assert_eq!(
        panel[1..4],
        [
            "First original line|Second original line",
            "Next · Chorus",
            "Original refrain"
        ]
    );
    settle(&mut rig.cx, &operator, "both receipts", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1") && stage_shows(o, "Signal Hymn · Verse 1")
    });
    // Independent sessions: different epochs and their own counters.
    operator.read_with(&rig.cx, |o, _| {
        let stage = o.stage.output.as_ref().unwrap();
        assert_ne!(stage.epoch(), o.output.as_ref().unwrap().epoch());
        assert!(stage.counters().applied > before.applied);
    });

    click(&mut rig.cx, "live-next");
    let panel = wait_for_panel(&mut rig, "chorus on stage", |p| {
        p[0] == "Signal Hymn · Chorus"
    });
    assert_eq!(
        panel[1..4],
        ["Original refrain", "Next · Bridge", "Original bridge"]
    );
    click(&mut rig.cx, "live-next");
    // Last slide of a library song (not in the schedule): nothing follows.
    let panel = wait_for_panel(&mut rig, "bridge on stage", |p| {
        p[0] == "Signal Hymn · Bridge"
    });
    assert_eq!(panel[2..4], ["End of schedule", ""]);
    settle(&mut rig.cx, &operator, "bridge both", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge") && stage_shows(o, "Signal Hymn · Bridge")
    });
    let audience = lines(&rig.log, "audience");
    assert_eq!(
        audience.len(),
        3,
        "one audience cue per slide: {audience:?}"
    );
    assert_eq!(audience[2][1], "Original bridge");
    assert_audience_private(&rig.log);

    // Live output off stops both; the stage never outlives it.
    click(&mut rig.cx, "live-output");
    rig.cx.run_until_parked();
    operator.read_with(&rig.cx, |o, _| {
        assert!(o.output.is_none() && o.stage.output.is_none());
    });
}

#[gpui::test]
fn a_lost_stage_never_touches_the_audience_and_restarts_alone(cx: &mut TestAppContext) {
    // The first stage child exits after its second applied panel.
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["exit-after:2", "apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    click(&mut rig.cx, "song-0");
    settle(&mut rig.cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut rig.cx, "go-live");
    settle(&mut rig.cx, &operator, "stage lost after the verse", |o| {
        matches!(
            o.stage.output.as_ref().map(|s| s.status()),
            Some(Status::Lost(Loss::Exited))
        )
    });
    let audience_epoch = operator.read_with(&rig.cx, |o, _| o.output.as_ref().unwrap().epoch());
    settle(&mut rig.cx, &operator, "verse on the audience", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1")
    });
    assert!(operator.read_with(&rig.cx, |o, _| {
        o.live_lines()
            .iter()
            .any(|(line, _)| line.starts_with("Stage output lost: its process ended"))
    }));
    // The audience keeps working: Next is applied and acknowledged.
    click(&mut rig.cx, "live-next");
    settle(&mut rig.cx, &operator, "chorus on the audience", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
    let sent = operator.read_with(&rig.cx, |o, _| {
        assert!(connected(o));
        assert_eq!(o.output.as_ref().unwrap().epoch(), audience_epoch);
        submitted(o)
    });

    // One Live › Show Stage Output, as the lost status says, restarts it: a
    // fresh stage session that gets the current slide without anything
    // being re-sent to the audience (review finding: it used to only stop
    // the lost session and needed a second activation).
    assert!(operator.read_with(&rig.cx, |o, _| {
        o.stage_lines()
            .iter()
            .any(|(line, _)| line.ends_with("Live › Show Stage Output restarts it"))
    }));
    let lost_epoch = operator.read_with(&rig.cx, |o, _| o.stage.output.as_ref().unwrap().epoch());
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage");
    operator.read_with(&rig.cx, |o, _| {
        let stage = o.stage.output.as_ref().expect("restarted, not stopped");
        assert_ne!(stage.epoch(), lost_epoch);
        assert!(!matches!(stage.status(), Status::Lost(_)));
    });
    let panel = wait_for_panel(&mut rig, "restarted stage", |p| {
        p[0] == "Signal Hymn · Chorus"
    });
    assert_eq!(panel[2], "Next · Bridge");
    settle(
        &mut rig.cx,
        &operator,
        "restarted stage acknowledged",
        |o| stage_connected(o) && stage_shows(o, "Signal Hymn · Chorus"),
    );
    operator.read_with(&rig.cx, |o, _| {
        assert_eq!(
            submitted(o),
            sent,
            "the stage restart sent nothing to the audience"
        );
        assert_eq!(o.output.as_ref().unwrap().epoch(), audience_epoch);
    });
    assert_eq!(rig.monitors.lock().unwrap().len(), 2);
    assert_audience_private(&rig.log);
}

#[gpui::test]
fn a_lost_audience_leaves_the_stage_running(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "exit",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "audience lost, stage up", |o| {
        matches!(o.output.as_ref().map(|s| s.status()), Some(Status::Lost(_))) && stage_connected(o)
    });
    wait_for_panel(&mut rig, "idle stage panel", |p| p[0] == "Nothing live");
    settle(&mut rig.cx, &operator, "stage acknowledged", |o| {
        stage_shows(o, "Nothing live")
    });
    for _ in 0..20 {
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(operator.read_with(&rig.cx, |o, _| stage_connected(o)));
}

#[gpui::test]
fn the_stage_message_is_private_until_ok_and_never_reaches_the_audience(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    click(&mut rig.cx, "song-0");
    settle(&mut rig.cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut rig.cx, "go-live");
    wait_for_panel(&mut rig, "verse on stage", |p| {
        p[0] == "Signal Hymn · Verse 1"
    });

    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage-message");
    assert_eq!(
        operator.read_with(&rig.cx, |o, _| o.dialog.clone()),
        Some(Dialog::StageMessage)
    );
    // Typing goes to the text field: no show shortcut fires, nothing is sent.
    rig.cx.simulate_input("Two minutes b");
    for _ in 0..10 {
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(last_panel(&rig.log).unwrap()[4], "");
    assert!(!operator.read_with(&rig.cx, |o, _| o.masks.any()));
    rig.cx.simulate_keystrokes("backspace backspace");
    click(&mut rig.cx, "dialog-confirm");
    let panel = wait_for_panel(&mut rig, "message on stage", |p| p[4] == "Two minutes");
    assert_eq!(panel[0], "Signal Hymn · Verse 1");
    assert!(operator.read_with(&rig.cx, |o, _| {
        o.dialog.is_none()
            && o.live_lines()
                .iter()
                .any(|(line, _)| line == "Stage message: Two minutes")
    }));
    // A slide change keeps the message; the audience never sees it.
    click(&mut rig.cx, "live-next");
    wait_for_panel(&mut rig, "chorus keeps the message", |p| {
        p[0] == "Signal Hymn · Chorus" && p[4] == "Two minutes"
    });
    // Cancel discards a new draft; Clear removes the message.
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage-message");
    rig.cx.simulate_input(" now");
    click(&mut rig.cx, "dialog-cancel");
    assert_eq!(
        operator.read_with(&rig.cx, |o, _| o.stage.message.clone()),
        "Two minutes"
    );
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage-message");
    click(&mut rig.cx, "stage-message-clear");
    wait_for_panel(&mut rig, "message cleared", |p| {
        p[0] == "Signal Hymn · Chorus" && p[4].is_empty()
    });
    assert_audience_private(&rig.log);
}

#[gpui::test]
fn the_last_slide_points_at_the_next_schedule_item(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_songs,
        "apply",
        vec!["apply"],
        "stage_enabled=1\nstage_clock=0\n",
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    settle(&mut rig.cx, &operator, "two songs", |o| {
        o.catalog.len() == 2
    });
    add_song(&mut rig.cx, &operator, "Signal Hymn");
    add_song(&mut rig.cx, &operator, SECOND_SONG);
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    rig.cx.simulate_keystrokes("down");
    settle(&mut rig.cx, &operator, "first item previewed", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry.is_some() && p.title == "Signal Hymn")
    });
    click(&mut rig.cx, "preview-slide-2");
    click(&mut rig.cx, "go-live");
    let panel = wait_for_panel(&mut rig, "bridge on stage", |p| {
        p[0] == "Signal Hymn · Bridge"
    });
    assert_eq!(
        panel[2..4],
        [format!("Next item · {SECOND_SONG}"), String::new()]
    );
    assert_eq!(panel[5], "01", "clock off, next area on");
    // The live entry leaves the schedule: nothing is "next" any more.
    operator.update(&mut rig.cx, |o, _| {
        let id = o.schedule.entries()[0].id;
        o.schedule.remove(id);
    });
    let panel = wait_for_panel(&mut rig, "end after removal", |p| p[2] == "End of schedule");
    assert_eq!(panel[0], "Signal Hymn · Bridge");
    assert_audience_private(&rig.log);
}

/// "Arranged": Verse, Chorus, Bridge, arranged as Full (V C B) and Short
/// (C C, a repeated chorus).
fn library_with_arrangements(dir: &tempfile::TempDir) -> std::path::PathBuf {
    use sela::arrangement::{Occurrence, OccurrenceId, Variant, VariantId};
    let path = dir.path().join("library.sqlite");
    let section = |label: &str, lyrics: &str| Section {
        id: SectionId::allocate(),
        label: label.into(),
        lyrics: lyrics.into(),
        format: Default::default(),
        background: None,
    };
    let sections = vec![
        section("Verse", "verse words"),
        section("Chorus", "chorus words"),
        section("Bridge", "bridge words"),
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
    Repository::open(&path)
        .unwrap()
        .save_song(
            None,
            Song {
                title: "Arranged".into(),
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
    path
}

#[gpui::test]
fn arrangement_changes_and_repeated_choruses_keep_next_deterministic(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_arrangements,
        "apply",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    add_song(&mut rig.cx, &operator, "Arranged");
    click(&mut rig.cx, "schedule-item-0");
    settle(&mut rig.cx, &operator, "Full preview", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.entry.is_some() && p.slides.len() == 3)
    });
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    click(&mut rig.cx, "go-live");
    let panel = wait_for_panel(&mut rig, "verse on stage", |p| p[0] == "Arranged · Verse");
    assert_eq!(panel[2..4], ["Next · Chorus", "chorus words"]);
    let sent = lines(&rig.log, "stage").len();

    // A rapid arrangement change on the live entry (Preview's arrangement
    // button) changes Preview only: the stage keeps the live snapshot's next.
    click(&mut rig.cx, "preview-arrangement");
    settle(&mut rig.cx, &operator, "Short preview", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 2)
    });
    // Rapid switching, each click landing before the previous rebuild has
    // settled, also leaves the stage alone.
    for _ in 0..6 {
        click(&mut rig.cx, "preview-arrangement");
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
    }
    for _ in 0..10 {
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    if operator.read_with(&rig.cx, |o, _| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 3)
    }) {
        click(&mut rig.cx, "preview-arrangement");
    }
    settle(&mut rig.cx, &operator, "Short preview again", |o| {
        o.preview.as_ref().is_some_and(|p| p.slides.len() == 2)
    });
    assert_eq!(
        lines(&rig.log, "stage").len(),
        sent,
        "no new panel was sent"
    );
    assert_eq!(
        last_panel(&rig.log).unwrap()[2..4],
        ["Next · Chorus", "chorus words"]
    );
    click(&mut rig.cx, "live-next");
    let panel = wait_for_panel(&mut rig, "chorus of Full", |p| p[0] == "Arranged · Chorus");
    assert_eq!(panel[2..4], ["Next · Bridge", "bridge words"]);

    // Short goes live: its first chorus names the repeated chorus as next,
    // then the last slide names the end.
    click(&mut rig.cx, "preview-slide-0");
    click(&mut rig.cx, "go-live");
    let panel = wait_for_panel(&mut rig, "short first chorus", |p| {
        p[0] == "Arranged · Chorus" && p[2] == "Next · Chorus"
    });
    assert_eq!(
        panel[1..4],
        ["chorus words", "Next · Chorus", "chorus words"]
    );
    click(&mut rig.cx, "live-next");
    let panel = wait_for_panel(&mut rig, "short last chorus", |p| {
        p[0] == "Arranged · Chorus" && p[2] == "End of schedule"
    });
    assert_eq!(panel[3], "");
    assert_audience_private(&rig.log);
}

#[gpui::test]
fn a_rejected_panel_keeps_the_previous_one_and_reports(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["reject-at:3"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    click(&mut rig.cx, "song-0");
    settle(&mut rig.cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut rig.cx, "go-live");
    settle(&mut rig.cx, &operator, "verse on both", |o| {
        stage_shows(o, "Signal Hymn · Verse 1")
    });
    click(&mut rig.cx, "live-next");
    settle(&mut rig.cx, &operator, "stage rejection reported", |o| {
        o.stage_lines()
            .iter()
            .any(|(line, _)| line.starts_with("Stage did not show the latest panel"))
    });
    operator.read_with(&rig.cx, |o, _| {
        // The stage still names the verse: the rejected chorus never counts.
        assert!(stage_shows(o, "Signal Hymn · Verse 1"));
        assert!(stage_connected(o));
    });
    settle(&mut rig.cx, &operator, "audience unaffected", |o| {
        o.on_screen() == Some("Signal Hymn · Chorus")
    });
}

#[gpui::test]
fn options_foldback_enables_routes_and_persists_the_stage(cx: &mut TestAppContext) {
    let monitor = |name: &str, primary| settings::MonitorInfo {
        name: name.into(),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        primary,
    };
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        "",
        false,
        vec![
            monitor("P", true),
            monitor("AUD", false),
            monitor("STG", false),
        ],
    );
    let operator = rig.operator.clone();
    settle(&mut rig.cx, &operator, "monitors listed", |o| {
        o.settings.monitors.is_some()
    });
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "audience", connected);
    assert!(operator.read_with(&rig.cx, |o, _| o.stage.output.is_none()));
    // Foldback disabled: the Live menu's stage rows are inert.
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage");
    assert!(operator.read_with(&rig.cx, |o, _| o.stage.output.is_none()));
    rig.cx.simulate_keystrokes("escape");

    rig.cx.simulate_keystrokes("alt-e enter");
    click(&mut rig.cx, "options-section-foldback");
    assert!(rig.cx.debug_bounds("options-foldback-page").is_some());
    assert!(rig.cx.debug_bounds("options-monitor-0").is_none());
    click(&mut rig.cx, "options-stage-enabled");
    // Rows below follow the checkbox: Automatic plus three monitors.
    click(&mut rig.cx, "options-stage-monitor-0");
    click(&mut rig.cx, "dialog-confirm");
    settle(&mut rig.cx, &operator, "stage started with Live on", |o| {
        stage_connected(o)
    });
    // Automatic never lands on the audience monitor (first non-primary).
    assert_eq!(*rig.monitors.lock().unwrap(), ["name:STG".to_string()]);
    let file = settings::path_in(rig.log.parent().unwrap());
    settle(&mut rig.cx, &operator, "settings written", |_| {
        std::fs::read_to_string(&file).is_ok_and(|text| text.contains("stage_enabled=1"))
    });

    // Disabling it in Options stops only the stage.
    let audience_epoch = operator.read_with(&rig.cx, |o, _| o.output.as_ref().unwrap().epoch());
    rig.cx.simulate_keystrokes("alt-e enter");
    click(&mut rig.cx, "options-section-foldback");
    click(&mut rig.cx, "options-stage-enabled");
    click(&mut rig.cx, "dialog-confirm");
    operator.read_with(&rig.cx, |o, _| {
        assert!(o.stage.output.is_none());
        assert!(connected(o));
        assert_eq!(o.output.as_ref().unwrap().epoch(), audience_epoch);
    });
    settle(&mut rig.cx, &operator, "disabled written", |_| {
        std::fs::read_to_string(&file).is_ok_and(|text| !text.contains("stage_enabled"))
    });
}

#[gpui::test]
fn no_free_monitor_keeps_the_stage_off_instead_of_covering_the_audience(cx: &mut TestAppContext) {
    let monitor = |name: &str, primary| settings::MonitorInfo {
        name: name.into(),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        primary,
    };
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        false,
        vec![monitor("P", true), monitor("AUD", false)],
    );
    let operator = rig.operator.clone();
    settle(&mut rig.cx, &operator, "monitors listed", |o| {
        o.settings.monitors.is_some()
    });
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "audience", connected);
    operator.read_with(&rig.cx, |o, _| {
        assert!(o.stage.output.is_none());
        assert!(
            o.live_lines()
                .iter()
                .any(|(line, _)| line.starts_with("No free monitor for the stage output"))
        );
    });
    assert!(rig.monitors.lock().unwrap().is_empty());
}

fn monitor(name: &str, primary: bool) -> settings::MonitorInfo {
    settings::MonitorInfo {
        name: name.into(),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        primary,
    }
}

fn stage_epoch(o: &Operator) -> Option<Epoch> {
    o.stage.output.as_ref().map(|stage| stage.epoch())
}

/// Review blocker: Go Live of another item used to put that item on the
/// stage (with the old slide index) before its cue was handed to the
/// audience, and a failed preparation left that wrong panel on the stage.
#[gpui::test]
fn a_new_item_that_fails_to_prepare_never_changes_the_stage(cx: &mut TestAppContext) {
    use sela::preparation::cue::{Built, CuePreparer};
    let mut rig = rig(
        cx,
        library_with_songs,
        "apply",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    settle(&mut rig.cx, &operator, "two songs", |o| {
        o.catalog.len() == 2
    });
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    let hymn = song_row(&mut rig.cx, &operator, "Signal Hymn");
    let canticle = song_row(&mut rig.cx, &operator, SECOND_SONG);
    click(&mut rig.cx, hymn);
    settle(&mut rig.cx, &operator, "hymn preview", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == "Signal Hymn" && p.resolved.is_some())
    });
    click(&mut rig.cx, "preview-slide-2");
    click(&mut rig.cx, "go-live");
    wait_for_panel(&mut rig, "bridge on stage", |p| {
        p[0] == "Signal Hymn · Bridge"
    });
    settle(&mut rig.cx, &operator, "bridge on both", |o| {
        o.on_screen() == Some("Signal Hymn · Bridge") && stage_shows(o, "Signal Hymn · Bridge")
    });
    // The face that reached the stage child is the one the audience child
    // got for the same slide (review round 2: compare what was received,
    // not the operator's own state).
    let audience_face = lines(&rig.log, "audience").pop().unwrap()[2].clone();
    let stage_face = last_panel(&rig.log).unwrap()[6].clone();
    assert_eq!(stage_face, audience_face);
    operator.read_with(&rig.cx, |o, _| {
        let live = o.on_air.as_ref().unwrap();
        let face = live.resolved.as_ref().unwrap()[2].face();
        assert_eq!(stage_face, face_field(face.version(), face.index()));
    });
    let sent = lines(&rig.log, "stage").len();

    // The other song's cue fails to prepare: neither output changes.
    let failing = CuePreparer::with_builder(|_, _| -> Result<Built, _> {
        Err(sela::scene::PrepareError::InvalidScene)
    })
    .unwrap();
    operator.update(&mut rig.cx, |o, _| o.cue_preparer = Some(failing));
    click(&mut rig.cx, canticle);
    settle(&mut rig.cx, &operator, "canticle preview", |o| {
        o.preview
            .as_ref()
            .is_some_and(|p| p.title == SECOND_SONG && p.resolved.is_some())
    });
    click(&mut rig.cx, "go-live");
    settle(&mut rig.cx, &operator, "failure reported", |o| {
        o.live_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Slide could not be prepared"))
    });
    for _ in 0..20 {
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    operator.read_with(&rig.cx, |o, _| {
        assert_eq!(o.live.as_ref().unwrap().title, SECOND_SONG, "the request");
        assert_eq!(o.on_screen(), Some("Signal Hymn · Bridge"));
        assert!(stage_shows(o, "Signal Hymn · Bridge"));
    });
    assert_eq!(lines(&rig.log, "stage").len(), sent, "no panel was sent");
    assert_eq!(last_panel(&rig.log).unwrap()[0], "Signal Hymn · Bridge");

    // While the new item's cue is still preparing the stage stays too, and
    // it follows once the cue is handed to the audience.
    let (open, preparer) = gated_preparer(Default::default());
    operator.update(&mut rig.cx, |o, _| o.cue_preparer = Some(preparer));
    click(&mut rig.cx, "go-live");
    for _ in 0..20 {
        operator.update(&mut rig.cx, |o, cx| o.poll(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        lines(&rig.log, "stage").len(),
        sent,
        "nothing while preparing"
    );
    open.send(()).unwrap();
    let panel = wait_for_panel(&mut rig, "canticle on stage", |p| {
        p[0] == format!("{SECOND_SONG} · Verse 1")
    });
    assert_eq!(panel[1], "Quiet original line");
    settle(&mut rig.cx, &operator, "canticle on both", |o| {
        o.on_screen() == Some("Quiet Canticle · Verse 1")
            && stage_shows(o, "Quiet Canticle · Verse 1")
    });
    assert_audience_private(&rig.log);
}

/// Review finding: the stage could be started from the Live menu (or the
/// command) while Live output was off.
#[gpui::test]
fn the_stage_never_starts_without_live_output(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        true,
        Vec::new(),
    );
    let operator = rig.operator.clone();
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage");
    rig.cx.simulate_keystrokes("escape");
    operator.update(&mut rig.cx, |o, _| {
        assert!(o.output.is_none() && o.stage.output.is_none());
        o.toggle_stage_output();
        assert!(o.stage.output.is_none(), "the command refuses too");
        assert_eq!(o.stage.note.as_deref(), Some("Turn on Live output first"));
    });
    assert!(
        rig.monitors.lock().unwrap().is_empty(),
        "nothing was spawned"
    );
}

/// Review finding: Live output turned on before the monitor scan answered
/// skipped the stage for good ("No free monitor").
#[gpui::test]
fn a_late_monitor_list_still_starts_the_stage(cx: &mut TestAppContext) {
    let list = vec![
        monitor("P", true),
        monitor("AUD", false),
        monitor("STG", false),
    ];
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        false,
        list.clone(),
    );
    let operator = rig.operator.clone();
    // As if `sela --list-monitors` had not answered yet.
    operator.update(&mut rig.cx, |o, _| o.settings.monitors = None);
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "audience", connected);
    operator.read_with(&rig.cx, |o, _| {
        assert!(o.stage.output.is_none());
        assert_eq!(o.stage.note.as_deref(), Some("Detecting monitors…"));
    });
    operator.update(&mut rig.cx, |o, _| o.monitors_scanned(list));
    settle(&mut rig.cx, &operator, "stage started", stage_connected);
    assert_eq!(*rig.monitors.lock().unwrap(), ["name:STG".to_string()]);
    // A later scan result never starts a stage the operator stopped.
    click(&mut rig.cx, "bar-live");
    click(&mut rig.cx, "bar-live-stage");
    operator.update(&mut rig.cx, |o, _| {
        assert!(o.stage.output.is_none());
        o.monitors_scanned(vec![
            monitor("P", true),
            monitor("AUD", false),
            monitor("STG", false),
        ]);
        assert!(o.stage.output.is_none());
    });
}

/// Review finding: Live output turned on before the stored settings loaded
/// never started the enabled stage.
#[gpui::test]
fn settings_that_load_after_live_output_still_start_the_stage(cx: &mut TestAppContext) {
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let gate = std::sync::Mutex::new(gate);
    let loader: settings::Loader = Arc::new(move |path| {
        let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(20));
        settings::load(path)
    });
    let mut rig = rig_with(
        cx,
        library_with_song,
        ("apply", vec!["apply"]),
        ENABLED,
        true,
        Vec::new(),
        Some(loader),
    );
    let operator = rig.operator.clone();
    assert!(!operator.read_with(&rig.cx, |o, _| o.settings_loaded_for_options()));
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "audience", connected);
    assert!(operator.read_with(&rig.cx, |o, _| o.stage.output.is_none()));
    release.send(()).unwrap();
    settle(
        &mut rig.cx,
        &operator,
        "stage after the load",
        stage_connected,
    );
}

/// Review finding: the Options paths that move an output had no test. A new
/// Foldback monitor restarts only the stage; a new audience monitor
/// restarts both, and Automatic keeps the stage off the new audience monitor.
#[gpui::test]
fn options_monitor_changes_restart_the_right_outputs(cx: &mut TestAppContext) {
    let mut rig = rig(
        cx,
        library_with_song,
        "apply",
        vec!["apply"],
        ENABLED,
        false,
        vec![
            monitor("P", true),
            monitor("AUD", false),
            monitor("STG", false),
            monitor("STG2", false),
        ],
    );
    let operator = rig.operator.clone();
    settle(&mut rig.cx, &operator, "monitors listed", |o| {
        o.settings.monitors.is_some()
    });
    click(&mut rig.cx, "live-output");
    settle(&mut rig.cx, &operator, "both outputs", |o| {
        connected(o) && stage_connected(o)
    });
    click(&mut rig.cx, "song-0");
    settle(&mut rig.cx, &operator, "preview", |o| o.preview.is_some());
    click(&mut rig.cx, "go-live");
    settle(&mut rig.cx, &operator, "verse on both", |o| {
        o.on_screen() == Some("Signal Hymn · Verse 1") && stage_shows(o, "Signal Hymn · Verse 1")
    });
    let (audience_epoch, first_stage, sent) = operator.read_with(&rig.cx, |o, _| {
        (
            o.output.as_ref().unwrap().epoch(),
            stage_epoch(o),
            submitted(o),
        )
    });

    // Foldback › STG2 (rows: Automatic, P, AUD, STG, STG2).
    rig.cx.simulate_keystrokes("alt-e enter");
    click(&mut rig.cx, "options-section-foldback");
    click(&mut rig.cx, "options-stage-monitor-4");
    click(&mut rig.cx, "dialog-confirm");
    settle(&mut rig.cx, &operator, "stage restarted", |o| {
        stage_connected(o) && stage_epoch(o) != first_stage
    });
    wait_for_panel(&mut rig, "restarted stage panel", |p| {
        p[0] == "Signal Hymn · Verse 1"
    });
    operator.read_with(&rig.cx, |o, _| {
        assert_eq!(o.output.as_ref().unwrap().epoch(), audience_epoch);
        assert_eq!(submitted(o), sent, "the audience got nothing");
        assert!(connected(o));
    });
    assert_eq!(
        *rig.monitors.lock().unwrap(),
        ["name:STG".to_string(), "name:STG2".to_string()]
    );

    // Back to Automatic, then Main Output › STG: both outputs restart and the
    // stage moves to the free AUD (rows: Default, P, AUD, STG, STG2).
    rig.cx.simulate_keystrokes("alt-e enter");
    click(&mut rig.cx, "options-section-foldback");
    click(&mut rig.cx, "options-stage-monitor-0");
    click(&mut rig.cx, "dialog-confirm");
    settle(
        &mut rig.cx,
        &operator,
        "stage back on Automatic",
        stage_connected,
    );
    let before = operator.read_with(&rig.cx, |o, _| stage_epoch(o));
    rig.cx.simulate_keystrokes("alt-e enter");
    click(&mut rig.cx, "options-monitor-3");
    click(&mut rig.cx, "dialog-confirm");
    settle(&mut rig.cx, &operator, "both restarted", |o| {
        connected(o)
            && stage_connected(o)
            && o.output.as_ref().unwrap().epoch() != audience_epoch
            && stage_epoch(o) != before
    });
    let spawned = rig.monitors.lock().unwrap().clone();
    assert_eq!(spawned.last().unwrap(), "name:AUD", "{spawned:?}");
}
