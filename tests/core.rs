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
