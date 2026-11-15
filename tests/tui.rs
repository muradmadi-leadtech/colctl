//! Cycle 8: the TUI. The model is driven with keys and mouse events, drawn into ratatui's
//! TestBackend with motion off, and compared as text. Named after AC8-NN / EC8-NN.

mod support;

use colctl::core::run::{Job, Report};
use colctl::ctx::Ctx;
use colctl::error::CliError;
use colctl::tui::app::{self, Cmd, JobEvent, Load, Model, Msg, Options, Overlay, Tab, ask, update};
use colctl::tui::worker;
use colctl::tui::{launch, view};
use colctl::ui::term::{ColorChoice, Facts, Settings};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::style::Color;
use std::ffi::OsString;
use std::fs;
use support::Env;

const PLAIN: Options = Options {
    color: false,
    motion: false,
};

fn model(w: u16, h: u16) -> Model {
    Model::new(PLAIN, w, h)
}

fn draw(m: &Model) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(m.width, m.height)).unwrap();
    t.draw(|f| view::draw(m, f)).unwrap();
    t
}

fn screen(m: &Model) -> String {
    view::text(draw(m).backend().buffer())
}

fn press(m: &mut Model, code: KeyCode) -> Vec<Cmd> {
    update(m, Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

fn ch(m: &mut Model, c: char) -> Vec<Cmd> {
    press(m, KeyCode::Char(c))
}

fn click(m: &mut Model, x: u16, y: u16) -> Vec<Cmd> {
    update(
        m,
        Msg::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }),
    )
}

/// Where `text` first appears on screen: (x, y).
fn find(m: &Model, text: &str) -> (u16, u16) {
    for (y, line) in screen(m).lines().enumerate() {
        if let Some(i) = line.find(text) {
            let x = colctl::ui::render::width(&line[..i]);
            return (x as u16, y as u16);
        }
    }
    panic!("{text:?} not on screen:\n{}", screen(m));
}

fn tty() -> Facts {
    Facts {
        stdin_tty: true,
        stdout_tty: true,
        stderr_tty: true,
        term: Some("xterm-256color".into()),
        ..Facts::default()
    }
}

// ── Launch (AC8-06, AC8-07, AC8-12, EC8-20, EC8-23 – 25) ────────────────────────────────────

#[test]
fn ac8_06_ec8_23_opens_only_in_an_interactive_terminal() {
    let s = Settings::default();
    assert!(launch(&s, &tty(), None).is_some());
    for f in [
        Facts {
            stdout_tty: false,
            ..tty()
        },
        Facts {
            stdin_tty: false,
            ..tty()
        },
        Facts {
            term: Some("dumb".into()),
            ..tty()
        },
        Facts {
            ci: Some("true".into()),
            ..tty()
        },
    ] {
        assert!(launch(&s, &f, None).is_none(), "{f:?}");
    }
    // EC8-23: TERM unset still opens.
    assert!(
        launch(
            &s,
            &Facts {
                term: None,
                ..tty()
            },
            None
        )
        .is_some()
    );
}

#[test]
fn ac8_06_piped_colctl_is_the_welcome_help() {
    let out = assert_cmd::Command::new(env!("CARGO_BIN_EXE_colctl"))
        .env_clear()
        .env("HOME", std::env::temp_dir())
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Start here"), "{text}");
}

#[test]
fn ac8_12_ec8_24_ec8_25_colour_follows_the_flags() {
    let color = |s: Settings, f: Facts| launch(&s, &f, None).unwrap().color;
    assert!(color(Settings::default(), tty()));
    assert!(!color(
        Settings::default(),
        Facts {
            no_color: Some("1".into()),
            ..tty()
        }
    ));
    let never = Settings {
        color: ColorChoice::Never,
        ..Settings::default()
    };
    assert!(!color(never, tty()));
    let always = Settings {
        color: ColorChoice::Always,
        ..Settings::default()
    };
    assert!(color(
        always,
        Facts {
            no_color: Some("1".into()),
            ..tty()
        }
    ));
    // EC8-25: -q and -v don't stop it opening.
    let quiet = Settings {
        quiet: true,
        verbose: true,
        ..Settings::default()
    };
    assert!(launch(&quiet, &tty(), None).is_some());
}

#[test]
fn ac8_12_ec8_20_reduced_motion() {
    let s = Settings::default();
    let motion = |v: Option<&str>| launch(&s, &tty(), v).unwrap().motion;
    assert!(motion(None));
    assert!(motion(Some("")));
    assert!(motion(Some("0")));
    assert!(!motion(Some("1")));
    assert!(!motion(Some("yes")));
}

#[test]
fn ac8_07_ec8_22_restoring_twice_is_harmless() {
    colctl::tui::terminal::restore();
    colctl::tui::terminal::restore();
}

#[test]
fn ac8_24_ticks_and_mouse_moves_never_hide_a_redraw() {
    let mut m = model(80, 24);
    assert!(m.dirty, "the first frame must be drawn");
    update(&mut m, Msg::Tick(10));
    assert!(m.dirty);
    m.dirty = false;
    update(&mut m, Msg::Tick(20));
    assert!(!m.dirty, "idle ticks draw nothing");
    update(
        &mut m,
        Msg::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 3,
            row: 3,
            modifiers: KeyModifiers::NONE,
        }),
    );
    assert!(!m.dirty, "mouse moves draw nothing");
    ch(&mut m, '2');
    assert!(m.dirty);
}

#[test]
fn ac8_08_ac8_38_the_status_line_names_what_loads() {
    let mut m = model(80, 24);
    m.start();
    assert_eq!(m.pending.len(), 3);
    assert!(screen(&m).contains("… colctl status"), "{}", screen(&m));
    update(
        &mut m,
        Msg::Loaded(app::Loaded::Snapshot(Box::new(Err(CliError::usage("x"))))),
    );
    assert!(screen(&m).contains("… colctl review"), "{}", screen(&m));
    update(&mut m, Msg::Loaded(app::Loaded::Held(Ok(Vec::new()))));
    assert!(screen(&m).contains("… colctl search"), "{}", screen(&m));
    update(
        &mut m,
        Msg::Loaded(app::Loaded::Catalog(Err(CliError::usage("x")))),
    );
    assert!(m.pending.is_empty());
    assert!(!screen(&m).contains("colctl s"), "{}", screen(&m));
    // A reload counts too.
    ch(&mut m, 'r');
    assert_eq!(m.pending.len(), 3);
}

#[test]
fn ac8_25_a_confirmation_is_as_tall_as_what_it_says() {
    let mut m = model(80, 24);
    ask(
        &mut m,
        "Sync",
        vec!["one".into(), "two".into()],
        Job::Sync(None),
    );
    let s = screen(&m);
    let top = s.lines().position(|l| l.contains("╭ Sync")).unwrap();
    let bottom = s
        .lines()
        .position(|l| l.contains("y yes · n no ─"))
        .unwrap();
    assert_eq!(bottom - top, 3, "{s}");
}

#[test]
fn ac8_08_local_state_loads_first() {
    let mut m = model(80, 24);
    assert_eq!(
        m.start(),
        [
            Cmd::Load(Load::Snapshot),
            Cmd::Load(Load::Held),
            Cmd::Load(Load::Catalog)
        ]
    );
}

// ── The shell (AC8-09 – 12) ─────────────────────────────────────────────────────────────────

#[test]
fn ac8_09_tabs_by_key() {
    let mut m = model(80, 24);
    assert_eq!(m.tab, Tab::Overview);
    press(&mut m, KeyCode::Tab);
    assert_eq!(m.tab, Tab::Profiles);
    press(&mut m, KeyCode::BackTab);
    press(&mut m, KeyCode::BackTab);
    assert_eq!(m.tab, Tab::Discover);
    // In Discover digits search; from anywhere else they go to a tab.
    press(&mut m, KeyCode::Tab);
    for (c, t) in [
        ('1', Tab::Overview),
        ('2', Tab::Profiles),
        ('3', Tab::Items),
        ('4', Tab::Review),
        ('5', Tab::Discover),
    ] {
        ch(&mut m, c);
        assert_eq!(m.tab, t);
    }
}

#[test]
fn ac8_09_the_tab_bar_and_key_bar() {
    let mut m = model(80, 24);
    let s = screen(&m);
    let first = s.lines().next().unwrap();
    assert!(
        first.starts_with(" Overview  Profiles  Items  Review  Discover"),
        "{s}"
    );
    assert!(first.ends_with(&format!("colctl {}", env!("CARGO_PKG_VERSION"))));
    let keys = s.lines().last().unwrap();
    assert!(keys.contains("s sync"), "{s}");
    assert!(keys.ends_with("? keys · q quit"), "{s}");
    ch(&mut m, '4');
    let keys = screen(&m).lines().last().unwrap().to_string();
    assert!(
        keys.starts_with(" a approve · x reject · u check updates"),
        "{keys}"
    );
}

#[test]
fn ac8_09_help_lists_every_key() {
    let mut m = model(100, 40);
    ch(&mut m, '?');
    assert_eq!(m.overlay, Some(Overlay::Help));
    let s = screen(&m);
    for needle in ["Keys", "sync", "reload", "next tab", "quit", "ctrl-c", "1"] {
        assert!(s.contains(needle), "{needle}:\n{s}");
    }
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.overlay, None);
    // At 80×24 it fits in two columns, nothing cut.
    let mut m = model(80, 24);
    ch(&mut m, '?');
    let s = screen(&m);
    for needle in ["go to a tab", "quit", "ctrl-c", "reload"] {
        assert!(s.contains(needle), "{needle}:\n{s}");
    }
}

#[test]
fn ac8_09_quit() {
    let mut m = model(80, 24);
    assert_eq!(ch(&mut m, 'q'), [Cmd::Quit]);
    assert!(m.quit);
    let mut m = model(80, 24);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(update(&mut m, Msg::Key(ctrl_c)), [Cmd::Quit]);
}

#[test]
fn ac8_10_mouse_on_tabs_and_keys() {
    let mut m = model(80, 24);
    let (x, y) = find(&m, "Items");
    click(&mut m, x + 1, y);
    assert_eq!(m.tab, Tab::Items);
    let (x, y) = find(&m, "? keys");
    click(&mut m, x, y);
    assert_eq!(m.overlay, Some(Overlay::Help));
    // A click outside an overlay closes it.
    click(&mut m, 0, 5);
    assert_eq!(m.overlay, None);
    let (x, y) = find(&m, "q quit");
    assert_eq!(click(&mut m, x, y), [Cmd::Quit]);
}

