//! Cycle 4: quarantine. `update` and `refresh` hold new versions; `review`, `approve` and `reject`
//! decide them. Upstreams are local `file://` repos and origins are folders in the temp home.
//! Named after AC4-NN / EC4-NN.

mod support;

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.ok(&["init"]);
    env
}

fn state(env: &Env) -> Value {
    serde_json::from_str(
        &fs::read_to_string(env.source.join(".state/state.json")).unwrap_or("{}".into()),
    )
    .unwrap()
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn head_of(dir: &Path) -> String {
    git_in(dir, &["rev-parse", "HEAD"])
}

fn pin(env: &Env) -> String {
    let t: toml::Table = toml::from_str(&env.config()).unwrap();
    t["source"][0]["rev"].as_str().unwrap().to_string()
}

fn clone(env: &Env) -> PathBuf {
    env.source.join("plugins/superpowers")
}

fn held_ref(env: &Env) -> String {
    git_in(
        &clone(env),
        &["rev-parse", "-q", "--verify", "refs/colctl/candidate"],
    )
}

/// A machine with source plugin superpowers added at its upstream head; returns (env, repo, v1).
fn with_source() -> (Env, PathBuf, String) {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    (env, repo, v1)
}

/// Everything update must leave alone: config, history, clone HEAD, CLI calls, installed revs.
fn untouched(env: &Env) -> (String, Vec<String>, String, usize, usize, Value) {
    (
        env.config(),
        env.commits(),
        head_of(&clone(env)),
        env.log().len(),
        env.codex_log().len(),
        state(env)["harnesses"].clone(),
    )
}

// ---------------------------------------------------------------- update holds (T3)

#[test]
fn ac4_11_update_holds_instead_of_installing() {
    let (env, repo, v1) = with_source();
    env.upstream_commit(&repo, "v2");
    let v3 = env.upstream_commit(&repo, "v3");
    let before = untouched(&env);
    let out = env.ok(&["update"]);
    assert!(
        out.contains(&format!(
            "superpowers: {} → {} (2 new commits), held for review; colctl review superpowers",
            &v1[..7],
            &v3[..7]
        )),
        "{out}"
    );
    assert_eq!(untouched(&env), before);
    assert_eq!(pin(&env), v1);
    let held = &state(&env)["quarantine"]["sources/superpowers"];
    assert_eq!(held["kind"], "source");
    assert_eq!(held["from"], v1.as_str());
    assert_eq!(held["to"], v3.as_str());
    assert!(held["held_at"].as_u64().unwrap() > 0);
    assert_eq!(held_ref(&env), v3);
}

#[test]
fn ac4_12_nothing_newer_holds_nothing() {
    let (env, _, _) = with_source();
    let out = env.ok(&["update"]);
    assert!(out.contains("superpowers: up to date"), "{out}");
    assert_eq!(state(&env)["quarantine"], Value::Null);
    assert_eq!(held_ref(&env), "");
}

#[test]
fn ac4_13_a_newer_commit_replaces_the_held_one() {
    let (env, repo, _) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    let v3 = env.upstream_commit(&repo, "v3");
    let out = env.ok(&["update"]);
    assert!(out.contains(&format!("(replaces {})", &v2[..7])), "{out}");
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v3.as_str()
    );
    assert_eq!(held_ref(&env), v3);
    let out = env.ok(&["update"]);
    assert!(
        out.contains(&format!(
            "superpowers: {} already held for review",
            &v3[..7]
        )),
        "{out}"
    );
}

#[test]
fn ac4_14_update_rev_holds_the_named_commit() {
    let (env, repo, v1) = with_source();
    git_in(&repo, &["tag", "v1.0", &v1]);
    let v2 = env.upstream_commit(&repo, "v2");
    git_in(&repo, &["tag", "v2.0", &v2]);
    env.upstream_commit(&repo, "v3");
    env.ok(&["update", "superpowers", "--rev", "v2.0"]);
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v2.as_str()
    );
    assert_eq!(pin(&env), v1);
}

#[test]
fn ac4_15_update_check_says_what_is_held_or_rejected() {
    let env = machine();
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    let (repo2, url2, _) = env.upstream("other", true, true, Some("other-m"));
    env.ok(&["add", "source", &url]);
    env.ok(&["add", "source", &url2]);
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update", "superpowers"]);
    let r2 = env.upstream_commit(&repo2, "r2");
    reject_by_hand(&env, "sources/other", &r2);
    let before = state(&env);
    let out = env.expect(1, &["update", "--check"]);
    assert!(
        out.contains(&format!("→ {} (1 new commit) (held for review)", &v2[..7])),
        "{out}"
    );
    assert!(
        out.contains(&format!("→ {} (1 new commit) (rejected)", &r2[..7])),
        "{out}"
    );
    assert_eq!(state(&env), before, "--check holds nothing");
    // Only the rejected one is newer: exit 0.
    env.ok(&["update", "--check", "other"]);
}

