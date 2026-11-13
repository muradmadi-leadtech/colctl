//! End-to-end tests for cycle 2 (Codex CLI support): the real binary against a temp home with
//! Claude and Codex profiles, the shared ~/.agents folder, and fake `claude` and `codex` CLIs.
//! Named after AC2-NN / EC2-NN in docs/codex-cli-support/.

mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use support::{Env, KILO_CODEX};

/// A machine with the native Claude profile, both Codex profiles and ~/.agents, all registered.
fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.codex();
    env.kilo_codex();
    env.agents_dir();
    env.ok(&["init"]);
    env.set_harnesses(&[
        ("claude", "claude", "~/.claude"),
        ("codex", "codex", "~/.codex"),
        ("kilo-codex", "codex", &format!("~/{KILO_CODEX}")),
        ("agents", "agents", "~/.agents"),
    ]);
    env
}

fn codex_plugins(env: &Env, markets: &str, plugins: &str) {
    let config = env.config();
    let text = config
        .split("\n[codex.marketplaces]")
        .next()
        .unwrap()
        .replace("[plugins]\nenabled = []\n", "");
    env.write_config(&format!(
        "{text}\n[codex.marketplaces]\n{markets}\n\n[codex.plugins]\nenabled = [{plugins}]\n"
    ));
}

const REVIEWER: &str =
    "---\nname: reviewer\ndescription: Reviews diffs\nmodel: opus\n---\nYou review diffs.\n";

fn parse(path: &std::path::Path) -> toml::Table {
    toml::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn ac2_05_agents_md_is_linked_and_unlinked() {
    let env = machine();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);
    for profile in [env.home.join(".codex"), env.home.join(KILO_CODEX)] {
        assert_eq!(
            fs::read_link(profile.join("AGENTS.md")).unwrap(),
            env.source.join("AGENTS.md")
        );
    }
    assert_eq!(
        env.item_state("kilo-codex", "instructions", "AGENTS.md")
            .as_deref(),
        Some("ok")
    );
    assert!(
        !env.home.join(".agents/AGENTS.md").exists(),
        "the agents kind has no instructions"
    );
    fs::remove_file(env.source.join("AGENTS.md")).unwrap();
    env.ok(&["sync"]);
    assert!(fs::symlink_metadata(env.home.join(".codex/AGENTS.md")).is_err());
}