#[test]
fn ac8_10_the_wheel_scrolls_a_detail() {
    let mut m = model(80, 24);
    m.overlay = Some(Overlay::Detail {
        title: "x".into(),
        lines: (0..100).map(|i| format!("line {i}")).collect(),
        scroll: 0,
        held: None,
    });
    let wheel = |m: &mut Model, kind| {
        update(
            m,
            Msg::Mouse(MouseEvent {
                kind,
                column: 10,
                row: 10,
                modifiers: KeyModifiers::NONE,
            }),
        )
    };
    wheel(&mut m, MouseEventKind::ScrollDown);
    wheel(&mut m, MouseEventKind::ScrollDown);
    let scroll = |m: &Model| match &m.overlay {
        Some(Overlay::Detail { scroll, .. }) => *scroll,
        _ => usize::MAX,
    };
    assert_eq!(scroll(&m), 2);
    wheel(&mut m, MouseEventKind::ScrollUp);
    assert_eq!(scroll(&m), 1);
    press(&mut m, KeyCode::Char('G'));
    assert_eq!(scroll(&m), 99);
    assert!(screen(&m).contains("line 99"));
}

#[test]
fn ac8_11_too_small() {
    for (w, h) in [(59, 24), (80, 19)] {
        let m = model(w, h);
        let s = screen(&m);
        assert!(
            s.contains(&format!("Make the window at least 60×20 (now {w}×{h})")),
            "{s}"
        );
        assert!(!s.contains("Overview"), "{s}");
    }
    let mut m = model(59, 24);
    ch(&mut m, '3');
    update(&mut m, Msg::Resize(60, 20));
    assert_eq!(m.tab, Tab::Items);
    assert!(screen(&m).contains("Overview"));
}

#[test]
fn ac8_12_no_colour_means_no_colour() {
    let m = model(80, 24);
    let t = draw(&m);
    let buf = t.backend().buffer();
    assert!(
        buf.content()
            .iter()
            .all(|c| c.fg == Color::Reset && c.bg == Color::Reset),
        "colour without colour"
    );
    let m = Model::new(
        Options {
            color: true,
            motion: false,
        },
        80,
        24,
    );
    let t = draw(&m);
    let (x, y) = find(&m, "Overview");
    assert_eq!(t.backend().buffer()[(x, y)].fg, Color::Cyan);
}

// ── Jobs, confirmations, errors (AC8-25 – 28, EC8-19) ───────────────────────────────────────

fn report(verdict: &str) -> Report {
    Report {
        exit: 0,
        lines: vec![verdict.into()],
        errors: Vec::new(),
    }
}

#[test]
fn ac8_25_writes_ask_first() {
    let mut m = model(80, 24);
    assert!(
        ask(
            &mut m,
            "Sync",
            vec!["Every profile".into()],
            Job::Sync(None)
        )
        .is_empty()
    );
    let s = screen(&m);
    assert!(s.contains("Sync") && s.contains("y yes · n no"), "{s}");
    // ⏎ alone does nothing; n changes nothing.
    assert!(press(&mut m, KeyCode::Enter).is_empty());
    assert!(m.overlay.is_some());
    ch(&mut m, 'n');
    assert_eq!(m.overlay, None);
    assert!(m.job.is_none());
    // y runs it.
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    assert_eq!(ch(&mut m, 'y'), [Cmd::Run(Job::Sync(None))]);
    assert!(m.job.is_some());
}

#[test]
fn ac8_26_one_job_at_a_time() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    assert!(ask(&mut m, "Update", vec![], Job::Update(vec![])).is_empty());
    assert_eq!(m.overlay, None);
    assert_eq!(
        m.toast.as_ref().unwrap().text,
        "Wait for colctl sync to finish"
    );
    // Reading still works.
    ch(&mut m, '2');
    assert_eq!(m.tab, Tab::Profiles);
}

#[test]
fn ac8_15_progress_rows_then_the_verdict() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    let s = screen(&m);
    assert!(s.contains("… colctl sync"), "{s}");
    for e in [
        JobEvent::Step {
            profile: "claude".into(),
            action: "linking alpha".into(),
            n: 1,
            total: 2,
        },
        JobEvent::Step {
            profile: "codex".into(),
            action: "installing x".into(),
            n: 1,
            total: 1,
        },
        JobEvent::Done {
            profile: "codex".into(),
            summary: "1 change".into(),
            ok: true,
        },
    ] {
        update(&mut m, Msg::Job(e));
    }
    let s = screen(&m);
    assert!(s.contains("… claude  linking alpha  1/2"), "{s}");
    assert!(s.contains("✓ codex   1 change"), "{s}");
    let cmds = update(
        &mut m,
        Msg::Job(JobEvent::Finished(Ok(report("✓ Synced 2 profiles")))),
    );
    assert!(m.job.is_none());
    assert!(cmds.contains(&Cmd::Load(Load::Snapshot)));
    assert!(screen(&m).contains("✓ Synced 2 profiles"));
    // The toast goes after 4 s.
    let later = m.now + app::TOAST_MS;
    update(&mut m, Msg::Tick(later));
    assert!(m.toast.is_none());
}

#[test]
fn ac8_15_a_failed_run_stays_on_screen_until_a_key() {
    let mut m = model(80, 24);
    ask(&mut m, "Approve", vec![], Job::Approve(vec!["q".into()]));
    ch(&mut m, 'y');
    let failed = Report {
        exit: 1,
        lines: vec![
            "✓ Approved 1 held version".into(),
            "✗ Synced 3 of 5 profiles · 2 failed".into(),
        ],
        errors: Vec::new(),
    };
    update(&mut m, Msg::Job(JobEvent::Finished(Ok(failed))));
    let toast = m.toast.clone().unwrap();
    assert!(toast.text.starts_with("✗ Synced 3 of 5"), "{toast:?}");
    assert_eq!(toast.until, None, "a failure stays");
    update(&mut m, Msg::Tick(60_000));
    assert!(m.toast.is_some());
    ch(&mut m, '2');
    assert!(m.toast.is_none());
}

#[test]
fn ac8_27_errors_show_what_why_and_what_to_do() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    let err = CliError::refused("couldn't sync\nclaude failed").hint("run colctl status");
    update(&mut m, Msg::Job(JobEvent::Finished(Err(err))));
    let s = screen(&m);
    assert!(s.contains("error: couldn't sync"), "{s}");
    assert!(s.contains("claude failed"), "{s}");
    assert!(s.contains("hint: run colctl status"), "{s}");
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.overlay, None);
}

#[test]
fn ac8_28_quitting_waits_for_a_write() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    assert!(ch(&mut m, 'q').is_empty());
    assert_eq!(m.overlay, Some(Overlay::ConfirmQuit));
    assert!(screen(&m).contains("colctl sync is running. Quit when it finishes?"));
    ch(&mut m, 'n');
    assert!(!m.quit && !m.quit_after_job);
    ch(&mut m, 'q');
    ch(&mut m, 'y');
    assert!(m.quit_after_job && !m.quit);
    let cmds = update(&mut m, Msg::Job(JobEvent::Finished(Ok(report("✓ done")))));
    assert_eq!(cmds, [Cmd::Quit]);
    assert!(m.quit);
}

#[test]
fn ec8_19_esc_stops_an_update_but_not_a_sync() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    assert!(press(&mut m, KeyCode::Esc).is_empty());
    assert_eq!(
        m.toast.as_ref().unwrap().text,
        "Sync can't be stopped half way"
    );
    let mut m = model(80, 24);
    ask(&mut m, "Update", vec![], Job::Update(vec![]));
    ch(&mut m, 'y');
    assert_eq!(press(&mut m, KeyCode::Esc), [Cmd::Cancel]);
}

#[test]
fn ec8_08_resize_keeps_the_job_and_overlay() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    update(&mut m, Msg::Resize(120, 40));
    assert!(matches!(m.overlay, Some(Overlay::Confirm { .. })));
    ch(&mut m, 'y');
    update(&mut m, Msg::Resize(70, 22));
    assert!(m.job.is_some());
    assert!(screen(&m).contains("colctl sync"));
}

#[test]
fn ac8_12_the_spinner_turns_only_with_motion() {
    let mut m = Model::new(
        Options {
            color: false,
            motion: true,
        },
        80,
        24,
    );
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    update(&mut m, Msg::Tick(80));
    assert!(screen(&m).contains("⠙ colctl sync"), "{}", screen(&m));
}

#[test]
fn ec8_17_a_paste_into_a_filter_is_one_change() {
    let mut m = model(80, 24);
    ch(&mut m, '3');
    ch(&mut m, '/');
    update(&mut m, Msg::Paste("tools\n".into()));
    assert_eq!(m.filter().text, "tools");
    press(&mut m, KeyCode::Backspace);
    assert_eq!(m.filter().text, "tool");
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.filter().text, "");
    assert!(!m.filter().focused);
    // Outside a filter, a paste does nothing.
    update(&mut m, Msg::Paste("x".into()));
    assert_eq!(m.filter().text, "");
}

#[test]
fn cursor_stays_in_range_and_on_screen() {
    let mut c = app::Cursor::default();
    c.move_by(5, 3, 10);
    assert_eq!(c.selected, 2);
    c.move_by(-10, 3, 10);
    assert_eq!(c.selected, 0);
    c.move_by(50, 100, 10);
    assert_eq!((c.selected, c.offset), (50, 41));
    c.move_by(-45, 100, 10);
    assert_eq!((c.selected, c.offset), (5, 5));
    c.fit(0, 10);
    assert_eq!(c, app::Cursor::default());
    c.move_by(1, 0, 10);
    assert_eq!(c, app::Cursor::default());
}

// ── Fixtures driven end to end ─────────────────────────────────────────────────────────────

fn ctx(env: &Env) -> Ctx {
    Ctx::from_vars(&[
        ("HOME".into(), OsString::from(&env.home)),
        ("PATH".into(), env.path.clone()),
    ])
    .unwrap()
}