#[test]
fn ac4_16_sync_never_installs_a_candidate() {
    let (env, repo, v1) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    env.ok(&["sync"]);
    assert_eq!(head_of(&clone(&env)), v1);
    for h in ["claude", "kilo-claude", "codex"] {
        assert_eq!(
            state(&env)["harnesses"][h]["sources"]["superpowers"]["rev"],
            v1.as_str(),
            "{h}"
        );
    }
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v2.as_str()
    );
}

#[test]
fn ec4_01_update_with_no_sources() {
    let env = machine();
    let out = env.ok(&["update"]);
    assert!(out.contains("no sources declared"), "{out}");
}

#[test]
fn ec4_05_update_rev_at_the_pin_is_up_to_date() {
    let (env, repo, v1) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    let out = env.ok(&["update", "superpowers", "--rev", &v1]);
    assert!(out.contains("superpowers: up to date"), "{out}");
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v2.as_str(),
        "the held candidate stays"
    );
}

#[test]
fn ec4_06_upstream_going_back_to_the_pin_keeps_the_candidate() {
    let (env, repo, v1) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    git_in(&repo, &["reset", "-q", "--hard", &v1]);
    let out = env.ok(&["update"]);
    assert!(out.contains("superpowers: up to date"), "{out}");
    assert_eq!(held_ref(&env), v2);
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v2.as_str()
    );
}

/// Records `version` as rejected for `key` the way `reject` will (T7), so T3 can test that
/// `update` honours it.
fn reject_by_hand(env: &Env, key: &str, version: &str) {
    let mut st = state(env);
    st["rejected"][key] = serde_json::json!([version]);
    let obj = st.as_object_mut().unwrap();
    if let Some(q) = obj.get_mut("quarantine").and_then(Value::as_object_mut) {
        q.remove(key);
        if q.is_empty() {
            obj.remove("quarantine");
        }
    }
    fs::write(env.source.join(".state/state.json"), st.to_string()).unwrap();
}

#[test]
fn ec4_07_a_rejected_head_holds_nothing() {
    let (env, repo, _) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    reject_by_hand(&env, "sources/superpowers", &v2);
    let out = env.ok(&["update"]);
    assert!(
        out.contains(&format!(
            "superpowers: {} was rejected; nothing held",
            &v2[..7]
        )),
        "{out}"
    );
    assert_eq!(state(&env)["quarantine"], Value::Null);
    env.ok(&["update", "--check"]);
}

#[test]
fn ec4_08_update_rev_to_an_older_commit_goes_back() {
    let (env, repo, v1) = with_source();
    env.upstream_commit(&repo, "v2");
    env.upstream_commit(&repo, "v3");
    // Pin the newest commit, then hold the oldest.
    let config = env.config().replace(&v1, &head_of(&repo));
    env.write_config(&config);
    env.ok(&["sync"]);
    let out = env.ok(&["update", "superpowers", "--rev", &v1]);
    assert!(out.contains("(goes back 2 commits)"), "{out}");
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v1.as_str()
    );
}

#[test]
fn ec4_17_an_unreachable_upstream_holds_nothing_and_others_continue() {
    let env = machine();
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    let (repo2, url2, _) = env.upstream("other", true, true, Some("other-m"));
    env.ok(&["add", "source", &url]);
    env.ok(&["add", "source", &url2]);
    fs::remove_dir_all(&repo).unwrap();
    let o2 = env.upstream_commit(&repo2, "o2");
    let out = env.expect(1, &["update"]);
    assert!(
        out.contains("superpowers:") && out.contains("fetch"),
        "{out}"
    );
    let q = &state(&env)["quarantine"];
    assert_eq!(q["sources/superpowers"], Value::Null);
    assert_eq!(q["sources/other"]["to"], o2.as_str());
}

#[test]
fn ec4_20_update_waits_for_the_lock_but_check_does_not() {
    let (env, repo, _) = with_source();
    env.upstream_commit(&repo, "v2");
    fs::write(
        env.source.join(".state/lock"),
        std::process::id().to_string(),
    )
    .unwrap();
    let (code, _, err) = env.code(&["update"]);
    assert_eq!(code, 3, "{err}");
    assert!(err.contains("another colctl is running"), "{err}");
    assert_eq!(env.code(&["update", "--check"]).0, 1);
    assert_eq!(env.code(&["update", "--dry-run"]).0, 0);
    assert_eq!(state(&env)["quarantine"], Value::Null);
    fs::remove_file(env.source.join(".state/lock")).unwrap();
}

#[test]
fn update_dry_run_holds_nothing() {
    let (env, repo, _) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    let out = env.ok(&["update", "--dry-run"]);
    assert!(out.contains("would hold superpowers:"), "{out}");
    assert!(out.contains(&v2[..7]), "{out}");
    assert_eq!(state(&env)["quarantine"], Value::Null);
    assert_eq!(held_ref(&env), "");
}
