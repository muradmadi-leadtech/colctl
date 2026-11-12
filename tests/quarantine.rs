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

// ---------------------------------------------------------------- review (T5)

/// Commits files to an upstream repo: (path, content, executable). Returns the new head.
fn commit_files(repo: &Path, files: &[(&str, &str, bool)], message: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    for (path, content, exec) in files {
        let p = repo.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, content).unwrap();
        let mode = if *exec { 0o755 } else { 0o644 };
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
    }
    let g = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["-c", "user.name=Ann Author", "-c", "user.email=a@a"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    g(&["add", "-A"]);
    g(&["commit", "-qm", message]);
    head_of(repo)
}

/// superpowers held at a commit that adds a hook, an executable script and changes the manifest.
fn risky_candidate() -> (Env, PathBuf, String, String) {
    let (env, repo, v1) = with_source();
    let v2 = commit_files(
        &repo,
        &[
            ("hooks/hooks.json", "{\"SessionStart\": []}", false),
            ("scripts/session-start.sh", "#!/bin/sh\necho hi\n", true),
            (
                ".claude-plugin/plugin.json",
                "{\"name\": \"superpowers\", \"version\": \"2\"}",
                false,
            ),
        ],
        "Add a session hook",
    );
    env.ok(&["update"]);
    (env, repo, v1, v2)
}

#[test]
fn ac4_19_review_lists_what_is_held() {
    let env = machine();
    let out = env.ok(&["review"]);
    assert!(out.contains("nothing held for review"), "{out}");
    let (env, _, v1, v2) = risky_candidate();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    fs::write(origin.join("run.sh"), "echo").unwrap();
    env.ok(&["refresh"]);
    let out = env.expect(1, &["review"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].starts_with("skills/twg-jira"), "{out}");
    assert!(lines[0].contains("~/.config/twg/skills/twg-jira"), "{out}");
    assert!(
        lines[0].contains("2 files") && lines[0].contains("1 flag"),
        "{out}"
    );
    assert!(lines[1].starts_with("sources/superpowers"), "{out}");
    assert!(
        lines[1].contains(&format!("{} → {}", &v1[..7], &v2[..7]))
            && lines[1].contains("1 commit")
            && lines[1].contains("4 flags"),
        "{out}"
    );
}

#[test]
fn ac4_20_reviewing_a_source_plugin() {
    let (env, _, v1, v2) = risky_candidate();
    let before = env.snapshot();
    let out = env.ok(&["review", "superpowers"]);
    for want in [
        format!("superpowers  {} → {}", &v1[..7], &v2[..7]),
        "commits (1)".into(),
        format!("  {} Add a session hook", &v2[..7]),
        "files (3)".into(),
        "  A  hooks/hooks.json".into(),
        "  A  scripts/session-start.sh    (+x)".into(),
        "  M  .claude-plugin/plugin.json".into(),
        "flags (4)".into(),
        "adds hooks".into(),
        "new executable file".into(),
        "new script".into(),
        "plugin manifest changed".into(),
        "diff".into(),
        "+echo hi".into(),
    ] {
        assert!(out.contains(&want), "{want:?} in:\n{out}");
    }
    assert_eq!(env.snapshot(), before, "review writes nothing");
}

#[test]
fn ac4_21_reviewing_a_skill() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(
        origin.join("SKILL.md"),
        "---\nname: twg-jira\nallowed-tools: Bash\n---\nv2\n",
    )
    .unwrap();
    env.ok(&["refresh"]);
    let out = env.ok(&["review", "skills/twg-jira"]);
    assert!(
        out.contains("skills/twg-jira  from ~/.config/twg/skills/twg-jira"),
        "{out}"
    );
    assert!(
        out.contains("files (1)") && out.contains("  M  SKILL.md"),
        "{out}"
    );
    assert!(
        out.contains("allowed-tools") && out.contains("grants Bash: Bash"),
        "{out}"
    );
    assert!(out.contains("+allowed-tools: Bash"), "{out}");
    assert!(!out.contains("commits ("), "{out}");
}