/// Runs `cmds` (and whatever they lead to) to the end, inline.
fn drive(env: &Env, m: &mut Model, cmds: Vec<Cmd>) {
    let c = ctx(env);
    let mut queue = cmds;
    while !queue.is_empty() {
        let next: Vec<Cmd> = std::mem::take(&mut queue);
        for cmd in next {
            for msg in worker::run_now(&c, &cmd, view::detail_width(m)) {
                queue.extend(update(m, msg));
            }
        }
    }
}

/// A model with everything loaded from `env`.
fn opened(env: &Env, w: u16, h: u16) -> Model {
    let mut m = model(w, h);
    let start = m.start();
    drive(env, &mut m, start);
    m
}

/// Commit ids and colctl's version masked, for snapshots.
fn mask(text: &str) -> String {
    let text = &text.replace(
        &format!("colctl {}", env!("CARGO_PKG_VERSION")),
        "colctl [VERSION]",
    );
    // Repos in temp folders.
    let mut text = text.clone();
    while let Some(at) = text.find("file://") {
        let end = text[at..].find(['\n', '│']).map_or(text.len(), |e| at + e);
        text.replace_range(at..end, "[REPO]");
    }
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let run = chars[i..]
            .iter()
            .take_while(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            .count();
        let boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        let end = chars
            .get(i + run)
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        // A commit id cut short ("cc2…").
        if boundary && (1..7).contains(&run) && chars.get(i + run) == Some(&'…') {
            out.push_str("[CUT]");
            i += run;
            continue;
        }
        if boundary
            && end
            && (run == 7 || run == 40)
            && chars[i..i + run].iter().any(|c| c.is_ascii_digit())
        {
            out.push_str(if run == 7 { "[SHORT]" } else { "[SHA]" });
            i += run;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Three profiles, a source plugin with an update held, and a skill missing from one profile.
fn held_fixture() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.ok(&["init"]);
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    env.source_skill("alpha");
    env.ok(&["sync"]);
    fs::remove_file(env.home.join(".claude/skills/alpha")).unwrap();
    env
}

fn in_sync_fixture() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.ok(&["init"]);
    env.source_skill("alpha");
    env.ok(&["sync"]);
    env
}

fn row_of(m: &Model, text: &str) -> usize {
    let rows = view::rows(m);
    rows.iter()
        .position(|l| l.to_string().contains(text))
        .unwrap_or_else(|| panic!("no row with {text:?}"))
}

// ── Overview and Profiles (AC8-13 – 16) ─────────────────────────────────────────────────────

#[test]
fn ac8_13_overview_leads_with_the_status_verdict() {
    let env = held_fixture();
    let m = opened(&env, 100, 30);
    let (_, out, _) = env.code(&["status", "--color", "never"]);
    let cli = out.lines().next().unwrap();
    let s = screen(&m);
    assert_eq!(s.lines().nth(2).unwrap().trim(), cli, "{s}");
    assert!(s.contains("Profiles") && s.contains("Needs you"), "{s}");
    assert!(s.contains("claude") && s.contains("1 missing"), "{s}");
    assert!(s.contains("→ colctl review superpowers"), "{s}");
    assert!(s.lines().next().unwrap().contains("Overview !"), "{s}");
    assert!(s.lines().next().unwrap().contains("Review !1"), "{s}");
}

#[test]
fn ac8_13_ec8_09_snapshots_of_overview() {
    let env = held_fixture();
    for (w, h) in [(80, 24), (120, 40), (60, 20)] {
        let m = opened(&env, w, h);
        insta::assert_snapshot!(format!("overview_held_{w}x{h}"), mask(&screen(&m)));
    }
    let env = in_sync_fixture();
    let m = opened(&env, 80, 24);
    let s = screen(&m);
    assert!(!s.contains("Needs you"), "{s}");
    insta::assert_snapshot!("overview_in_sync_80x24", mask(&s));
}

#[test]
fn ac8_14_a_review_fix_opens_the_review() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    m.cursor_mut().selected = row_of(&m, "colctl review superpowers");
    let cmds = press(&mut m, KeyCode::Enter);
    assert_eq!(m.tab, Tab::Review);
    assert_eq!(cmds, [Cmd::Load(Load::Review("superpowers".into()))]);
    drive(&env, &mut m, cmds);
    assert!(matches!(&m.overlay, Some(Overlay::Detail { title, .. }) if title == "superpowers"));
}

#[test]
fn ac8_14_a_writing_fix_asks_first() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    m.cursor_mut().selected = row_of(&m, "→ colctl sync");
    assert!(press(&mut m, KeyCode::Enter).is_empty());
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm {
            job: Job::Sync(None),
            ..
        })
    ));
}

#[test]
fn ac8_15_sync_from_overview_ends_in_sync() {
    let env = held_fixture();
    // Leave only the missing skill to fix.
    env.ok(&["reject", "superpowers"]);
    let mut m = opened(&env, 100, 30);
    ch(&mut m, 's');
    let s = screen(&m);
    assert!(s.contains("Make every profile match ~/.colctl."), "{s}");
    assert!(s.contains("Profiles: claude, kilo-claude, codex"), "{s}");
    let cmds = ch(&mut m, 'y');
    assert_eq!(cmds, [Cmd::Run(Job::Sync(None))]);
    // Progress arrives while it runs.
    let c = ctx(&env);
    let msgs = worker::run_now(&c, &cmds[0], 96);
    let mut after = Vec::new();
    for msg in msgs {
        let finished = matches!(msg, Msg::Job(JobEvent::Finished(_)));
        if finished {
            let s = screen(&m);
            assert!(s.contains("✓ claude"), "{s}");
        }
        after.extend(update(&mut m, msg));
    }
    drive(&env, &mut m, after);
    let s = screen(&m);
    assert!(s.contains("✓ All 3 profiles in sync"), "{s}");
    assert_eq!(env.code(&["status"]).0, 0);
}

#[test]
fn ac8_16_profiles_list_filter_open_and_sync_one() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '2');
    assert_eq!(m.list_len(), 3);
    insta::assert_snapshot!(
        "profiles_80x24",
        mask(&{
            let mut small = opened(&env, 80, 24);
            ch(&mut small, '2');
            screen(&small)
        })
    );
    ch(&mut m, '/');
    for c in "kilo".chars() {
        ch(&mut m, c);
    }
    assert_eq!(m.list_len(), 1);
    assert!(screen(&m).contains("› kilo▏"));
    press(&mut m, KeyCode::Enter);
    assert!(!m.filter().focused);
    press(&mut m, KeyCode::Enter);
    let s = screen(&m);
    assert!(
        matches!(&m.overlay, Some(Overlay::Detail { title, .. }) if title == "kilo-claude"),
        "{s}"
    );
    assert!(s.contains("skill") && s.contains("alpha"), "{s}");
    press(&mut m, KeyCode::Esc);
    ch(&mut m, 's');
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm { job: Job::Sync(Some(p)), .. }) if p == "kilo-claude"
    ));
    ch(&mut m, 'n');
    // Esc clears the filter.
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.list_len(), 3);
    ch(&mut m, '/');
    ch(&mut m, 'z');
    assert!(screen(&m).contains("No profiles match \"z\""));
}

#[test]
fn ec8_01_not_set_up() {
    let env = Env::new();
    env.native();
    let m = opened(&env, 80, 24);
    let s = screen(&m);
    assert!(s.contains("colctl isn't set up yet"), "{s}");
    assert!(s.contains("Quit and run colctl init."), "{s}");
}

#[test]
fn ec8_02_a_broken_config_says_why_and_r_retries() {
    let env = in_sync_fixture();
    fs::write(env.source.join("config.toml"), "this is = = not toml").unwrap();
    let mut m = opened(&env, 100, 24);
    let s = screen(&m);
    assert!(s.contains("✗ "), "{s}");
    assert!(s.contains("r tries again"), "{s}");
    let cmds = ch(&mut m, 'r');
    assert!(cmds.contains(&Cmd::Load(Load::Snapshot)));
}

#[test]
fn ec8_03_no_profiles() {
    let env = Env::new();
    env.ok(&["init"]);
    let m = opened(&env, 80, 24);
    let s = screen(&m);
    assert!(s.contains("! No profiles registered"), "{s}");
    assert!(s.contains("colctl init"), "{s}");
}

#[test]
fn ec8_21_control_characters_never_reach_the_terminal() {
    let env = in_sync_fixture();
    env.source_skill("evil\u{1b}[31mred");
    let mut m = opened(&env, 100, 30);
    let s = screen(&m);
    assert!(!s.contains('\u{1b}'), "{s:?}");
    ch(&mut m, '2');
    press(&mut m, KeyCode::Enter);
    assert!(!screen(&m).contains('\u{1b}'));
}

// ── Items (AC8-17) ──────────────────────────────────────────────────────────────────────────

#[test]
fn ac8_17_every_item_with_its_state_in_each_profile() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '3');
    let s = screen(&m);
    assert!(s.contains("1 claude · 2 kilo-claude · 3 codex"), "{s}");
    assert!(s.contains("1 needs you"), "{s}");
    let alpha = view::rows(&m)[row_of(&m, "alpha")].to_string();
    // Missing in claude, fine in kilo-claude, not in codex.
    assert!(alpha.ends_with("! ✓ – "), "{alpha:?}");
    insta::assert_snapshot!(
        "items_80x24",
        mask(&{
            let mut small = opened(&env, 80, 24);
            ch(&mut small, '3');
            screen(&small)
        })
    );
}

#[test]
fn ac8_17_problems_first_within_a_kind() {
    let env = held_fixture();
    env.source_skill("aaa");
    env.ok(&["sync"]);
    fs::remove_file(env.home.join(".claude/skills/alpha")).unwrap();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '3');
    assert!(row_of(&m, "alpha") < row_of(&m, "aaa"));
}

#[test]
fn ac8_17_filter_and_open_an_item() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '3');
    let all = m.list_len();
    ch(&mut m, '/');
    for c in "alp".chars() {
        ch(&mut m, c);
    }
    assert_eq!(m.list_len(), 1, "{}", screen(&m));
    assert!(m.list_len() < all);
    press(&mut m, KeyCode::Enter);
    press(&mut m, KeyCode::Enter);
    let s = screen(&m);
    assert!(
        matches!(&m.overlay, Some(Overlay::Detail { title, .. }) if title == "skill alpha"),
        "{s}"
    );
    assert!(s.contains("missing  skills/alpha  → colctl sync"), "{s}");
    assert!(s.contains("· Not in codex"), "{s}");
    press(&mut m, KeyCode::Esc);
    ch(&mut m, '/');
    ch(&mut m, 'z');
    assert!(screen(&m).contains("No items match \"alpz\""));
}

