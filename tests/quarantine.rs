//! Cycle 4: quarantine. `update` and `refresh` hold new versions; `review`, `approve` and `reject`
//! decide them. Upstreams are local `file://` repos and origins are folders in the temp home.
//! Named after AC4-NN / EC4-NN.

mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.agents_dir();
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

// ---------------------------------------------------------------- refresh holds (T4)

fn hash(path: &Path) -> String {
    if path.is_dir() {
        colctl::fsx::hash_tree(path).unwrap()
    } else {
        colctl::fsx::hash_file(path).unwrap()
    }
}

/// A skill twg keeps in its own folder and links into ~/.agents/skills, taken over by colctl.
fn adopted_skill(env: &Env, name: &str, body: &str) -> PathBuf {
    let origin = env.skill_dir(&env.home.join(".config/twg/skills"), name, body);
    let skills = env.home.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink(&origin, skills.join(name)).unwrap();
    env.ok(&["adopt", "agents", &format!("skills/{name}")]);
    origin
}

/// An agent file another tool keeps elsewhere and links into ~/.claude/agents, taken over.
fn adopted_agent(env: &Env, name: &str, body: &str) -> PathBuf {
    let dir = env.home.join(".config/tool/agents");
    fs::create_dir_all(&dir).unwrap();
    let origin = dir.join(format!("{name}.md"));
    fs::write(
        &origin,
        format!("---\nname: {name}\ndescription: d\n---\n{body}\n"),
    )
    .unwrap();
    let agents = env.home.join(".claude/agents");
    fs::create_dir_all(&agents).unwrap();
    symlink(&origin, agents.join(format!("{name}.md"))).unwrap();
    env.ok(&["adopt", "claude", &format!("agents/{name}.md")]);
    origin
}

fn source_text(env: &Env, rel: &str) -> String {
    fs::read_to_string(env.source.join(rel)).unwrap()
}

#[test]
fn ac4_02_held_content_is_never_committed() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    let commits = env.commits();
    env.ok(&["refresh"]);
    let snap = env.source.join(".state/quarantine/skills/twg-jira");
    assert_eq!(fs::read_to_string(snap.join("SKILL.md")).unwrap(), "v2");
    assert_eq!(env.git(&["status", "--porcelain"]), "");
    assert_eq!(env.commits(), commits);
}

#[test]
fn ac4_17_refresh_holds_instead_of_copying() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    let agent = adopted_agent(&env, "rev", "v1");
    let base_skill = hash(&env.source.join("skills/twg-jira"));
    let base_agent = hash(&env.source.join("agents/rev.md"));
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    fs::write(&agent, "---\nname: rev\ndescription: d\n---\nv2\n").unwrap();
    let commits = env.commits();
    let calls = (env.log().len(), env.codex_log().len());
    let out = env.ok(&["refresh"]);
    assert!(
        out.contains("held skills/twg-jira for review; colctl review skills/twg-jira"),
        "{out}"
    );
    assert!(out.contains("held agents/rev.md for review"), "{out}");
    assert!(source_text(&env, "skills/twg-jira/SKILL.md").ends_with("v1"));
    assert!(source_text(&env, "agents/rev.md").contains("v1"));
    assert_eq!(env.commits(), commits);
    assert_eq!((env.log().len(), env.codex_log().len()), calls, "no sync");
    let st = state(&env);
    assert_eq!(st["origins"]["skills/twg-jira"]["hash"], base_skill);
    let held = &st["quarantine"]["skills/twg-jira"];
    assert_eq!(held["kind"], "skill");
    assert_eq!(held["origin"], origin.display().to_string());
    assert_eq!(held["base"], base_skill);
    assert_eq!(held["hash"], hash(&origin));
    let held = &st["quarantine"]["agents/rev.md"];
    assert_eq!(held["kind"], "agent");
    assert_eq!(held["base"], base_agent);
    assert_eq!(held["hash"], hash(&agent));
    assert_eq!(
        hash(&env.source.join(".state/quarantine/agents/rev.md")),
        hash(&agent)
    );
}

#[test]
fn ac4_18_refusals_stay_and_held_items_are_known() {
    let env = machine();
    let both = adopted_skill(&env, "both", "v1");
    let gone = adopted_skill(&env, "gone", "v1");
    let held = adopted_skill(&env, "held", "v1");
    fs::write(held.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh", "skills/held"]);
    fs::write(both.join("SKILL.md"), "theirs").unwrap();
    fs::write(env.source.join("skills/both/SKILL.md"), "mine").unwrap();
    fs::remove_dir_all(&gone).unwrap();
    let out = env.expect(1, &["refresh"]);
    assert!(
        out.contains("refused skills/both: changed in both places"),
        "{out}"
    );
    assert!(out.contains("refused skills/gone: origin is gone"), "{out}");
    assert!(out.contains("skills/held already held for review"), "{out}");
    // The origin changes again: a new snapshot replaces the old one.
    fs::write(held.join("SKILL.md"), "v3").unwrap();
    let out = env.ok(&["refresh", "skills/held"]);
    assert!(out.contains("(replaces the copy held before)"), "{out}");
    assert_eq!(
        fs::read_to_string(env.source.join(".state/quarantine/skills/held/SKILL.md")).unwrap(),
        "v3"
    );
    assert_eq!(
        state(&env)["quarantine"]["skills/held"]["hash"],
        hash(&held)
    );
}

#[test]
fn ec4_10_an_origin_back_at_its_adopted_content_keeps_the_held_copy() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    let adopted = fs::read_to_string(origin.join("SKILL.md")).unwrap();
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    fs::write(origin.join("SKILL.md"), adopted).unwrap();
    let out = env.ok(&["refresh"]);
    assert!(out.contains("skills/twg-jira is up to date with"), "{out}");
    assert!(
        env.source
            .join(".state/quarantine/skills/twg-jira")
            .is_dir()
    );
    assert_eq!(
        state(&env)["quarantine"]["skills/twg-jira"]["kind"],
        "skill"
    );
}

#[test]
fn ec4_21_a_symlink_leaving_a_held_skill_stays_a_symlink() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    let secret = env.home.join("secret.txt");
    fs::write(&secret, "do not copy").unwrap();
    symlink(&secret, origin.join("leak")).unwrap();
    env.ok(&["refresh"]);
    let snap = env.source.join(".state/quarantine/skills/twg-jira/leak");
    assert_eq!(fs::read_link(&snap).unwrap(), secret);
    assert!(
        fs::symlink_metadata(&snap)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn refresh_dry_run_holds_nothing() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    let out = env.ok(&["refresh", "--dry-run"]);
    assert!(out.contains("would hold skills/twg-jira from"), "{out}");
    assert_eq!(state(&env)["quarantine"], Value::Null);
}
