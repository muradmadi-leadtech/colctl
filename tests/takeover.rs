//! Cycle 2: commands for Codex and ~/.agents (init, add/rm --codex, adopt), and taking over
//! skills that other tools (like twg) link in from their own folders, with `colctl refresh`.
//! Named after AC2-NN / EC2-NN in docs/codex-cli-support/.

mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use support::{Env, KILO_CODEX};

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.codex();
    env.kilo_codex();
    env.agents_dir();
    env.ok(&["init"]);
    env
}

/// twg's own folder with one skill, linked into ~/.agents/skills like twg does.
fn twg_skill(env: &Env, name: &str, body: &str) -> PathBuf {
    let origin = env.skill_dir(&env.home.join(".config/twg/skills"), name, body);
    let skills = env.home.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink(&origin, skills.join(name)).unwrap();
    origin
}

fn hash(path: &Path) -> String {
    colctl::fsx::hash_tree(path).unwrap()
}

fn state(env: &Env) -> Value {
    serde_json::from_str(
        &fs::read_to_string(env.source.join(".state/state.json")).unwrap_or("{}".into()),
    )
    .unwrap()
}

// ---------------------------------------------------------------- init, add, rm

#[test]
fn ac2_04_init_finds_codex_profiles_and_the_agents_folder() {
    let env = Env::new();
    env.native();
    env.codex();
    env.ok(&["init"]);
    let config = env.config();
    assert!(
        config.contains("name = \"codex\"\nkind = \"codex\"\npath = \"~/.codex\""),
        "{config}"
    );
    assert!(!config.contains("kilo-codex") && !config.contains("name = \"agents\""));
    env.kilo_codex();
    env.agents_dir();
    let out = env.ok(&["init"]);
    assert!(
        out.contains("registered harness kilo-codex") && out.contains("registered harness agents"),
        "{out}"
    );
    let config = env.config();
    assert!(config.contains(&format!(
        "name = \"kilo-codex\"\nkind = \"codex\"\npath = \"~/{KILO_CODEX}\""
    )));
    assert!(config.contains("name = \"agents\"\nkind = \"agents\"\npath = \"~/.agents\""));
    assert_eq!(config.matches("name = \"codex\"").count(), 1);
}