// ── Review (AC8-18 – 21) ────────────────────────────────────────────────────────────────────

#[test]
fn ac8_18_the_queue() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    let s = screen(&m);
    assert!(s.contains("! 1 held for review"), "{s}");
    let row = view::rows(&m)[0].to_string();
    assert!(
        row.contains("superpowers") && row.contains("source"),
        "{row}"
    );
    assert!(row.contains(" → ") && row.contains("1 commit"), "{row}");
    insta::assert_snapshot!(
        "review_80x24",
        mask(&{
            let mut small = opened(&env, 80, 24);
            ch(&mut small, '4');
            screen(&small)
        })
    );
    let env = in_sync_fixture();
    let mut m = opened(&env, 80, 24);
    ch(&mut m, '4');
    let s = screen(&m);
    assert!(
        s.contains("Nothing held for review. u checks for updates"),
        "{s}"
    );
    assert!(s.lines().next().unwrap().contains(" Review "), "{s}");
}

#[test]
fn ac8_19_ac8_36_a_held_version_at_a_glance_then_in_full() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    // ⏎ opens the summary at once; d asks for the full review.
    assert!(press(&mut m, KeyCode::Enter).is_empty());
    let s = screen(&m);
    assert!(s.contains("✓ Recommendation: approve"), "{s}");
    assert!(s.contains("d full review"), "{s}");
    insta::assert_snapshot!("review_summary_100x30", mask(&s));
    let cmds = ch(&mut m, 'd');
    assert_eq!(cmds, [Cmd::Load(Load::Review("superpowers".into()))]);
    drive(&env, &mut m, cmds);
    assert!(!screen(&m).contains("d full review"));
    let Some(Overlay::Detail { lines, held, .. }) = &m.overlay else {
        panic!("{:?}", m.overlay);
    };
    assert_eq!(held.as_deref(), Some("superpowers"));
    // Exactly what colctl review prints, at the overlay's width.
    let (_, out, _) = env.code(&["review", "superpowers", "--color", "never", "--no-pager"]);
    let cli: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], cli[0]);
    assert_eq!(lines.len(), cli.len());
    let first_file = lines
        .iter()
        .position(|l| l.starts_with("diff --git"))
        .expect("a diff");
    ch(&mut m, ']');
    assert!(matches!(&m.overlay, Some(Overlay::Detail { scroll, .. }) if *scroll == first_file));
    ch(&mut m, '[');
    assert!(matches!(&m.overlay, Some(Overlay::Detail { scroll, .. }) if *scroll == first_file));
    press(&mut m, KeyCode::Home);
    let s = screen(&m);
    assert!(s.contains("a approve · x reject · ] next file"), "{s}");
    insta::assert_snapshot!("review_detail_100x30", mask(&s));
}

#[test]
fn ac8_20_approve_from_the_detail() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    let cmds = press(&mut m, KeyCode::Enter);
    drive(&env, &mut m, cmds);
    ch(&mut m, 'a');
    let s = screen(&m);
    assert!(s.contains("Approve superpowers"), "{s}");
    assert!(s.contains("exactly as reviewed"), "{s}");
    let cmds = ch(&mut m, 'y');
    assert_eq!(cmds, [Cmd::Run(Job::Approve(vec!["superpowers".into()]))]);
    drive(&env, &mut m, cmds);
    assert_eq!(m.held_count(), 0);
    assert!(screen(&m).contains("Nothing held for review"));
    assert!(
        env.code(&["review", "--color", "never"])
            .1
            .contains("Nothing held")
    );
}

#[test]
fn ac8_20_reject_from_the_list_and_no_changes_nothing() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'x');
    assert!(screen(&m).contains("Reject superpowers"));
    ch(&mut m, 'n');
    assert_eq!(m.held_count(), 1);
    ch(&mut m, 'x');
    let cmds = ch(&mut m, 'y');
    assert_eq!(cmds, [Cmd::Run(Job::Reject(vec!["superpowers".into()]))]);
    drive(&env, &mut m, cmds);
    assert_eq!(m.held_count(), 0);
}

#[test]
fn ac8_20_approve_all_lists_what_it_installs() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'A');
    let s = screen(&m);
    assert!(
        s.contains("Approve all") && s.contains("superpowers"),
        "{s}"
    );
    let cmds = ch(&mut m, 'y');
    assert_eq!(cmds, [Cmd::Run(Job::Approve(vec![]))]);
    drive(&env, &mut m, cmds);
    assert_eq!(m.held_count(), 0);
}

#[test]
fn ac8_21_check_for_updates() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let (repo, url, _) = env.upstream("superpowers", true, false, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    env.upstream_commit(&repo, "v2");
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'u');
    assert!(screen(&m).contains("Check for updates"));
    let cmds = ch(&mut m, 'y');
    assert_eq!(cmds, [Cmd::Run(Job::Update(vec![]))]);
    drive(&env, &mut m, cmds);
    assert_eq!(m.held_count(), 1);
    assert!(screen(&m).contains("held for review"), "{}", screen(&m));
}

#[test]
fn ec8_18_approved_elsewhere_meanwhile() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '4');
    env.ok(&["reject", "superpowers"]);
    ch(&mut m, 'a');
    let cmds = ch(&mut m, 'y');
    drive(&env, &mut m, cmds);
    let s = screen(&m);
    assert!(s.contains("error:"), "{s}");
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.held_count(), 0);
}

// ── Discover (AC8-22, AC8-23) ───────────────────────────────────────────────────────────────

/// A Claude profile that knows one marketplace with "tool", "toolbox" and "local".
fn catalog_fixture() -> Env {
    let env = Env::new();
    let native = env.native();
    env.ok(&["init"]);
    let market = env.home.parent().unwrap().join("markets/official");
    let write = |path: &std::path::Path, v: serde_json::Value| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, v.to_string()).unwrap();
    };
    write(
        &market.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "official", "plugins": [
            {"name": "tool", "description": "A tool", "author": {"name": "Ann"},
             "source": {"source": "github", "repo": "ann/tool"}},
            {"name": "toolbox", "description": "Many tools", "author": {"name": "Bo"},
             "source": {"source": "github", "repo": "bo/toolbox"}},
            {"name": "local", "description": "A local one", "source": "./local"}]}),
    );
    write(
        &native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"official": {"source": {"source": "github", "repo": "o/official"},
                              "installLocation": market.display().to_string()}}),
    );
    env
}

#[test]
fn ac8_08_discover_says_it_is_loading() {
    let mut m = model(80, 24);
    ch(&mut m, '5');
    assert!(screen(&m).contains("Loading catalogs…"));
}

#[test]
fn ac8_22_ec8_05_search_as_you_type() {
    let env = catalog_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    let s = screen(&m);
    assert!(s.contains("3 of 3 plugins · 1 catalog"), "{s}");
    insta::assert_snapshot!(
        "discover_80x24",
        mask(&{
            let mut small = opened(&env, 80, 24);
            ch(&mut small, '5');
            screen(&small)
        })
    );
    for c in "tl".chars() {
        ch(&mut m, c);
    }
    // Letters in order: tool and toolbox, the closer match first.
    let s = screen(&m);
    assert!(s.contains("2 of 3 plugins"), "{s}");
    assert!(view::rows(&m)[0].to_string().contains("tool "), "{s}");
    // The same order colctl search gives.
    let (_, out, _) = env.code(&["search", "tl", "--color", "never"]);
    let cli: Vec<String> = out
        .lines()
        .filter(|l| l.contains("@official"))
        .map(|l| {
            l.split_whitespace()
                .next()
                .unwrap()
                .split('@')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    let ours: Vec<String> = view::rows(&m)
        .iter()
        .map(|l| l.to_string().split_whitespace().next().unwrap().to_string())
        .collect();
    assert_eq!(ours, cli);
    ch(&mut m, 'z');
    assert!(screen(&m).contains("No plugins match \"tlz\""));
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.list_len(), 3);
}

#[test]
fn ec8_26_spaces_list_everything() {
    let env = catalog_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    ch(&mut m, ' ');
    assert_eq!(m.list_len(), 3);
}

#[test]
fn ac8_23_show_and_add() {
    let env = catalog_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    m.cursor_mut().selected = row_of(&m, "local");
    let cmds = press(&mut m, KeyCode::Enter);
    assert_eq!(cmds, [Cmd::Load(Load::Show("local@official".into()))]);
    drive(&env, &mut m, cmds);
    let Some(Overlay::Detail { lines, .. }) = &m.overlay else {
        panic!("{:?}", m.overlay);
    };
    let (_, out, _) = env.code(&["show", "local@official", "--color", "never", "--no-pager"]);
    assert_eq!(lines[0], out.lines().next().unwrap());
    // No diff, so no file keys.
    assert!(!screen(&m).contains("] next file"), "{}", screen(&m));
    press(&mut m, KeyCode::Esc);
    // "tool" lives in a GitHub repo: adding it is a source add, which would clone; check the ask.
    m.cursor_mut().selected = row_of(&m, "tool ");
    let cmds = press(&mut m, KeyCode::Enter);
    drive(&env, &mut m, cmds);
    ch(&mut m, 'a');
    let s = screen(&m);
    assert!(
        s.contains("Add tool") && s.contains("Runs colctl add source"),
        "{s}"
    );
    ch(&mut m, 'n');
}

#[test]
fn ac8_23_what_cannot_be_added_says_why() {
    let env = catalog_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    if let app::Data::Ready(c) = &mut m.catalog {
        for p in &mut c.plugins {
            p.add = "already source plugin tool".into();
        }
    }
    let cmds = press(&mut m, KeyCode::Enter);
    drive(&env, &mut m, cmds);
    assert!(ch(&mut m, 'a').is_empty());
    assert!(!matches!(m.overlay, Some(Overlay::Confirm { .. })));
    assert!(
        m.toast
            .as_ref()
            .unwrap()
            .text
            .contains("already source plugin tool")
    );
}

#[test]
fn ec8_04_no_catalogs() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    let s = screen(&m);
    assert!(s.contains("! No catalogs found"), "{s}");
}

// ── Motion (AC8-24, AC8-12) ─────────────────────────────────────────────────────────────────

use colctl::tui::motion::{self, Fx};