#[test]
fn ac4_22_review_json() {
    let (env, _, v1, v2) = risky_candidate();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    let (code, out, _) = env.code(&["review", "--json"]);
    assert_eq!(code, 1);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], 1);
    let list = v["candidates"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    let src = list.iter().find(|c| c["kind"] == "source").unwrap();
    assert_eq!(src["key"], "sources/superpowers");
    assert_eq!(src["name"], "superpowers");
    assert_eq!(src["from"], v1.as_str());
    assert_eq!(src["to"], v2.as_str());
    assert_eq!(src["direction"], "forward");
    assert_eq!(src["commits"][0]["subject"], "Add a session hook");
    assert_eq!(src["commits"][0]["author"], "Ann Author");
    assert_eq!(src["files"].as_array().unwrap().len(), 3);
    assert_eq!(src["flags"].as_array().unwrap().len(), 4);
    assert_eq!(src["error"], Value::Null);
    assert!(src.get("diff").is_none(), "no diff in the list");
    let skill = list.iter().find(|c| c["kind"] == "skill").unwrap();
    assert_eq!(skill["name"], "twg-jira");
    assert_eq!(skill["origin"], origin.display().to_string());
    let one: Value = serde_json::from_str(&env.ok(&["review", "superpowers", "--json"])).unwrap();
    assert!(
        one["candidates"][0]["diff"]
            .as_str()
            .unwrap()
            .contains("+echo hi")
    );
}

