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
        // The watcher shows the source's name; the summary doesn't repeat it.
        for e in events(&rec) {
            if let Event::Done {
                profile, summary, ..
            } = e
            {
                assert!(!summary.starts_with(&format!("{profile}:")), "{summary}");
            }
        }
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

mod review_and_discovery {
    use super::*;
    use colctl::core::discover;
    use colctl::core::observer::Recorder;
    use colctl::core::quarantine;
    use colctl::core::run::{Job, run};
    use serde_json::{Value, json};
    use std::path::Path;

    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value.to_string()).unwrap();
    }

    /// A Claude profile that knows one marketplace with one plugin, "tool".
    fn catalog_fixture() -> Env {
        let env = Env::new();
        let native = env.native();
        env.ok(&["init"]);
        let market = env.home.parent().unwrap().join("markets/official");
        write(
            &market.join(".claude-plugin/marketplace.json"),
            &json!({"name": "official", "plugins": [
                {"name": "tool", "description": "A tool", "author": {"name": "Ann"},
                 "source": {"source": "github", "repo": "ann/tool"}, "keywords": ["x"]},
                {"name": "local", "description": "A local one", "source": "./local"}]}),
        );
        write(
            &native.join("plugins/known_marketplaces.json"),
            &json!({"official": {"source": {"source": "github", "repo": "o/official"},
                                  "installLocation": market.display().to_string()}}),
        );
        env
    }

    fn stdout_lines(env: &Env, args: &[&str]) -> Vec<String> {
        let mut all = args.to_vec();
        all.extend(["--color", "never", "--no-pager"]);
        let (_, out, _) = env.code(&all);
        out.lines().map(String::from).collect()
    }

    #[test]
    fn ac8_04_held_lists_what_review_lists() {
        let env = held_fixture();
        let held = quarantine::held(&ctx(&env)).unwrap();
        assert_eq!(held.len(), 1);
        let h = &held[0];
        assert_eq!(
            (h.name.as_str(), h.kind.as_str()),
            ("superpowers", "source")
        );
        assert!(h.change.contains(" → "), "{h:?}");
        assert_eq!(h.size, "1 commit");
        assert_eq!(h.error, None);
        let list = stdout_lines(&env, &["review"]).join("\n");
        assert!(list.contains("1 commit"), "{list}");
    }

    #[test]
    fn ac8_04_review_detail_is_the_clis() {
        let env = held_fixture();
        let ours = quarantine::review(&ctx(&env), "superpowers", 80).unwrap();
        assert_eq!(ours, stdout_lines(&env, &["review", "superpowers"]));
        assert!(quarantine::review(&ctx(&env), "nope", 80).is_err());
    }

    #[test]
    fn ac8_04_catalog_and_show_are_the_clis() {
        let env = catalog_fixture();
        let c = discover::catalog(&ctx(&env)).unwrap();
        let names: Vec<&str> = c.plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"tool") && names.contains(&"local"),
            "{names:?}"
        );
        assert_eq!(c.catalogs, 1);
        assert!(c.warnings.is_empty());
        let ours = discover::show(&ctx(&env), "tool", 80).unwrap();
        assert_eq!(ours, stdout_lines(&env, &["show", "tool"]));
    }

    #[test]
    fn ac8_04_add_runs_what_show_says() {
        let env = catalog_fixture();
        let c = discover::catalog(&ctx(&env)).unwrap();
        let tool = c.plugins.iter().find(|p| p.name == "tool").unwrap();
        let words = discover::add_words(tool).unwrap();
        assert_eq!(words[..2], ["add".to_string(), "source".to_string()]);
        // Running a source add would clone from GitHub; a plugin add runs on the fake claude.
        let market = env.home.parent().unwrap().join("markets/official");
        let words_local: Vec<String> = vec![
            "add".into(),
            "plugin".into(),
            "local@official".into(),
            "--marketplace".into(),
            market.display().to_string(),
        ];
        let job = Job::Add(words_local.clone());
        assert_eq!(job.command(), format!("colctl {}", words_local.join(" ")));
        let report = run(&ctx(&env), &job, 80, Box::new(Recorder::default())).unwrap();
        assert_eq!(report.exit, 0, "{report:?}");
        assert!(env.config().contains("local@official"), "{}", env.config());
    }

    #[test]
    fn ac8_05_colctl_search_matches_letters_in_order() {
        let env = catalog_fixture();
        // "tol" is in order in "tool"; nothing has it as written.
        let out = stdout_lines(&env, &["search", "tol"]).join("\n");
        assert!(out.contains("tool@official"), "{out}");
        assert!(!out.contains("local@official"), "{out}");
        let c = discover::catalog(&ctx(&env)).unwrap();
        let q = colctl::search::Query {
            terms: vec!["tol".into()],
            ..Default::default()
        };
        let (hits, matched) = discover::rank(&c.plugins, &q);
        assert_eq!((hits[0].name.as_str(), matched), ("tool", 1));
    }

    #[test]
    fn ac8_04_nothing_to_add_says_why() {
        let mut p = discover::catalog(&ctx(&catalog_fixture())).unwrap().plugins[0].clone();
        p.add = "already source plugin tool".into();
        assert_eq!(
            discover::add_words(&p).unwrap_err(),
            "already source plugin tool"
        );
    }

    #[test]
    fn ac8_04_only_add_commands_run_as_add() {
        let env = catalog_fixture();
        let err = run(
            &ctx(&env),
            &Job::Add(vec!["rm".into(), "plugin".into(), "x".into()]),
            80,
            Box::new(Recorder::default()),
        )
        .unwrap_err();
        assert!(err.message.contains("isn't an add command"), "{err:?}");
        let err = run(
            &ctx(&env),
            &Job::Add(vec!["add".into(), "--nope".into()]),
            80,
            Box::new(Recorder::default()),
        )
        .unwrap_err();
        assert_eq!(err.code, 2, "{err:?}");
    }
}