fn moving(color: bool) -> Model {
    Model::new(
        Options {
            color,
            motion: true,
        },
        80,
        24,
    )
}

/// Draws a frame with `fx` applied at `now`.
fn frame(m: &Model, fx: &mut Fx, now: u64) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(m.width, m.height)).unwrap();
    t.draw(|f| {
        view::draw(m, f);
        fx.apply(now, f.buffer_mut());
    })
    .unwrap();
    t
}

#[test]
fn ac8_24_a_tab_switch_moves_then_stops() {
    let mut m = moving(true);
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    assert!(fx.is_empty(), "the first frame just appears");
    ch(&mut m, '2');
    fx.observe(&m);
    assert_eq!(fx.len(), 1);
    frame(&m, &mut fx, 1000);
    assert!(fx.running(), "a new effect starts at its first frame");
    frame(&m, &mut fx, 1000 + u64::from(motion::TAB_MS) / 2);
    assert!(fx.running());
    let end = frame(&m, &mut fx, 1000 + u64::from(motion::TAB_MS) + 20);
    assert!(!fx.running(), "idle once it ends");
    // The last frame is the plain frame.
    assert_eq!(view::text(end.backend().buffer()), screen(&m));
}

#[test]
fn ac8_24_overlays_finished_rows_and_reloads_move() {
    let mut m = moving(true);
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    ch(&mut m, '?');
    fx.observe(&m);
    assert_eq!(fx.len(), 1, "overlay");
    fx.finish();
    press(&mut m, KeyCode::Esc);
    fx.observe(&m);
    assert!(fx.is_empty(), "closing just closes");
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    fx.observe(&m);
    fx.finish();
    update(
        &mut m,
        Msg::Job(JobEvent::Done {
            profile: "claude".into(),
            summary: "1 change".into(),
            ok: true,
        }),
    );
    fx.observe(&m);
    assert_eq!(fx.len(), 1, "a finished row");
    fx.finish();
    update(
        &mut m,
        Msg::Loaded(app::Loaded::Snapshot(Box::new(Err(CliError::usage("x"))))),
    );
    fx.observe(&m);
    assert_eq!(fx.len(), 1, "a reload");
}

#[test]
fn ac8_12_reduced_motion_moves_nothing() {
    let mut m = model(80, 24);
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    ch(&mut m, '2');
    ch(&mut m, '?');
    fx.observe(&m);
    assert!(fx.is_empty());
    assert!(!fx.running());
}

#[test]
fn ac8_12_motion_without_colour_adds_no_colour() {
    let mut m = moving(false);
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    ch(&mut m, '3');
    fx.observe(&m);
    ch(&mut m, '?');
    fx.observe(&m);
    assert_eq!(fx.len(), 2);
    for t in (0..=300).step_by(30) {
        let term = frame(&m, &mut fx, 5000 + t);
        assert!(
            term.backend()
                .buffer()
                .content()
                .iter()
                .all(|c| c.fg == Color::Reset && c.bg == Color::Reset),
            "colour at {t} ms"
        );
    }
    assert!(!fx.running());
}

#[test]
fn ac8_24_a_key_finishes_effects() {
    let mut m = moving(true);
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    ch(&mut m, '4');
    fx.observe(&m);
    assert!(fx.running());
    fx.finish();
    assert!(!fx.running());
    // Too small: nothing moves.
    let mut m = Model::new(
        Options {
            color: true,
            motion: true,
        },
        50,
        10,
    );
    let mut fx = Fx::new(&m);
    fx.observe(&m);
    ch(&mut m, '2');
    fx.observe(&m);
    assert!(fx.is_empty());
}

// ── Parity (AC8-30) ─────────────────────────────────────────────────────────────────────────

/// Everything colctl and the tools wrote under `env`'s home, with what differs between two
/// identical machines masked: the temp root, commit ids, timestamps.
fn state_of(env: &Env) -> Vec<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path();
            let rel = path.strip_prefix(base).unwrap().display().to_string();
            // Git's own files, colctl's lock and temp files, and its cache (derived data, AC9-11)
            // aren't state.
            if rel.ends_with("/.git")
                || rel == ".colctl/.git"
                || rel.contains(".lock")
                || rel.starts_with(".colctl/.state/cache")
            {
                continue;
            }
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                let to = fs::read_link(&path).unwrap().display().to_string();
                out.push((rel, format!("-> {to}")));
            } else if meta.is_dir() {
                out.push((rel.clone(), "dir".into()));
                walk(&path, base, out);
            } else {
                let text = fs::read(&path)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                out.push((rel, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(&env.home, &env.home, &mut out);
    out.sort();
    let root = env.home.parent().unwrap().display().to_string();
    let mut lines: Vec<String> = out
        .into_iter()
        .map(|(k, v)| {
            let v = v.replace(&root, "[ROOT]");
            // Timestamps: "held_at": 1731600000, "installedAt": "2026-…", lastUpdated…
            let v: String = v
                .split('\n')
                .map(|l| {
                    if ["held_at", "At\"", "_at\"", "lastUpdated", "updated"]
                        .iter()
                        .any(|k| l.contains(k))
                    {
                        "[TIME LINE]".to_string()
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            mask(&format!("{k}: {v}"))
        })
        .collect();
    lines.extend(
        env.commits()
            .into_iter()
            .map(|c| mask(&format!("commit: {c}"))),
    );
    lines
}

fn same_state(tui: &Env, cli: &Env) {
    let (a, b) = (state_of(tui), state_of(cli));
    let only_tui: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
    let only_cli: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
    assert!(
        only_tui.is_empty() && only_cli.is_empty(),
        "TUI only:\n{only_tui:#?}\nCLI only:\n{only_cli:#?}"
    );
}

/// Runs what the open confirmation asks, to the end.
fn confirm(env: &Env, m: &mut Model) {
    assert!(
        matches!(m.overlay, Some(Overlay::Confirm { .. })),
        "{:?}\n{}",
        m.overlay,
        screen(m)
    );
    let cmds = ch(m, 'y');
    drive(env, m, cmds);
    assert!(m.job.is_none());
}

#[test]
fn ac8_30_sync_matches_the_cli() {
    let (tui, cli) = (held_fixture(), held_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, 's');
    confirm(&tui, &mut m);
    cli.ok(&["sync"]);
    same_state(&tui, &cli);
}

#[test]
fn ac8_30_sync_one_profile_matches_the_cli() {
    let (tui, cli) = (held_fixture(), held_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '2');
    m.cursor_mut().selected = row_of(&m, "claude ");
    ch(&mut m, 's');
    confirm(&tui, &mut m);
    cli.ok(&["sync", "--harness", "claude"]);
    same_state(&tui, &cli);
}

fn update_fixture() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.ok(&["init"]);
    let (repo, url, _) = env.upstream("superpowers", true, false, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    env.upstream_commit(&repo, "v2");
    env
}

#[test]
fn ac8_30_update_matches_the_cli() {
    let (tui, cli) = (update_fixture(), update_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'u');
    confirm(&tui, &mut m);
    cli.ok(&["update"]);
    same_state(&tui, &cli);
}

#[test]
fn ac8_30_approve_matches_the_cli() {
    let (tui, cli) = (held_fixture(), held_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'a');
    confirm(&tui, &mut m);
    cli.ok(&["approve", "superpowers"]);
    same_state(&tui, &cli);
}

#[test]
fn ac8_30_approve_all_matches_the_cli() {
    let (tui, cli) = (held_fixture(), held_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'A');
    confirm(&tui, &mut m);
    cli.ok(&["approve", "--all"]);
    same_state(&tui, &cli);
}

#[test]
fn ac8_30_reject_matches_the_cli() {
    let (tui, cli) = (held_fixture(), held_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '4');
    ch(&mut m, 'x');
    confirm(&tui, &mut m);
    cli.ok(&["reject", "superpowers"]);
    same_state(&tui, &cli);
}

/// A marketplace in a local folder, so adding from it never touches the network.
fn local_catalog_fixture() -> Env {
    let env = Env::new();
    let native = env.native();
    env.ok(&["init"]);
    let market = env.home.parent().unwrap().join("markets/local");
    let write = |path: &std::path::Path, v: serde_json::Value| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, v.to_string()).unwrap();
    };
    write(
        &market.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "local", "plugins": [
            {"name": "helper", "description": "Helps", "source": "./helper"}]}),
    );
    write(
        &native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"local": {"source": {"source": "directory", "path": market.display().to_string()},
                           "installLocation": market.display().to_string()}}),
    );
    // A git repo with the plugin in it, so adding it as a source clones from disk.
    write(
        &market.join("helper/.claude-plugin/plugin.json"),
        serde_json::json!({"name": "helper", "version": "1.0.0"}),
    );
    fs::create_dir_all(market.join("helper/skills/help")).unwrap();
    fs::write(
        market.join("helper/skills/help/SKILL.md"),
        "---\nname: help\ndescription: Helps\n---\n",
    )
    .unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "v1",
        ],
    ] {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(&market)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }
    env
}

#[test]
fn ac8_30_add_matches_the_cli() {
    let (tui, cli) = (local_catalog_fixture(), local_catalog_fixture());
    let mut m = opened(&tui, 100, 30);
    ch(&mut m, '5');
    m.cursor_mut().selected = row_of(&m, "helper");
    let cmds = press(&mut m, KeyCode::Enter);
    drive(&tui, &mut m, cmds);
    ch(&mut m, 'a');
    let Some(Overlay::Confirm {
        job: Job::Add(words),
        ..
    }) = &m.overlay
    else {
        panic!("{:?}", m.overlay);
    };
    let words = words.clone();
    assert_eq!(words[0], "add", "{words:?}");
    confirm(&tui, &mut m);
    let args: Vec<&str> = words.iter().map(String::as_str).collect();
    cli.ok(&args);
    same_state(&tui, &cli);
}

// ── The rest of the edge cases ──────────────────────────────────────────────────────────────

#[test]
fn ac8_29_reload_keeps_the_tab_and_the_row() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '3');
    m.cursor_mut().selected = row_of(&m, "superpowers");
    let before = m.cursor().selected;
    // Changed outside: a new skill.
    env.source_skill("beta");
    env.ok(&["sync"]);
    let cmds = ch(&mut m, 'r');
    drive(&env, &mut m, cmds);
    assert_eq!(m.tab, Tab::Items);
    assert!(screen(&m).contains("beta"));
    let now = row_of(&m, "superpowers");
    assert_eq!(m.cursor().selected, before.min(m.list_len() - 1));
    let _ = now;
}