#[test]
fn ec4_02_ec4_03_review_names() {
    let (env, _, _) = with_source();
    let (code, _, err) = env.code(&["review", "superpowers"]);
    assert_eq!(code, 2);
    assert!(err.contains("nothing held for superpowers"), "{err}");
    let (code, _, err) = env.code(&["review", "nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("no source named \"nope\""), "{err}");
    let (code, _, err) = env.code(&["review", "skills/nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("no taken-over item"), "{err}");
}

#[test]
fn ec4_08_review_of_an_older_commit_goes_back() {
    let (env, repo, v1) = with_source();
    env.upstream_commit(&repo, "v2");
    let v3 = env.upstream_commit(&repo, "v3");
    env.write_config(&env.config().replace(&v1, &v3));
    env.ok(&["sync"]);
    env.ok(&["update", "superpowers", "--rev", &v1]);
    let out = env.ok(&["review", "superpowers"]);
    assert!(out.contains("goes back 2 commits (removed)"), "{out}");
    assert!(out.contains("-v3"), "{out}");
    let list = env.expect(1, &["review"]);
    assert!(list.contains("goes back 2 commits"), "{list}");
}

#[test]
fn ec4_09_review_covers_the_whole_repo() {
    let env = machine();
    let (repo, url, _) = env.upstream("tools", true, false, Some("tools-m"));
    // A second plugin in the same repo; the source picks "tools" but review shows everything.
    env.ok(&["add", "source", &url]);
    commit_files(&repo, &[("other-plugin/run.sh", "echo", false)], "other");
    env.ok(&["update"]);
    let out = env.ok(&["review", "tools"]);
    assert!(out.contains("  A  other-plugin/run.sh"), "{out}");
    assert!(out.contains("new script"), "{out}");
}

#[test]
fn ec4_12_a_tampered_snapshot_is_refused() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    fs::write(
        env.source
            .join(".state/quarantine/skills/twg-jira/SKILL.md"),
        "evil",
    )
    .unwrap();
    let out = env.expect(1, &["review", "skills/twg-jira"]);
    assert!(
        out.contains("the held copy of skills/twg-jira changed since it was held; reject it and refresh again"),
        "{out}"
    );
    let list = env.expect(1, &["review"]);
    assert!(list.contains("error: the held copy"), "{list}");
}

#[test]
fn ec4_13_a_missing_held_commit_is_refused() {
    let (env, repo, _) = with_source();
    env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    git_in(&clone(&env), &["update-ref", "-d", "refs/colctl/candidate"]);
    let out = env.expect(1, &["review", "superpowers"]);
    assert!(
        out.contains("the held commit for superpowers is missing; run colctl update superpowers"),
        "{out}"
    );
    let v: Value = serde_json::from_str(&env.code(&["review", "--json"]).1).unwrap();
    assert!(
        v["candidates"][0]["error"]
            .as_str()
            .unwrap()
            .contains("missing")
    );
}

#[test]
fn ec4_14_review_warns_when_the_pin_changed_by_hand() {
    let (env, repo, v1) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.upstream_commit(&repo, "v3");
    env.ok(&["update", "superpowers", "--rev", &v2]);
    let v3 = head_of(&repo);
    env.write_config(&env.config().replace(&v1, &v3));
    let out = env.ok(&["review", "superpowers"]);
    assert!(
        out.contains("warning: superpowers's pin changed since it was held"),
        "{out}"
    );
}

#[test]
fn ec4_22_review_escapes_control_characters() {
    let (env, repo, _) = with_source();
    commit_files(
        &repo,
        &[("evil\u{1b}[2Jname", "x", false)],
        "subject \u{1b}]0;pwned\u{07}",
    );
    env.ok(&["update"]);
    let out = env.ok(&["review", "superpowers"]);
    assert!(
        !out.contains('\u{1b}') && !out.contains('\u{07}'),
        "{out:?}"
    );
    assert!(out.contains("subject \\x1b]0;pwned\\x07"), "{out}");
    let v: Value = serde_json::from_str(&env.ok(&["review", "superpowers", "--json"])).unwrap();
    assert_eq!(
        v["candidates"][0]["commits"][0]["subject"],
        "subject \u{1b}]0;pwned\u{07}"
    );
}

// ---------------------------------------------------------------- approve (T6)

fn installed_rev(env: &Env, harness: &str) -> Value {
    state(env)["harnesses"][harness]["sources"]["superpowers"]["rev"].clone()
}

#[test]
fn ac4_23_approving_a_source_plugin() {
    let (env, _, _, v2) = risky_candidate();
    let out = env.ok(&["approve", "superpowers"]);
    assert!(out.contains("approved superpowers:"), "{out}");
    assert_eq!(pin(&env), v2);
    assert!(env.commits().contains(&format!(
        "colctl: approve source superpowers at {}",
        &v2[..7]
    )));
    assert_eq!(held_ref(&env), "");
    assert_eq!(state(&env)["quarantine"], Value::Null);
    assert_eq!(head_of(&clone(&env)), v2);
    for h in ["claude", "kilo-claude", "codex"] {
        assert_eq!(installed_rev(&env, h), v2.as_str(), "{h}");
    }
    assert!(
        env.log_args()
            .contains(&"plugin update superpowers@superpowers-dev".to_string())
    );
    env.ok(&["status"]);
}

#[test]
fn ac4_24_approving_a_skill_and_an_agent() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    let agent = adopted_agent(&env, "rev", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    fs::write(&agent, "---\nname: rev\ndescription: d\n---\nv2\n").unwrap();
    env.ok(&["refresh"]);
    let out = env.ok(&["approve", "skills/twg-jira", "agents/rev.md"]);
    assert!(
        out.contains("approved skills/twg-jira from ~/.config/twg/skills/twg-jira"),
        "{out}"
    );
    assert_eq!(source_text(&env, "skills/twg-jira/SKILL.md"), "v2");
    assert!(source_text(&env, "agents/rev.md").contains("v2"));
    let st = state(&env);
    assert_eq!(st["origins"]["skills/twg-jira"]["hash"], hash(&origin));
    assert_eq!(st["origins"]["agents/rev.md"]["hash"], hash(&agent));
    assert_eq!(st["quarantine"], Value::Null);
    let commits = env.commits();
    assert!(
        commits.contains(&"colctl: approve skill twg-jira".to_string()),
        "{commits:?}"
    );
    assert!(
        commits.contains(&"colctl: approve agent rev".to_string()),
        "{commits:?}"
    );
    assert!(
        !env.source
            .join(".state/quarantine/skills/twg-jira")
            .exists()
    );
    assert!(!env.source.join(".state/quarantine/agents/rev.md").exists());
    assert_eq!(env.git(&["status", "--porcelain"]), "");
    // Codex agent TOML follows the approved content (sync ran).
    let toml = fs::read_to_string(env.home.join(".codex/agents/rev.toml")).unwrap();
    assert!(toml.contains("v2"), "{toml}");
    env.ok(&["status"]);
}

#[test]
fn ac4_25_what_is_approved_is_what_was_reviewed() {
    let (env, repo, _, v2) = risky_candidate();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "reviewed").unwrap();
    env.ok(&["refresh"]);
    env.upstream_commit(&repo, "v3");
    fs::write(origin.join("SKILL.md"), "newer, unreviewed").unwrap();
    // The origin moved on after review, so sync reports it as origin-changed (exit 1).
    let out = env.expect(1, &["approve", "--all"]);
    assert!(out.contains("origin-changed skills/twg-jira"), "{out}");
    assert_eq!(pin(&env), v2);
    assert_eq!(source_text(&env, "skills/twg-jira/SKILL.md"), "reviewed");
}