#[test]
fn ac2_07_skills_are_linked_into_the_shared_folder() {
    let env = machine();
    env.source_skill("foo");
    env.source_skill("bar");
    let shared = env.home.join(".agents");
    env.skill_dir(&shared.join("skills"), "twg-jira", "theirs");
    fs::write(shared.join(".skill-lock.json"), "{\"version\":3}").unwrap();
    let before = colctl::fsx::hash_tree(&shared.join("skills/twg-jira")).unwrap();
    let out = env.expect(1, &["sync"]);
    for name in ["foo", "bar"] {
        assert_eq!(
            fs::read_link(shared.join("skills").join(name)).unwrap(),
            env.source.join("skills").join(name)
        );
    }
    assert_eq!(
        colctl::fsx::hash_tree(&shared.join("skills/twg-jira")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_to_string(shared.join(".skill-lock.json")).unwrap(),
        "{\"version\":3}"
    );
    assert!(out.contains("colctl adopt agents skills/twg-jira"), "{out}");
    assert_eq!(
        env.item_state("agents", "skill", "twg-jira").as_deref(),
        Some("foreign")
    );
    assert!(
        !env.home.join(".codex/skills").exists(),
        "codex profiles get skills through ~/.agents"
    );
}

#[test]
fn ac2_08_agents_become_codex_toml() {
    let env = machine();
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    env.ok(&["sync"]);
    for profile in [env.home.join(".codex"), env.home.join(KILO_CODEX)] {
        let path = profile.join("agents/reviewer.toml");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(
            "# Managed by colctl. Edit ~/.colctl/agents/reviewer.md instead; this file is regenerated.\n"
        ));
        let t = parse(&path);
        assert_eq!(t["name"].as_str(), Some("reviewer"));
        assert_eq!(t["description"].as_str(), Some("Reviews diffs"));
        assert_eq!(
            t["developer_instructions"].as_str(),
            Some("You review diffs.")
        );
        assert!(!text.contains("opus"));
    }
    let state: Value =
        serde_json::from_str(&fs::read_to_string(env.source.join(".state/state.json")).unwrap())
            .unwrap();
    assert!(
        state["harnesses"]["kilo-codex"]["generated"]["agents/reviewer.toml"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(
        fs::read_link(env.home.join(".claude/agents/reviewer.md")).unwrap(),
        env.source.join("agents/reviewer.md")
    );
    assert_eq!(
        env.item_state("codex", "agent", "reviewer").as_deref(),
        Some("ok")
    );
}

#[test]
fn ac2_09_generated_agents_follow_their_source() {
    let env = machine();
    let md = env.source.join("agents/reviewer.md");
    fs::write(&md, REVIEWER).unwrap();
    env.ok(&["sync"]);
    fs::write(
        &md,
        REVIEWER.replace("You review diffs.", "You review diffs carefully."),
    )
    .unwrap();
    assert_eq!(
        env.item_state("codex", "agent", "reviewer").as_deref(),
        Some("broken")
    );
    env.ok(&["sync"]);
    let toml = env.home.join(".codex/agents/reviewer.toml");
    assert_eq!(
        parse(&toml)["developer_instructions"].as_str(),
        Some("You review diffs carefully.")
    );
    fs::remove_file(&md).unwrap();
    assert_eq!(
        env.item_state("codex", "agent", "reviewer").as_deref(),
        Some("orphaned")
    );
    env.ok(&["sync"]);
    assert!(!toml.exists());
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(!state.contains("agents/reviewer.toml"), "{state}");
}

#[test]
fn ec2_02_no_source_agents_means_no_agent_files() {
    let env = machine();
    env.ok(&["sync"]);
    assert!(!env.home.join(".codex/agents").exists());
}

#[test]
fn ac2_10_hand_edited_and_foreign_agent_toml_are_left_alone() {
    let env = machine();
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    env.ok(&["sync"]);
    let edited = env.home.join(".codex/agents/reviewer.toml");
    fs::write(&edited, "name = \"mine now\"\n").unwrap();
    let mine = env.home.join(".codex/agents/mine.toml");
    fs::write(&mine, "name = \"mine\"\n").unwrap();
    let out = env.expect(1, &["sync"]);
    assert_eq!(
        fs::read_to_string(&edited).unwrap(),
        "name = \"mine now\"\n"
    );
    assert_eq!(fs::read_to_string(&mine).unwrap(), "name = \"mine\"\n");
    assert_eq!(
        env.item_state("codex", "agent", "reviewer").as_deref(),
        Some("edited")
    );
    assert_eq!(
        env.item_state("codex", "agent", "mine").as_deref(),
        Some("foreign")
    );
    assert!(out.contains("colctl adopt codex agents/mine.toml"), "{out}");
}

#[test]
fn ec2_17_a_hand_made_toml_wins_over_a_source_agent_of_the_same_name() {
    let env = machine();
    fs::create_dir_all(env.home.join(".codex/agents")).unwrap();
    fs::write(
        env.home.join(".codex/agents/reviewer.toml"),
        "name = \"hand made\"\n",
    )
    .unwrap();
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    env.expect(1, &["sync"]);
    assert_eq!(
        fs::read_to_string(env.home.join(".codex/agents/reviewer.toml")).unwrap(),
        "name = \"hand made\"\n"
    );
    assert_eq!(
        env.item_state("codex", "agent", "reviewer").as_deref(),
        Some("foreign")
    );
    assert!(
        env.home
            .join(KILO_CODEX)
            .join("agents/reviewer.toml")
            .is_file(),
        "the other profile still gets it"
    );
}

#[test]
fn ac2_11_unconvertible_agents_are_invalid_for_codex_only() {
    let env = machine();
    fs::write(env.source.join("agents/bad.md"), "no frontmatter here\n").unwrap();
    let out = env.expect(1, &["sync"]);
    assert!(out.contains("no frontmatter"), "{out}");
    assert_eq!(
        env.item_state("kilo-codex", "agent", "bad").as_deref(),
        Some("invalid")
    );
    assert!(fs::read_link(env.home.join(".claude/agents/bad.md")).is_ok());
    assert!(!env.home.join(".codex/agents/bad.toml").exists());
}

#[test]
fn ac2_12_codex_plugins_are_installed_with_codex() {
    let env = machine();
    codex_plugins(
        &env,
        "superpowers = \"github:obra/superpowers\"",
        "\"superpowers@superpowers\"",
    );
    env.ok(&["sync"]);
    assert_eq!(
        env.codex_log_args(),
        vec![
            "plugin marketplace add obra/superpowers",
            "plugin add superpowers@superpowers",
            "plugin marketplace add obra/superpowers",
            "plugin add superpowers@superpowers",
        ]
    );
    for profile in [env.home.join(".codex"), env.home.join(KILO_CODEX)] {
        assert_eq!(
            env.codex_installed(&profile),
            vec!["superpowers@superpowers"]
        );
    }
    assert!(env.log().is_empty(), "no claude calls");
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(state.contains("superpowers@superpowers"));
    env.ok(&["sync"]);
    assert_eq!(env.codex_log().len(), 4, "a second sync has nothing to do");
}

#[test]
fn ac2_13_codex_plugins_are_removed_only_where_colctl_installed_them() {
    let env = machine();
    fs::write(
        env.home.join(KILO_CODEX).join("config.toml"),
        "[plugins.\"x@openai-curated\"]\nenabled = true\n",
    )
    .unwrap();
    codex_plugins(&env, "", "\"x@openai-curated\"");
    env.ok(&["sync"]);
    assert_eq!(env.codex_log_args(), vec!["plugin add x@openai-curated"]);
    codex_plugins(&env, "", "");
    env.expect(1, &["sync"]);
    let args = env.codex_log_args();
    assert_eq!(args.last().unwrap(), "plugin remove x@openai-curated");
    assert_eq!(args.len(), 2);
    assert_eq!(
        env.codex_log()[1]["codex_home"],
        Value::Null,
        "only the native profile"
    );
    assert_eq!(
        env.codex_installed(&env.home.join(KILO_CODEX)),
        vec!["x@openai-curated"]
    );
}

#[test]
fn ac2_14_ec2_15_ec2_20_each_codex_profile_gets_its_own_env() {
    let env = machine()
        .var("CODEX_HOME", "/wrong")
        .var("OPENAI_API_KEY", "secret")
        .var("OPENAI_BASE_URL", "http://127.0.0.1:8877");
    // The native profile written as an absolute path is still native.
    let config = env.config().replace(
        "path = \"~/.codex\"",
        &format!("path = \"{}\"", env.home.join(".codex").display()),
    );
    env.write_config(&config);
    codex_plugins(&env, "", "\"x@openai-curated\"");
    env.ok(&["sync"]);
    let log = env.codex_log();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0]["codex_home"], Value::Null);
    assert_eq!(
        log[1]["codex_home"],
        env.home.join(KILO_CODEX).display().to_string()
    );
    for line in &log {
        assert_eq!(line["openai"], serde_json::json!([]));
    }
}