#[test]
fn ec8_11_outside_changes_show_on_reload_only() {
    let env = in_sync_fixture();
    let mut m = opened(&env, 100, 30);
    env.source_skill("gamma");
    env.ok(&["sync"]);
    ch(&mut m, '3');
    assert!(!screen(&m).contains("gamma"));
    let cmds = ch(&mut m, 'r');
    drive(&env, &mut m, cmds);
    assert!(screen(&m).contains("gamma"));
}

#[test]
fn ec8_06_big_lists_and_diffs_draw_fast() {
    use std::time::Instant;
    let mut m = model(120, 40);
    m.overlay = Some(Overlay::Detail {
        title: "big".into(),
        lines: (0..10_000).map(|i| format!("+line {i}")).collect(),
        scroll: 5_000,
        held: None,
    });
    let started = Instant::now();
    let s = screen(&m);
    let took = started.elapsed();
    assert!(s.contains("+line 5000"), "{s}");
    // Generous for unoptimised, instrumented test builds; a release frame is far below 16 ms.
    assert!(took.as_millis() < 250, "{took:?}");
}

#[test]
fn ec8_06_a_thousand_plugins() {
    use std::time::Instant;
    let env = Env::new();
    let native = env.native();
    env.ok(&["init"]);
    let market = env.home.parent().unwrap().join("markets/big");
    let plugins: Vec<serde_json::Value> = (0..1000)
        .map(|i| serde_json::json!({"name": format!("plugin-{i:04}"), "description": "x", "source": format!("./p{i}")}))
        .collect();
    fs::create_dir_all(market.join(".claude-plugin")).unwrap();
    fs::write(
        market.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "big", "plugins": plugins}).to_string(),
    )
    .unwrap();
    fs::write(
        native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"big": {"source": {"source": "directory", "path": market.display().to_string()},
                           "installLocation": market.display().to_string()}})
        .to_string(),
    )
    .unwrap_or_else(|_| {
        fs::create_dir_all(native.join("plugins")).unwrap();
        fs::write(
            native.join("plugins/known_marketplaces.json"),
            serde_json::json!({"big": {"source": {"source": "directory", "path": market.display().to_string()},
                               "installLocation": market.display().to_string()}})
            .to_string(),
        )
        .unwrap();
    });
    let mut m = opened(&env, 120, 40);
    ch(&mut m, '5');
    assert_eq!(m.list_len(), 1000);
    press(&mut m, KeyCode::End);
    let started = Instant::now();
    let s = screen(&m);
    let took = started.elapsed();
    assert!(s.contains("plugin-0999") && s.contains("1000/1000"), "{s}");
    assert!(took.as_millis() < 1000, "{took:?}");
}

#[test]
fn ec8_07_wide_characters_stay_aligned() {
    let env = Env::new();
    let native = env.native();
    env.ok(&["init"]);
    let market = env.home.parent().unwrap().join("markets/wide");
    fs::create_dir_all(market.join(".claude-plugin")).unwrap();
    fs::write(
        market.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "wide", "plugins": [
            {"name": "工具箱", "description": "道具 🧰 tools", "source": "./a"},
            {"name": "ascii", "description": "plain", "source": "./b"}]})
        .to_string(),
    )
    .unwrap();
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"wide": {"source": {"source": "directory", "path": market.display().to_string()},
                           "installLocation": market.display().to_string()}})
        .to_string(),
    )
    .unwrap();
    let mut m = opened(&env, 80, 24);
    ch(&mut m, '5');
    let rows = view::rows(&m);
    let col = |needle: &str| {
        let line = rows
            .iter()
            .map(|l| l.to_string())
            .find(|l| l.contains(needle))
            .unwrap();
        let at = line.find(needle).unwrap();
        colctl::ui::render::width(&line[..at])
    };
    // The developer column starts at the same width in both rows.
    let s = screen(&m);
    let wide = s.lines().find(|l| l.contains("工具箱")).unwrap();
    let narrow = s.lines().find(|l| l.contains("ascii")).unwrap();
    let dev = |l: &str, name: &str| {
        let rest = &l[l.find(name).unwrap() + name.len()..];
        let skip = rest.len() - rest.trim_start().len();
        colctl::ui::render::width(&l[..l.find(name).unwrap() + name.len() + skip])
    };
    assert_eq!(dev(wide, "工具箱"), dev(narrow, "ascii"), "{s}");
    let _ = col;
}

#[test]
fn ec8_10_another_colctl_holds_the_lock() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    let lock = env.source.join(".state/lock");
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    fs::write(&lock, std::process::id().to_string()).unwrap();
    ch(&mut m, 's');
    confirm(&env, &mut m);
    let s = screen(&m);
    assert!(s.contains("error: another colctl is running"), "{s}");
    assert!(s.contains("hint: wait for the other colctl"), "{s}");
    fs::remove_file(&lock).unwrap();
}

#[test]
fn ec8_12_one_profile_failing_leaves_the_others() {
    use std::os::unix::fs::PermissionsExt;
    let env = Env::new();
    env.native();
    let kilo = env.kilo();
    env.ok(&["init"]);
    env.source_skill("alpha");
    fs::create_dir_all(kilo.join("skills")).unwrap();
    fs::set_permissions(kilo.join("skills"), fs::Permissions::from_mode(0o555)).unwrap();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, 's');
    let cmds = ch(&mut m, 'y');
    let c = ctx(&env);
    let mut after = Vec::new();
    for msg in worker::run_now(&c, &cmds[0], 96) {
        if matches!(msg, Msg::Job(JobEvent::Finished(_))) {
            let s = screen(&m);
            assert!(s.contains("✓ claude"), "{s}");
            assert!(s.contains("✗ kilo-claude"), "{s}");
        }
        after.extend(update(&mut m, msg));
    }
    fs::set_permissions(kilo.join("skills"), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        m.toast.as_ref().is_some_and(|t| !t.text.starts_with('✓')),
        "{:?}",
        m.toast
    );
    assert!(env.home.join(".claude/skills/alpha").exists());
}

#[test]
fn ec8_13_without_git_reads_work_and_writes_say_why() {
    let env = Env::without_git();
    env.native();
    let mut m = opened(&env, 100, 30);
    // No ~/.colctl and no git: it still opens and says what to do.
    assert!(screen(&m).contains("colctl isn't set up yet"));
    ch(&mut m, 's');
    confirm(&env, &mut m);
    let s = screen(&m);
    assert!(s.contains("error: colctl needs git"), "{s}");
}

#[test]
fn ec8_14_a_catalog_that_fails_is_named() {
    let env = catalog_fixture();
    fs::write(
        env.home.join(".claude/plugins/known_marketplaces.json"),
        serde_json::json!({"broken": {"source": {"source": "github", "repo": "x/y"},
                           "installLocation": env.home.join("nowhere").display().to_string()}})
        .to_string(),
    )
    .unwrap();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    let s = screen(&m);
    let (_, _, err) = env.code(&["search"]);
    let warned = err.lines().any(|l| l.contains("warning"));
    if warned {
        assert!(s.contains("! "), "{s}");
    }
    assert!(!s.contains("error:"), "{s}");
}

#[test]
fn ec8_16_a_dead_terminal_waits_for_the_write() {
    let mut m = model(80, 24);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(Msg::Job(JobEvent::Step {
        profile: "claude".into(),
        action: "linking".into(),
        n: 1,
        total: 1,
    }))
    .unwrap();
    tx.send(Msg::Job(JobEvent::Finished(Ok(report("✓ done")))))
        .unwrap();
    colctl::tui::wait_for_job(&rx, &mut m);
    assert!(m.job.is_none());
    // Nothing running: returns at once.
    colctl::tui::wait_for_job(&rx, &mut m);
}

#[test]
fn constraint_the_tui_has_no_logic_of_its_own() {
    for entry in fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tui")).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for banned in [
            "crate::commands",
            "crate::plan",
            "crate::apply",
            "crate::adapter",
        ] {
            assert!(
                !text.contains(banned),
                "{} uses {banned}; go through crate::core",
                path.display()
            );
        }
    }
}

// ── Revise (AC8-32 – 35, 38, 39, EC8-27) ────────────────────────────────────────────────────

#[test]
fn ac8_32_arrows_switch_tabs() {
    let mut m = model(80, 24);
    press(&mut m, KeyCode::Right);
    assert_eq!(m.tab, Tab::Profiles);
    press(&mut m, KeyCode::Left);
    press(&mut m, KeyCode::Left);
    assert_eq!(m.tab, Tab::Discover);
    press(&mut m, KeyCode::Right);
    assert_eq!(m.tab, Tab::Overview);
    // Not behind an overlay.
    ch(&mut m, '?');
    press(&mut m, KeyCode::Right);
    assert_eq!(m.tab, Tab::Overview);
}

#[test]
fn ac8_33_overview_offers_a_menu() {
    let env = held_fixture();
    let mut m = opened(&env, 100, 30);
    let rows: Vec<String> = view::rows(&m).iter().map(|l| l.to_string()).collect();
    assert!(rows[0].starts_with("Sync every profile"), "{rows:#?}");
    assert!(rows[1].starts_with("Check for updates"));
    assert!(rows[2].starts_with("Review 1 held version"));
    assert!(rows[3].starts_with("Find a plugin"));
    assert!(rows[4].starts_with("Look through every item"));
    let s = screen(&m);
    let menu = s
        .lines()
        .position(|l| l.contains("Sync every profile"))
        .unwrap();
    let profiles = s.lines().position(|l| l.trim() == "Profiles").unwrap();
    assert!(menu < profiles, "the menu comes before the profiles:\n{s}");
    insta::assert_snapshot!("overview_menu_100x30", mask(&s));
    // ⏎ on Sync asks; on Review goes there; on Find goes to Discover.
    press(&mut m, KeyCode::Enter);
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm {
            job: Job::Sync(None),
            ..
        })
    ));
    ch(&mut m, 'n');
    ch(&mut m, 'j');
    press(&mut m, KeyCode::Enter);
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm {
            job: Job::Update(_),
            ..
        })
    ));
    ch(&mut m, 'n');
    ch(&mut m, 'j');
    press(&mut m, KeyCode::Enter);
    assert_eq!(m.tab, Tab::Review);
    ch(&mut m, '1');
    m.cursor_mut().selected = 3;
    press(&mut m, KeyCode::Enter);
    assert_eq!(m.tab, Tab::Discover);
    ch(&mut m, 'x');
    press(&mut m, KeyCode::Esc);
    press(&mut m, KeyCode::Right);
    assert_eq!(m.tab, Tab::Overview);
    // ↓ from the last entry skips the profiles and lands on what needs you.
    m.cursor_mut().selected = 4;
    ch(&mut m, 'j');
    let at = view::rows(&m)[m.cursor().selected].to_string();
    assert!(at.starts_with("! "), "{at:?}");
    ch(&mut m, 'k');
    assert_eq!(m.cursor().selected, 4);
    // u checks for updates from Overview too.
    ch(&mut m, 'u');
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm {
            job: Job::Update(_),
            ..
        })
    ));
}