#[test]
fn ac4_26_several_at_once_all_and_dry_run() {
    let (env, _, v1, v2) = risky_candidate();
    let one = adopted_skill(&env, "one", "v1");
    let two = adopted_skill(&env, "two", "v1");
    fs::write(one.join("SKILL.md"), "v2").unwrap();
    fs::write(two.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    let before = env.snapshot();
    let out = env.ok(&["approve", "--all", "--dry-run"]);
    assert!(
        out.contains(&format!(
            "would approve sources/superpowers: {} → {}",
            &v1[..7],
            &v2[..7]
        )),
        "{out}"
    );
    assert!(
        out.contains("would approve skills/one: ~/.config/twg/skills/one"),
        "{out}"
    );
    assert_eq!(env.snapshot(), before);
    let syncs_before = env.log().len();
    env.ok(&["approve", "superpowers", "skills/one"]);
    let commits = env.commits();
    assert!(commits.contains(&"colctl: approve skill one".to_string()));
    assert!(
        commits
            .iter()
            .any(|c| c.starts_with("colctl: approve source superpowers"))
    );
    assert!(env.log().len() > syncs_before);
    // EC4-11: the other stays held.
    assert_eq!(state(&env)["quarantine"]["skills/two"]["kind"], "skill");
    env.ok(&["approve", "--all"]);
    assert_eq!(state(&env)["quarantine"], Value::Null);
    let out = env.ok(&["approve", "--all"]);
    assert!(out.contains("nothing held"), "{out}");
}

#[test]
fn ec4_01_ec4_02_ec4_03_approve_arguments() {
    let (env, _, _) = with_source();
    let out = env.ok(&["approve", "--all"]);
    assert!(out.contains("nothing held"), "{out}");
    for (args, needle) in [
        (&["approve"][..], "or use --all"),
        (&["approve", "superpowers", "--all"], "not both"),
        (&["approve", "superpowers"], "nothing held for superpowers"),
        (&["approve", "nope"], "no source named"),
        (&["approve", "skills/nope"], "no taken-over item"),
    ] {
        let (code, _, err) = env.code(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(err.contains(needle), "{args:?}: {err}");
    }
}

#[test]
fn ec4_11_approving_one_leaves_the_others() {
    let (env, _, _, _) = risky_candidate();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    env.ok(&["approve", "skills/twg-jira"]);
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["kind"],
        "source"
    );
    assert_ne!(held_ref(&env), "");
}

#[test]
fn ec4_12_approve_refuses_a_tampered_snapshot() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    fs::write(
        env.source
            .join(".state/quarantine/skills/twg-jira/SKILL.md"),
        "evil",
    )
    .unwrap();
    let commits = env.commits();
    let out = env.expect(1, &["approve", "skills/twg-jira"]);
    assert!(
        out.contains("refused skills/twg-jira: the held copy"),
        "{out}"
    );
    assert!(source_text(&env, "skills/twg-jira/SKILL.md").ends_with("v1"));
    assert_eq!(env.commits(), commits);
}