#[test]
fn ac2_15_codex_missing_still_syncs_files() {
    let env = Env::without_codex();
    env.codex();
    env.ok(&["init"]);
    env.set_harnesses(&[("codex", "codex", "~/.codex")]);
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    codex_plugins(&env, "", "\"x@openai-curated\"");
    let out = env.expect(1, &["sync"]);
    assert!(
        Env::words(&out).contains("install x failed: skipped: codex not found"),
        "{out}"
    );
    assert!(fs::read_link(env.home.join(".codex/AGENTS.md")).is_ok());
    assert!(env.home.join(".codex/agents/reviewer.toml").is_file());
}

#[test]
fn ac2_15_codex_failures_are_reported_in_full() {
    let env = machine().var("FAKE_CODEX_FAIL", "add x@openai-curated");
    codex_plugins(&env, "", "\"x@openai-curated\", \"y@openai-curated\"");
    let out = env.expect(1, &["sync", "--harness", "kilo-codex"]);
    assert!(
        Env::words(&out).contains("✗ install x failed: codex plugin add x@openai-curated exited 1: fake codex failure for: plugin add x@openai-curated"),
        "{out}"
    );
    assert!(Env::words(&out).contains("installed y"), "{out}");
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(!state.contains("x@openai-curated"), "{state}");
}