fn thousand() -> Env {
    let env = Env::new();
    let native = env.native();
    env.ok(&["init"]);
    let market = env.home.parent().unwrap().join("markets/big");
    let plugins: Vec<serde_json::Value> = (0..1000)
        .map(|i| serde_json::json!({"name": format!("plugin-{i:04}"), "description": "x", "source": format!("./p{i}")}))
        .collect();
    fs::create_dir_all(market.join(".claude-plugin")).unwrap();
    fs::write(
        market.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "big", "plugins": plugins}).to_string(),
    )
    .unwrap();
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"big": {"source": {"source": "directory", "path": market.display().to_string()},
                           "installLocation": market.display().to_string()}})
        .to_string(),
    )
    .unwrap();
    env
}

#[test]
fn ac8_34_the_wheel_scrolls_the_view() {
    let env = thousand();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    update(&mut m, Msg::Scroll(3));
    assert_eq!(m.cursor().offset, 3);
    assert_eq!(m.cursor().selected, 3, "the selection stays on screen");
    let sel = m.cursor().selected;
    m.cursor_mut().selected = sel + 5;
    update(&mut m, Msg::Scroll(3));
    assert_eq!(m.cursor().offset, 6);
    assert_eq!(
        m.cursor().selected,
        sel + 5,
        "a visible selection doesn't move"
    );
    update(&mut m, Msg::Scroll(-99));
    assert_eq!(m.cursor().offset, 3, "at most 3 rows a frame");
}

