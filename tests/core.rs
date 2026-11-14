//! Cycle 8: the core API the CLI and the TUI share (docs/tui/interfaces.md § Core API). Named after
//! AC8-NN / EC8-NN.

mod support;

use colctl::core::status::{self, Tone};
use colctl::ctx::Ctx;
use std::ffi::OsString;
use std::fs;
use support::Env;

/// A context for in-process calls on `env`'s machine.
fn ctx(env: &Env) -> Ctx {
    Ctx::from_vars(&[
        ("HOME".into(), OsString::from(&env.home)),
        ("PATH".into(), env.path.clone()),
    ])
    .unwrap()
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

/// The verdict sentence of `colctl status`'s first line, without its symbol and facts.
fn cli_verdict(env: &Env) -> (i32, String) {
    let (code, out, _) = env.code(&["status", "--color", "never"]);
    let first = out.lines().next().unwrap_or_default();
    let text = first
        .trim_start_matches(['✓', '!', '✗', '·', ' '])
        .split(" · ")
        .next()
        .unwrap()
        .to_string();
    (code, text)
}

#[test]
fn ac8_01_status_snapshot_matches_the_cli() {
    let env = held_fixture();
    let snap = status::snapshot(&ctx(&env), None).unwrap();
    let (code, text) = cli_verdict(&env);
    assert_eq!(snap.verdict.text, text);
    assert_eq!(snap.verdict.tone, Tone::Attention);
    assert_eq!(snap.exit, code);

    let json = env.status_json();
    let names: Vec<&str> = json["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].as_str().unwrap())
        .collect();
    let ours: Vec<&str> = snap.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(ours, names);

    let held: Vec<&str> = snap.held.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(held, ["superpowers"]);
    assert!(
        snap.attention
            .iter()
            .any(|a| a.name == "alpha" && a.what == "missing" && a.fix == "colctl sync"),
        "{:?}",
        snap.attention
    );
    assert!(
        snap.attention
            .iter()
            .any(|a| a.name == "superpowers" && a.fix == "colctl review superpowers")
    );

    let claude = snap.profiles.iter().find(|p| p.name == "claude").unwrap();
    assert!(claude.usable);
    assert_eq!(claude.mark, Tone::Attention);
    assert!(
        claude.contents.iter().any(|(t, _)| t == "1 missing"),
        "{:?}",
        claude.contents
    );
    assert!(
        claude
            .items
            .iter()
            .any(|i| i.kind == "skill" && i.name == "alpha" && i.state == "missing")
    );
}

#[test]
fn ac8_01_status_snapshot_in_sync() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    env.ok(&["sync"]);
    let snap = status::snapshot(&ctx(&env), None).unwrap();
    assert_eq!(snap.verdict.tone, Tone::Ok);
    assert_eq!(snap.verdict.text, "The profile is in sync");
    assert_eq!(snap.exit, 0);
    assert!(snap.attention.is_empty());
    assert!(snap.held.is_empty());
}

#[test]
fn ac8_01_status_snapshot_without_profiles() {
    let env = Env::new();
    env.ok(&["init"]);
    let snap = status::snapshot(&ctx(&env), None).unwrap();
    assert_eq!(snap.verdict.text, "No profiles registered");
    assert_eq!(snap.verdict.tone, Tone::Attention);
    assert!(snap.profiles.is_empty());
}

mod runs {
    use super::*;
    use colctl::core::observer::{Event, Recorder};
    use colctl::core::run::{Job, run};
    use std::sync::{Arc, Mutex};

    fn recorder() -> Arc<Mutex<Recorder>> {
        Arc::new(Mutex::new(Recorder::default()))
    }

    fn events(r: &Arc<Mutex<Recorder>>) -> Vec<Event> {
        r.lock().unwrap().events.clone()
    }

    #[test]
    fn ac8_02_sync_reports_progress_through_the_observer() {
        let env = Env::new();
        env.native();
        env.kilo();
        env.ok(&["init"]);
        env.source_skill("alpha");
        let rec = recorder();
        let report = run(&ctx(&env), &Job::Sync(None), 80, Box::new(rec.clone())).unwrap();
        assert_eq!(report.exit, 0, "{report:?}");
        assert!(report.verdict().starts_with('✓'), "{report:?}");
        let ev = events(&rec);
        for profile in ["claude", "kilo-claude"] {
            assert!(
                ev.iter()
                    .any(|e| matches!(e, Event::Step { profile: p, .. } if p == profile)),
                "{ev:?}"
            );
            assert!(
                ev.iter()
                    .any(|e| matches!(e, Event::Done { profile: p, ok: true, .. } if p == profile)),
                "{ev:?}"
            );
        }
        // The same state colctl sync leaves: nothing left to do.
        assert_eq!(env.code(&["status"]).0, 0);
        assert!(env.home.join(".claude/skills/alpha").exists());
    }

    #[test]
    fn ac8_02_update_reports_each_source_and_stops_when_asked() {
        let env = Env::new();
        env.native();
        env.ok(&["init"]);
        let (a, url_a, _) = env.upstream("alpha-plugin", true, false, None);
        let (b, url_b, _) = env.upstream("beta-plugin", true, false, None);
        env.ok(&["add", "source", &url_a]);
        env.ok(&["add", "source", &url_b]);
        env.upstream_commit(&a, "v2");
        env.upstream_commit(&b, "v2");

        let rec = Arc::new(Mutex::new(Recorder {
            cancel_after: Some(1),
            ..Recorder::default()
        }));
        let report = run(&ctx(&env), &Job::Update(vec![]), 80, Box::new(rec.clone())).unwrap();
        let steps: Vec<String> = events(&rec)
            .into_iter()
            .filter_map(|e| match e {
                Event::Step {
                    profile, n, total, ..
                } => Some(format!("{profile} {n}/{total}")),
                _ => None,
            })
            .collect();
        assert_eq!(steps.len(), 1, "{steps:?}");
        assert!(steps[0].ends_with("1/2"), "{steps:?}");
        assert!(
            events(&rec)
                .iter()
                .any(|e| matches!(e, Event::Done { ok: true, .. })),
            "{:?}",
            events(&rec)
        );
        // What was held before the stop stays held.
        let snap = status::snapshot(&ctx(&env), None).unwrap();
        assert_eq!(snap.held.len(), 1, "{report:?}");
    }

    #[test]
    fn ac8_02_approve_installs_and_reports() {
        let env = held_fixture();
        let rec = recorder();
        let report = run(
            &ctx(&env),
            &Job::Approve(vec!["superpowers".into()]),
            80,
            Box::new(rec.clone()),
        )
        .unwrap();
        assert!(
            report.lines.iter().any(|l| l.contains("superpowers")),
            "{report:?}"
        );
        assert!(
            events(&rec).iter().any(|e| matches!(e, Event::Step { .. })),
            "{:?}",
            events(&rec)
        );
        let snap = status::snapshot(&ctx(&env), None).unwrap();
        assert!(snap.held.is_empty());
    }

    #[test]
    fn ac8_02_reject_drops_the_held_version() {
        let env = held_fixture();
        let report = run(
            &ctx(&env),
            &Job::Reject(vec!["superpowers".into()]),
            80,
            Box::new(recorder()),
        )
        .unwrap();
        assert_eq!(report.exit, 0, "{report:?}");
        let snap = status::snapshot(&ctx(&env), None).unwrap();
        assert!(snap.held.is_empty());
    }

    #[test]
    fn ac8_02_a_refusal_is_the_clis_error() {
        let env = Env::new();
        env.native();
        env.ok(&["init"]);
        let err = run(
            &ctx(&env),
            &Job::Approve(vec!["nope".into()]),
            80,
            Box::new(recorder()),
        )
        .unwrap_err();
        let (code, _, stderr) = env.code(&["approve", "nope"]);
        assert_eq!(err.code, code);
        assert!(
            stderr.contains(err.message.lines().next().unwrap()),
            "{stderr}"
        );
    }

    #[test]
    fn ac8_02_jobs_name_their_command() {
        assert_eq!(Job::Sync(None).command(), "colctl sync");
        assert_eq!(
            Job::Sync(Some("codex".into())).command(),
            "colctl sync --harness codex"
        );
        assert_eq!(Job::Update(vec![]).command(), "colctl update");
        assert_eq!(Job::Update(vec!["a".into()]).command(), "colctl update a");
        assert_eq!(Job::Approve(vec![]).command(), "colctl approve --all");
        assert_eq!(Job::Reject(vec!["a".into()]).command(), "colctl reject a");
    }
}