#[test]
fn ec2_14_marketplace_that_names_itself_differently() {
    let env = machine().var("FAKE_CODEX_MARKET_NAME", "superpowers-dev");
    codex_plugins(
        &env,
        "superpowers = \"github:obra/superpowers\"",
        "\"superpowers@superpowers\"",
    );
    let out = env.expect(1, &["sync", "--harness", "codex"]);
    assert!(
        out.contains(
            "config calls it \"superpowers\", the marketplace calls itself \"superpowers-dev\""
        ),
        "{out}"
    );
    assert!(
        Env::words(&out).contains(
            "install superpowers failed: skipped: marketplace \"superpowers\" wasn't added"
        ),
        "{out}"
    );
}

#[test]
fn ec2_18_hung_codex_is_killed() {
    let env = machine()
        .var("FAKE_CODEX_SLEEP", "5")
        .var("COLCTL_TOOL_TIMEOUT_SECS", "1");
    codex_plugins(&env, "", "\"x@openai-curated\"");
    let started = std::time::Instant::now();
    let out = env.expect(1, &["sync", "--harness", "codex"]);
    assert!(out.contains("timed out after 1s"), "{out}");
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
}

#[test]
fn ec2_01_ec2_03_missing_folders_are_created() {
    let env = machine();
    assert!(!env.home.join(".agents/skills").exists());
    env.source_skill("foo");
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    env.ok(&["sync"]);
    assert!(env.home.join(".agents/skills").is_dir());
    assert!(env.home.join(".codex/agents").is_dir());
}

#[test]
fn ec2_19_one_unwritable_codex_profile_does_not_stop_the_other() {
    let env = machine();
    let agents = env.home.join(".codex/agents");
    fs::create_dir_all(&agents).unwrap();
    fs::set_permissions(&agents, fs::Permissions::from_mode(0o555)).unwrap();
    fs::write(env.source.join("agents/reviewer.md"), REVIEWER).unwrap();
    let out = env.expect(1, &["sync"]);
    fs::set_permissions(&agents, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(out.contains("✗ write"), "{out}");
    assert!(
        env.home
            .join(KILO_CODEX)
            .join("agents/reviewer.toml")
            .is_file()
    );
}

#[test]
fn ac2_20_status_json_shows_the_kinds() {
    let env = machine();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);
    let v = env.status_json();
    let kinds: Vec<(&str, &str)> = v["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| (h["name"].as_str().unwrap(), h["kind"].as_str().unwrap()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("claude", "claude"),
            ("codex", "codex"),
            ("kilo-codex", "codex"),
            ("agents", "agents")
        ]
    );
    let codex = &v["harnesses"][1]["items"];
    assert!(
        codex
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "instructions" && i["name"] == "AGENTS.md")
    );
}

#[test]
fn foreign_agents_md_in_a_codex_profile() {
    let env = machine();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    fs::write(env.home.join(".codex/AGENTS.md"), "my own").unwrap();
    symlink(
        "/elsewhere/AGENTS.md",
        env.home.join(KILO_CODEX).join("AGENTS.md"),
    )
    .unwrap();
    let out = env.expect(1, &["sync"]);
    assert_eq!(
        fs::read_to_string(env.home.join(".codex/AGENTS.md")).unwrap(),
        "my own"
    );
    assert!(out.contains("colctl adopt codex AGENTS.md"), "{out}");
    assert_eq!(
        fs::read_link(env.home.join(KILO_CODEX).join("AGENTS.md"))
            .unwrap()
            .to_str(),
        Some("/elsewhere/AGENTS.md")
    );
}