#[test]
fn ec8_27_a_trackpad_flick_is_three_rows() {
    let wheel = |kind| {
        Msg::Mouse(MouseEvent {
            kind,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    };
    let burst: Vec<Msg> = (0..200)
        .map(|_| wheel(MouseEventKind::ScrollDown))
        .collect();
    let out = colctl::tui::coalesce(burst);
    assert!(matches!(out.as_slice(), [Msg::Scroll(3)]), "{out:?}");
    let mixed = vec![
        wheel(MouseEventKind::ScrollUp),
        Msg::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
        wheel(MouseEventKind::ScrollDown),
        wheel(MouseEventKind::ScrollDown),
    ];
    let out = colctl::tui::coalesce(mixed);
    assert!(
        matches!(
            out.as_slice(),
            [Msg::Scroll(-1), Msg::Key(_), Msg::Scroll(2)]
        ),
        "{out:?}"
    );
}

#[test]
fn ac8_35_discover_searches_as_you_type() {
    let env = catalog_fixture();
    let mut m = opened(&env, 100, 30);
    ch(&mut m, '5');
    for c in "tl".chars() {
        ch(&mut m, c);
    }
    assert_eq!(m.filter().text, "tl");
    assert_eq!(m.list_len(), 2);
    // q types, it doesn't quit; ? is still help.
    assert!(ch(&mut m, 'q').is_empty());
    assert!(!m.quit);
    press(&mut m, KeyCode::Backspace);
    ch(&mut m, '?');
    assert_eq!(m.overlay, Some(Overlay::Help));
    press(&mut m, KeyCode::Esc);
    // The key bar says how it works.
    let keys = screen(&m).lines().last().unwrap().to_string();
    assert!(keys.contains("type to search"), "{keys}");
    assert!(!keys.contains("q quit"), "{keys}");
    // Results are ranked once per query.
    let (a, _) = colctl::tui::tabs::results(&m);
    let (b, _) = colctl::tui::tabs::results(&m);
    assert!(std::rc::Rc::ptr_eq(&a, &b));
    // ⏎ opens, a adds from there.
    let cmds = press(&mut m, KeyCode::Enter);
    assert!(matches!(cmds.as_slice(), [Cmd::Load(Load::Show(_))]));
    drive(&env, &mut m, cmds);
    assert!(screen(&m).contains("a add"), "{}", screen(&m));
    ch(&mut m, 'a');
    assert!(matches!(
        &m.overlay,
        Some(Overlay::Confirm {
            job: Job::Add(_),
            ..
        })
    ));
    ch(&mut m, 'n');
    press(&mut m, KeyCode::Esc);
    assert_eq!(m.filter().text, "");
    assert_eq!(m.list_len(), 3);
}

#[test]
fn ac8_39_a_failed_run_says_why() {
    let mut m = model(100, 30);
    ask(&mut m, "Sync", vec![], Job::Sync(None));
    ch(&mut m, 'y');
    let failed = Report {
        exit: 1,
        lines: vec![
            "✗ Synced 3 of 5 profiles · 2 failed".into(),
            "  ✗  claude       1 step failed".into(),
            "      ✗ update qdrant failed: qdrant@qdrant is still at d2127f6".into(),
            "→ colctl sync retries".into(),
        ],
        errors: Vec::new(),
    };
    update(&mut m, Msg::Job(JobEvent::Finished(Ok(failed))));
    let s = screen(&m);
    assert!(s.contains("Result: colctl sync"), "{s}");
    assert!(
        s.contains("update qdrant failed: qdrant@qdrant is still at d2127f6"),
        "{s}"
    );
}

#[test]
fn ac8_36_the_summary_has_one_line_per_flagged_file() {
    use colctl::core::quarantine::HeldSummary;
    use colctl::risk::{Flag, FlagKind, recommend};
    let flags = vec![
        Flag {
            kind: FlagKind::Executable,
            path: "build.sh".into(),
            reason: "changed executable file".into(),
        },
        Flag {
            kind: FlagKind::Script,
            path: "build.sh".into(),
            reason: "changed script".into(),
        },
    ];
    let h = HeldSummary {
        name: "q".into(),
        kind: "source".into(),
        change: "a → b".into(),
        size: "1 commit".into(),
        flags: 2,
        error: None,
        files: 1,
        recommendation: Some(recommend(&flags)),
        flag_list: flags,
    };
    let lines = colctl::tui::tabs::held_summary(&h);
    assert!(
        lines.contains(&"Flagged (1 file)".to_string()),
        "{lines:#?}"
    );
    assert!(
        lines.contains(&"  build.sh  changed executable file; changed script".to_string()),
        "{lines:#?}"
    );
}

// ── Cycle 9: the rest of the commands (AC9-17 – 22) ─────────────────────────────────────────

mod rest {
    use super::*;
    use colctl::tui::app::DiscoverView;

    fn ctrl(m: &mut Model, c: char) -> Vec<Cmd> {
        update(
            m,
            Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
        )
    }

    fn machine() -> Env {
        let env = Env::new();
        env.native();
        env.kilo();
        env.codex();
        env.agents_dir();
        env.ok(&["init"]);
        env
    }

    fn with_skill() -> Env {
        let env = machine();
        env.source_skill("alpha");
        env.ok(&["sync"]);
        env
    }

    fn write(path: &std::path::Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A Claude-only source; with `skills`, one Codex can use once converted.
    fn claude_only(skills: bool) -> Env {
        let env = machine();
        let repo = env.home.parent().unwrap().join("upstreams").join("cli");
        if skills {
            write(&repo.join("skills/edge/SKILL.md"), "---\nname: edge\n---\n");
        }
        let (_, url, head) = env.upstream("cli", true, false, Some("cli"));
        env.add_source_entry("cli", &url, &head, "");
        env.ok(&["sync"]);
        env
    }

    fn taken_over() -> Env {
        let env = with_skill();
        let origin = env.skill_dir(&env.home.join(".config/twg/skills"), "twg-jira", "v1");
        let skills = env.home.join(".agents/skills");
        fs::create_dir_all(&skills).unwrap();
        std::os::unix::fs::symlink(&origin, skills.join("twg-jira")).unwrap();
        env.ok(&["adopt", "agents", "skills/twg-jira"]);
        fs::write(origin.join("SKILL.md"), "v2").unwrap();
        env
    }

    fn with_foreign() -> Env {
        let env = machine();
        env.skill_dir(&env.home.join(".claude/skills"), "mine", "mine");
        env.skill_dir(&env.home.join(".claude/skills"), "yours", "yours");
        env
    }

    fn items_at(env: &Env, name: &str) -> Model {
        let mut m = opened(env, 110, 32);
        ch(&mut m, '3');
        m.cursor_mut().selected = row_of(&m, name);
        m
    }

    #[test]
    fn ac9_17_remove_from_items() {
        let (tui, cli) = (with_skill(), with_skill());
        let mut m = items_at(&tui, "alpha");
        ch(&mut m, 'x');
        let Some(Overlay::Confirm { job, .. }) = &m.overlay else {
            panic!("{:?}", m.overlay);
        };
        assert_eq!(
            *job,
            Job::Rm {
                kind: "skill".into(),
                name: "alpha".into(),
                codex: false
            }
        );
        assert!(screen(&m).contains("Remove skill alpha"), "{}", screen(&m));
        ch(&mut m, 'n');
        assert!(m.overlay.is_none());
        assert!(
            tui.source.join("skills/alpha").exists(),
            "n changes nothing"
        );
        ch(&mut m, 'x');
        confirm(&tui, &mut m);
        cli.ok(&["rm", "skill", "alpha", "-y"]);
        same_state(&tui, &cli);
    }

    #[test]
    fn ac9_18_convert_and_undo_from_items() {
        let (tui, cli) = (claude_only(true), claude_only(true));
        let mut m = items_at(&tui, "cli");
        let cmds = ch(&mut m, 'c');
        assert_eq!(cmds, vec![Cmd::Load(Load::ConvertPlan("cli".into()))]);
        drive(&tui, &mut m, cmds);
        let Some(Overlay::Confirm { job, body, .. }) = &m.overlay else {
            panic!("{:?}\n{}", m.overlay, screen(&m));
        };
        assert_eq!(*job, Job::Convert("cli".into()));
        assert!(body.join("\n").contains("1 skill"), "{body:#?}");
        confirm(&tui, &mut m);
        cli.ok(&["convert", "cli"]);
        same_state(&tui, &cli);
    }

    /// A Claude-only source whose Codex package was approved.
    fn converted() -> Env {
        let env = claude_only(true);
        env.ok(&["convert", "cli"]);
        env.run(&["approve", "--all", "-y"]);
        env
    }

    #[test]
    fn ac9_18_undo_a_conversion_from_items() {
        let (tui, cli) = (converted(), converted());
        let mut m = items_at(&tui, "cli");
        ch(&mut m, 'c');
        let Some(Overlay::Confirm { job, .. }) = &m.overlay else {
            panic!("{:?}", m.overlay);
        };
        assert_eq!(*job, Job::ConvertUndo("cli".into()));
        confirm(&tui, &mut m);
        cli.run(&["convert", "cli", "--undo"]);
        same_state(&tui, &cli);
    }

    #[test]
    fn ac9_18_nothing_codex_can_use_says_why() {
        let env = claude_only(false);
        let mut m = items_at(&env, "cli");
        let cmds = ch(&mut m, 'c');
        drive(&env, &mut m, cmds);
        assert!(
            !matches!(m.overlay, Some(Overlay::Confirm { .. })),
            "nothing to run: {:?}",
            m.overlay
        );
        assert!(
            screen(&m).contains("nothing Codex can use"),
            "{}",
            screen(&m)
        );
    }

    #[test]
    fn ac9_19_refresh_from_items() {
        let env = taken_over();
        let mut m = items_at(&env, "twg-jira");
        let cmds = ch(&mut m, 'f');
        assert_eq!(
            cmds,
            vec![Cmd::Run(Job::Refresh(vec!["skills/twg-jira".into()]))]
        );
        drive(&env, &mut m, cmds);
        assert_eq!(m.held.ready().map(Vec::len), Some(1), "held for review");
        m.cursor_mut().selected = row_of(&m, "alpha");
        assert!(ch(&mut m, 'f').is_empty());
        assert!(
            screen(&m).contains("alpha isn't taken over"),
            "{}",
            screen(&m)
        );
    }

    #[test]
    fn ac9_20_adopt_from_profiles() {
        let (tui, cli) = (with_foreign(), with_foreign());
        let mut m = opened(&tui, 110, 32);
        ch(&mut m, '2');
        m.cursor_mut().selected = row_of(&m, "~/.claude");
        let cmds = ch(&mut m, 'a');
        assert_eq!(cmds, vec![Cmd::Load(Load::Adopt("claude".into()))]);
        drive(&tui, &mut m, cmds);
        let Some(Overlay::Pick { items, .. }) = &m.overlay else {
            panic!("{:?}", m.overlay);
        };
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|(_, on)| *on), "all ticked");
        assert!(screen(&m).contains("skills/yours"), "{}", screen(&m));
        press(&mut m, KeyCode::Down);
        ch(&mut m, ' ');
        press(&mut m, KeyCode::Enter);
        let Some(Overlay::Confirm { job, .. }) = &m.overlay else {
            panic!("{:?}", m.overlay);
        };
        assert_eq!(
            *job,
            Job::Adopt {
                profile: "claude".into(),
                items: vec!["skills/mine".into()]
            }
        );
        confirm(&tui, &mut m);
        cli.run(&["adopt", "claude", "skills/mine"]);
        same_state(&tui, &cli);
    }

    #[test]
    fn ac9_20_nothing_to_adopt_says_so() {
        let env = machine();
        let mut m = opened(&env, 110, 32);
        ch(&mut m, '2');
        m.cursor_mut().selected = row_of(&m, "kilo-claude");
        let cmds = ch(&mut m, 'a');
        drive(&env, &mut m, cmds);
        assert!(m.overlay.is_none(), "{:?}", m.overlay);
        assert!(
            screen(&m).contains("Nothing in kilo-claude that colctl doesn't already have"),
            "{}",
            screen(&m)
        );
    }

    #[test]
    fn ac9_21_discover_cycles_its_views() {
        let env = local_catalog_fixture();
        let mut m = opened(&env, 110, 32);
        ch(&mut m, '5');
        assert_eq!(m.discover_view, DiscoverView::Plugins);
        let cmds = ctrl(&mut m, 'v');
        drive(&env, &mut m, cmds);
        assert_eq!(m.discover_view, DiscoverView::Developers);
        assert!(screen(&m).contains("Developers"), "{}", screen(&m));
        let dev = m.catalog.ready().unwrap().plugins[0].developer.clone();
        m.cursor_mut().selected = row_of(&m, &dev);
        press(&mut m, KeyCode::Enter);
        assert_eq!(m.discover_view, DiscoverView::Plugins);
        assert!(screen(&m).contains("helper"), "{}", screen(&m));
        ctrl(&mut m, 'v');
        let cmds = ctrl(&mut m, 'v');
        drive(&env, &mut m, cmds);
        assert_eq!(m.discover_view, DiscoverView::Marketplaces);
        assert!(screen(&m).contains("local"), "{}", screen(&m));
        m.cursor_mut().selected = row_of(&m, "local");
        press(&mut m, KeyCode::Enter);
        let Some(Overlay::Confirm { job, .. }) = &m.overlay else {
            panic!("{:?}", m.overlay);
        };
        assert_eq!(*job, Job::MarketplaceRefresh);
        press(&mut m, KeyCode::Esc);
        let cmds = ctrl(&mut m, 'v');
        drive(&env, &mut m, cmds);
        assert_eq!(m.discover_view, DiscoverView::Connectors);
        ctrl(&mut m, 'v');
        assert_eq!(m.discover_view, DiscoverView::Plugins);
    }

    #[test]
    fn ac9_21_typing_filters_the_view_showing() {
        let env = local_catalog_fixture();
        let mut m = opened(&env, 110, 32);
        ch(&mut m, '5');
        ctrl(&mut m, 'v');
        let cmds = ctrl(&mut m, 'v');
        drive(&env, &mut m, cmds);
        assert_eq!(m.discover_view, DiscoverView::Marketplaces);
        for c in "zzz".chars() {
            ch(&mut m, c);
        }
        assert!(!screen(&m).contains("local "), "{}", screen(&m));
        assert!(
            screen(&m).contains("No marketplaces match"),
            "{}",
            screen(&m)
        );
    }

    #[test]
    fn ac9_22_every_command_is_reachable() {
        use colctl::tui::keys::{Action, KEYMAP, Scope};
        let has = |scope: Scope, action: Action| {
            KEYMAP
                .iter()
                .any(|b| b.scope == scope && b.action == action)
        };
        // Each command, and the key that runs it.
        let table: &[(&str, Scope, Action)] = &[
            ("status", Scope::Global, Action::Reload),
            ("sync", Scope::Tab(Tab::Overview), Action::Sync),
            ("add", Scope::Tab(Tab::Discover), Action::Open),
            ("rm", Scope::Tab(Tab::Items), Action::Remove),
            ("adopt", Scope::Tab(Tab::Profiles), Action::Adopt),
            ("convert", Scope::Tab(Tab::Items), Action::Convert),
            ("update", Scope::Tab(Tab::Review), Action::Update),
            ("refresh", Scope::Tab(Tab::Items), Action::Refresh),
            ("review", Scope::Tab(Tab::Review), Action::Open),
            ("approve", Scope::Tab(Tab::Review), Action::Approve),
            ("reject", Scope::Tab(Tab::Review), Action::Reject),
            ("search", Scope::Tab(Tab::Discover), Action::Filter),
            ("show", Scope::Tab(Tab::Discover), Action::Open),
            ("developers", Scope::Tab(Tab::Discover), Action::NextView),
            ("marketplace", Scope::Tab(Tab::Discover), Action::NextView),
            ("connectors", Scope::Tab(Tab::Discover), Action::NextView),
        ];
        let help = Env::new().ok(&["--help"]);
        let commands: Vec<&str> = help
            .lines()
            .filter(|l| l.starts_with("  ") && !l.trim_start().starts_with('-'))
            .filter_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                match w.as_slice() {
                    ["colctl", c, ..] => Some(*c),
                    [c, ..] => Some(*c),
                    _ => None,
                }
            })
            .filter(|c| !["init", "completions", "version", "help"].contains(c))
            .collect();
        assert!(commands.len() >= 16, "{commands:?}");
        for c in commands {
            let Some((_, scope, action)) = table.iter().find(|(n, ..)| *n == c) else {
                panic!("no TUI key for colctl {c}");
            };
            assert!(
                has(*scope, *action),
                "colctl {c}: no {action:?} in {scope:?}"
            );
        }
    }

    #[test]
    fn ec9_12_connectors_without_a_codex_profile() {
        let env = Env::without_codex();
        env.native();
        env.ok(&["init"]);
        let mut m = opened(&env, 110, 32);
        ch(&mut m, '5');
        for _ in 0..3 {
            let cmds = ctrl(&mut m, 'v');
            drive(&env, &mut m, cmds);
        }
        assert_eq!(m.discover_view, DiscoverView::Connectors);
        assert!(screen(&m).contains("codex"), "{}", screen(&m));
    }

    #[test]
    fn ec9_16_a_write_waits_for_the_running_one() {
        let env = taken_over();
        let mut m = items_at(&env, "twg-jira");
        m.job = Some(app::Running {
            job: Job::Sync(None),
            rows: Vec::new(),
            started: 0,
        });
        assert!(ch(&mut m, 'f').is_empty());
        // The running job owns the status line; the toast says why nothing started.
        let toast = m.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
        assert!(toast.contains("Wait for colctl sync to finish"), "{toast}");
    }

    #[test]
    fn ec9_17_remove_refuses_what_colctl_doesnt_own() {
        let env = with_foreign();
        let mut m = items_at(&env, "mine");
        ch(&mut m, 'x');
        confirm(&env, &mut m);
        assert!(
            matches!(m.overlay, Some(Overlay::Error { .. })),
            "{:?}",
            m.overlay
        );
        assert!(screen(&m).contains("no managed skill"), "{}", screen(&m));
        assert!(env.home.join(".claude/skills/mine").exists());
    }
}