#[test]
fn ac2_17_add_and_rm_a_codex_plugin() {
    let env = machine();
    env.ok(&[
        "add",
        "plugin",
        "superpowers@superpowers",
        "--codex",
        "--marketplace",
        "github:obra/superpowers",
    ]);
    let config = env.config();
    assert!(
        config.contains("[codex.marketplaces]")
            && config.contains("superpowers = \"github:obra/superpowers\""),
        "{config}"
    );
    assert!(
        env.commits()
            .contains(&"colctl: add codex plugin superpowers@superpowers".to_string())
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["superpowers@superpowers"]
    );
    assert!(env.log().is_empty(), "nothing went to claude");
    let (code, _, err) = env.code(&["add", "plugin", "x@nowhere", "--codex"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown marketplace"), "{err}");
    env.ok(&["add", "plugin", "y@openai-curated", "--codex"]);

    env.ok(&["rm", "plugin", "superpowers@superpowers", "--codex"]);
    assert!(
        env.commits()
            .contains(&"colctl: rm codex plugin superpowers@superpowers".to_string())
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["y@openai-curated"]
    );
    assert_eq!(
        env.code(&["rm", "plugin", "nope@openai-curated", "--codex"])
            .0,
        2
    );
}

// ---------------------------------------------------------------- adopt for the new kinds

#[test]
fn ac2_06_adopt_a_codex_agents_md() {
    let env = machine();
    let native = env.home.join(".codex/AGENTS.md");
    fs::write(&native, "codex rules").unwrap();
    env.ok(&["adopt", "codex", "AGENTS.md"]);
    assert_eq!(
        fs::read_to_string(env.source.join("AGENTS.md")).unwrap(),
        "codex rules"
    );
    assert_eq!(
        fs::read_link(&native).unwrap(),
        env.source.join("AGENTS.md")
    );
    assert!(
        fs::read_link(env.home.join(KILO_CODEX).join("AGENTS.md")).is_ok(),
        "sync spread it"
    );
    assert!(
        env.commits()
            .contains(&"colctl: adopt instructions codex".to_string())
    );

    let kilo = env.home.join(KILO_CODEX).join("AGENTS.md");
    fs::remove_file(&kilo).unwrap();
    fs::write(&kilo, "different").unwrap();
    let out = env.expect(1, &["adopt", "kilo-codex", "AGENTS.md"]);
    assert!(out.contains("already exists with different text"), "{out}");
    assert_eq!(fs::read_to_string(&kilo).unwrap(), "different");
}

#[test]
fn ac2_10_foreign_codex_agent_toml_is_refused() {
    let env = machine();
    fs::create_dir_all(env.home.join(".codex/agents")).unwrap();
    fs::write(
        env.home.join(".codex/agents/mine.toml"),
        "name = \"mine\"\n",
    )
    .unwrap();
    let out = env.expect(1, &["adopt", "codex", "agents/mine.toml"]);
    assert!(
        out.contains(
            "Codex agents can't be converted back to .md; recreate it in ~/.colctl/agents"
        ),
        "{out}"
    );
    assert!(env.home.join(".codex/agents/mine.toml").is_file());
}

#[test]
fn ac2_18_adopt_a_codex_plugin() {
    let env = machine();
    fs::write(
        env.home.join(KILO_CODEX).join("config.toml"),
        "[marketplaces.m]\nsource_type = \"git\"\nsource = \"https://github.com/o/r.git\"\n\n[plugins.\"p@m\"]\nenabled = true\n",
    )
    .unwrap();
    let env = env.var("FAKE_CODEX_MARKET_NAME", "m");
    env.ok(&["adopt", "kilo-codex", "plugin", "p@m"]);
    let config = env.config();
    assert!(
        config.contains("[codex.marketplaces]")
            && config.contains("m = \"https://github.com/o/r.git\""),
        "{config}"
    );
    assert!(
        env.commits()
            .contains(&"colctl: adopt codex plugin p@m".to_string())
    );
    assert!(
        state(&env)["harnesses"]["kilo-codex"]["plugins"]
            .as_array()
            .is_none_or(|a| a.is_empty())
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["p@m"],
        "sync installed it in the other profile"
    );
}

#[test]
fn ac2_19_adopt_everything_includes_the_new_kinds() {
    let env = machine();
    fs::write(env.home.join(".codex/AGENTS.md"), "rules").unwrap();
    env.skill_dir(&env.home.join(".agents/skills"), "local-skill", "x");
    fs::write(
        env.home.join(".codex/config.toml"),
        "[plugins.\"x@openai-curated\"]\nenabled = true\n",
    )
    .unwrap();
    fs::create_dir_all(env.home.join(".codex/agents")).unwrap();
    fs::write(
        env.home.join(".codex/agents/mine.toml"),
        "name = \"mine\"\n",
    )
    .unwrap();
    let out = env.expect(1, &["adopt"]);
    for line in [
        "adopted AGENTS.md from codex",
        "adopted skills/local-skill from agents",
        "adopted plugin x@openai-curated from codex",
        "refused agents/mine.toml in codex: Codex agents can't be converted back",
    ] {
        assert!(out.contains(line), "{line:?} in:\n{out}");
    }
}

// ---------------------------------------------------------------- taking over linked skills

#[test]
fn ac2_24_links_into_other_folders_are_visible() {
    let env = machine();
    let origin = twg_skill(&env, "twg-jira", "jira");
    let out = env.expect(1, &["status"]);
    assert!(
        out.contains(&format!("linked-elsewhere     skill         twg-jira  → links to {}; colctl adopt agents skills/twg-jira", origin.display())),
        "{out}"
    );
    env.expect(1, &["sync"]);
    assert_eq!(
        fs::read_link(env.home.join(".agents/skills/twg-jira")).unwrap(),
        origin
    );
}

#[test]
fn ac2_25_adopt_takes_the_target_over() {
    let env = machine();
    let origin = twg_skill(&env, "twg-jira", "jira");
    let before = hash(&origin);
    env.ok(&["adopt", "agents", "skills/twg-jira"]);
    let copy = env.source.join("skills/twg-jira");
    assert_eq!(hash(&copy), before);
    assert_eq!(
        fs::read_link(env.home.join(".agents/skills/twg-jira")).unwrap(),
        copy
    );
    assert_eq!(hash(&origin), before, "twg's folder is untouched");
    let recorded = &state(&env)["origins"]["skills/twg-jira"];
    assert_eq!(recorded["path"], origin.display().to_string());
    assert_eq!(recorded["hash"], before);
    assert!(
        env.commits()
            .contains(&"colctl: adopt skill twg-jira".to_string())
    );
    assert_eq!(
        fs::read_link(env.home.join(".claude/skills/twg-jira")).unwrap(),
        copy,
        "spread to Claude"
    );
    env.ok(&["status"]);
}

#[test]
fn ac2_26_origin_changes_and_refresh() {
    let env = machine();
    let origin = twg_skill(&env, "twg-jira", "v1");
    env.ok(&["adopt"]);
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    let out = env.expect(1, &["status"]);
    assert!(
        out.contains("origin-changed") && out.contains("colctl refresh skills/twg-jira"),
        "{out}"
    );
    let v = env.status_json();
    assert_eq!(v["source"]["origin_changed"][0]["item"], "skills/twg-jira");

    env.ok(&["refresh", "--dry-run"]);
    assert_eq!(
        fs::read_to_string(env.source.join("skills/twg-jira/SKILL.md")).unwrap(),
        "v1"
    );
    // Cycle 4: refresh holds the new content for review; the source copy stays at v1.
    let out = env.ok(&["refresh", "skills/twg-jira"]);
    assert!(out.contains("held skills/twg-jira for review"), "{out}");
    assert_eq!(
        fs::read_to_string(env.source.join("skills/twg-jira/SKILL.md")).unwrap(),
        "v1"
    );
    assert_eq!(
        state(&env)["quarantine"]["skills/twg-jira"]["hash"],
        hash(&origin)
    );
    env.ok(&["approve", "skills/twg-jira"]);
    assert_eq!(
        fs::read_to_string(env.source.join("skills/twg-jira/SKILL.md")).unwrap(),
        "v2"
    );
    assert_eq!(
        state(&env)["origins"]["skills/twg-jira"]["hash"],
        hash(&origin)
    );
    env.ok(&["status"]);

    // Changed in both places: refused.
    fs::write(origin.join("SKILL.md"), "v3").unwrap();
    fs::write(env.source.join("skills/twg-jira/SKILL.md"), "my edit").unwrap();
    let out = env.expect(1, &["refresh"]);
    assert!(out.contains("changed in both places"), "{out}");
    assert_eq!(
        fs::read_to_string(env.source.join("skills/twg-jira/SKILL.md")).unwrap(),
        "my edit"
    );
}

#[test]
fn ac2_27_the_tool_re_links_over_colctl() {
    let env = machine();
    let origin = twg_skill(&env, "twg-jira", "v1");
    env.ok(&["adopt"]);
    let link = env.home.join(".agents/skills/twg-jira");
    fs::remove_file(&link).unwrap();
    symlink(&origin, &link).unwrap();
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    let out = env.expect(1, &["adopt", "agents", "skills/twg-jira"]);
    assert!(
        out.contains("origin has changes; run colctl refresh skills/twg-jira"),
        "{out}"
    );
    assert_eq!(
        fs::read_link(&link).unwrap(),
        env.source.join("skills/twg-jira")
    );
    assert_eq!(
        fs::read_to_string(env.source.join("skills/twg-jira/SKILL.md")).unwrap(),
        "v1",
        "no copy made"
    );
}

#[test]
fn ac2_28_what_adopt_still_refuses() {
    let env = machine();
    let skills = env.home.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink("/no/such/folder", skills.join("dangling")).unwrap();
    let file = env.home.join("a-file");
    fs::write(&file, "x").unwrap();
    symlink(&file, skills.join("not-a-folder")).unwrap();
    symlink("/elsewhere/AGENTS.md", env.home.join(".codex/AGENTS.md")).unwrap();
    let out = env.expect(1, &["adopt"]);
    assert!(
        out.contains(
            "refused skills/dangling in agents: it links to /no/such/folder, which doesn't exist"
        ),
        "{out}"
    );
    assert!(
        out.contains(
            "refused skills/not-a-folder in agents: it links to a file, not a skill folder"
        ),
        "{out}"
    );
    assert!(
        out.contains("refused AGENTS.md in codex: remove or replace this link yourself"),
        "{out}"
    );

    // A different source copy whose origin is somewhere else: both shown.
    env.skill_dir(&env.source.join("skills"), "clash", "mine");
    let other = env.skill_dir(&env.home.join("elsewhere"), "clash", "theirs");
    let kilo_skills = env.home.join(".claude/skills");
    fs::create_dir_all(&kilo_skills).unwrap();
    fs::remove_file(kilo_skills.join("clash")).ok();
    symlink(&other, kilo_skills.join("clash")).unwrap();
    let out = env.expect(1, &["adopt", "claude", "skills/clash"]);
    assert!(
        out.contains("differs from the source copy") && out.contains(&other.display().to_string()),
        "{out}"
    );
}

#[test]
fn ec2_23_ec2_27_chains_and_relative_links_resolve_to_the_final_folder() {
    let env = machine();
    let real = env.skill_dir(&env.home.join(".config/real"), "chained", "x");
    let hop = env.home.join(".config/hop");
    symlink(&real, &hop).unwrap();
    let skills = env.home.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink("../../.config/hop", skills.join("chained")).unwrap();
    env.ok(&["adopt", "agents", "skills/chained"]);
    assert_eq!(
        state(&env)["origins"]["skills/chained"]["path"],
        real.display().to_string()
    );
}

#[test]
fn ec2_24_origin_deleted_later() {
    let env = machine();
    let origin = twg_skill(&env, "twg-jira", "v1");
    env.ok(&["adopt"]);
    fs::remove_dir_all(&origin).unwrap();
    env.ok(&["status"]);
    let out = env.expect(1, &["refresh", "skills/twg-jira"]);
    assert!(out.contains("origin is gone"), "{out}");
    assert!(env.source.join("skills/twg-jira/SKILL.md").is_file());
}

#[test]
fn ec2_25_one_origin_under_two_names() {
    let env = machine();
    let origin = twg_skill(&env, "one", "x");
    symlink(&origin, env.home.join(".agents/skills/two")).unwrap();
    env.ok(&["adopt"]);
    let s = state(&env);
    assert_eq!(
        s["origins"]["skills/one"]["path"],
        origin.display().to_string()
    );
    assert_eq!(
        s["origins"]["skills/two"]["path"],
        origin.display().to_string()
    );
}

#[test]
fn ec2_26_origin_with_a_git_folder_is_copied_as_is() {
    let env = machine();
    let origin = twg_skill(&env, "tracked", "x");
    fs::create_dir_all(origin.join(".git")).unwrap();
    fs::write(origin.join(".git/HEAD"), "ref").unwrap();
    env.ok(&["adopt"]);
    assert!(env.source.join("skills/tracked/.git/HEAD").is_file());
}

#[test]
fn ec2_28_refresh_without_an_origin() {
    let env = machine();
    env.source_skill("plain");
    let (code, _, err) = env.code(&["refresh", "skills/plain"]);
    assert_eq!(code, 2);
    assert!(err.contains("skills/plain has no recorded origin"), "{err}");
    env.ok(&["refresh"]);
}