#[test]
fn ac8_03_the_cli_prints_what_the_core_reports() {
    use colctl::core::observer::Quiet;
    use colctl::core::run::{Job, run};
    let make = || {
        let env = Env::new();
        env.native();
        env.ok(&["init"]);
        env.source_skill("alpha");
        env
    };
    let (a, b) = (make(), make());
    let report = run(&ctx(&a), &Job::Sync(None), 80, Box::new(Quiet)).unwrap();
    let (code, out, _) = b.code(&["sync", "--color", "never"]);
    assert_eq!(report.exit, code);
    let mask =
        |s: &str, env: &Env| s.replace(&env.home.parent().unwrap().display().to_string(), "[ROOT]");
    let cli: Vec<String> = out.lines().map(|l| mask(l, &b)).collect();
    let ours: Vec<String> = report.lines.iter().map(|l| mask(l, &a)).collect();
    assert_eq!(ours, cli);
}

#[test]
fn ac8_15_a_run_with_two_verdicts_reports_both_worst_first() {
    use colctl::core::run::Report;
    let r = Report {
        exit: 1,
        lines: vec![
            "✓ Approved 1 held version".into(),
            String::new(),
            "  ✗  claude       1 step failed".into(),
            "      ✗ update qdrant failed: …".into(),
            String::new(),
            "✗ Synced 3 of 5 profiles · 2 failed".into(),
        ],
        errors: Vec::new(),
    };
    assert_eq!(
        r.verdict(),
        "✗ Synced 3 of 5 profiles · 2 failed · ✓ Approved 1 held version"
    );
    let one = Report {
        exit: 0,
        lines: vec!["✓ All 5 profiles in sync".into(), "  ✓ claude".into()],
        errors: Vec::new(),
    };
    assert_eq!(one.verdict(), "✓ All 5 profiles in sync");
    let none = Report {
        exit: 0,
        lines: vec!["plain".into()],
        errors: Vec::new(),
    };
    assert_eq!(none.verdict(), "plain");
}

mod same_version {
    use super::*;

    /// One Claude profile with a source plugin whose upstream moved on without bumping its version.
    fn fixture() -> (Env, String) {
        let env = Env::new();
        env.native();
        env.ok(&["init"]);
        let (repo, url, _) = env.upstream("superpowers", true, false, Some("superpowers-dev"));
        env.ok(&["add", "source", &url]);
        let new = env.upstream_commit(&repo, "v2");
        env.ok(&["update"]);
        (env, new)
    }

    fn installed_sha(env: &Env) -> String {
        let v: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(env.home.join(".claude/plugins/installed_plugins.json")).unwrap(),
        )
        .unwrap();
        v["plugins"]["superpowers@superpowers-dev"][0]["gitCommitSha"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn ac8_40_a_new_commit_at_the_same_version_still_installs() {
        let (env, new) = fixture();
        let (code, out, err) = env.code(&["approve", "superpowers", "--color", "never"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(installed_sha(&env), new);
        let calls = env.log_args().join("\n");
        assert!(
            calls.contains("plugin update superpowers@superpowers-dev"),
            "{calls}"
        );
        assert!(
            calls.contains("plugin uninstall superpowers@superpowers-dev"),
            "{calls}"
        );
        assert_eq!(env.code(&["status"]).0, 0);
    }

    #[test]
    fn ec8_28_a_reinstall_that_fails_is_retried_by_the_next_sync() {
        let (env, new) = fixture();
        let mut env = env.var("FAKE_CLAUDE_FAIL", "plugin install");
        let (code, out, _) = env.code(&["approve", "superpowers", "--color", "never"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("plugin install"), "{out}");
        env.vars.clear();
        env.ok(&["sync"]);
        assert_eq!(installed_sha(&env), new);
    }
}