#[test]
fn ec4_13_approve_refuses_a_missing_commit() {
    let (env, repo, v1) = with_source();
    env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    git_in(&clone(&env), &["update-ref", "-d", "refs/colctl/candidate"]);
    let out = env.expect(1, &["approve", "superpowers"]);
    assert!(
        out.contains("the held commit for superpowers is missing"),
        "{out}"
    );
    assert_eq!(pin(&env), v1);
}

#[test]
fn ec4_14_approve_refuses_when_the_pin_changed_by_hand() {
    let (env, repo, v1) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    let v3 = env.upstream_commit(&repo, "v3");
    env.ok(&["update", "superpowers", "--rev", &v2]);
    env.write_config(&env.config().replace(&v1, &v3));
    let out = env.expect(1, &["approve", "superpowers"]);
    assert!(
        out.contains(
            "superpowers's pin changed since it was held; run colctl update superpowers again"
        ),
        "{out}"
    );
    assert_eq!(pin(&env), v3);
}

#[test]
fn ec4_15_approve_refuses_a_source_copy_edited_since_it_was_held() {
    let env = machine();
    let origin = adopted_skill(&env, "twg-jira", "v1");
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    fs::write(env.source.join("skills/twg-jira/SKILL.md"), "my edit").unwrap();
    let out = env.expect(1, &["approve", "skills/twg-jira"]);
    assert!(out.contains("changed in both places"), "{out}");
    assert_eq!(source_text(&env, "skills/twg-jira/SKILL.md"), "my edit");
}

#[test]
fn ec4_16_approve_refuses_a_dirty_clone() {
    let (env, repo, v1) = with_source();
    env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    fs::write(clone(&env).join("local.txt"), "mine").unwrap();
    let commits = env.commits();
    let out = env.expect(1, &["approve", "superpowers"]);
    assert!(out.contains("has local changes"), "{out}");
    assert_eq!(pin(&env), v1);
    assert_eq!(env.commits(), commits);
}

#[test]
fn ec4_19_a_sync_failure_after_approve_keeps_the_approval() {
    let (env, repo, _) = with_source();
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    let env = env.var("FAKE_CLAUDE_FAIL", "plugin update");
    let (code, out, _) = env.code(&["approve", "superpowers"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(pin(&env), v2);
    assert_eq!(state(&env)["quarantine"], Value::Null);
    let env = env.var("FAKE_CLAUDE_FAIL", "");
    env.ok(&["sync"]);
    assert_eq!(installed_rev(&env, "claude"), v2.as_str());
}

// ---------------------------------------------------------------- status (T8)

#[test]
fn ac4_31_origin_changed_leaves_out_what_is_held_or_rejected() {
    let env = machine();
    let held = adopted_skill(&env, "held", "v1");
    let rejected = adopted_skill(&env, "rejected", "v1");
    fs::write(held.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh", "skills/held"]);
    fs::write(rejected.join("SKILL.md"), "no").unwrap();
    reject_by_hand(&env, "skills/rejected", &hash(&rejected));
    let v = env.status_json();
    assert_eq!(v["source"]["origin_changed"], serde_json::json!([]), "{v}");
    // Changing again makes it origin-changed once more.
    fs::write(held.join("SKILL.md"), "v3").unwrap();
    let v = env.status_json();
    assert_eq!(v["source"]["origin_changed"][0]["item"], "skills/held");
}
