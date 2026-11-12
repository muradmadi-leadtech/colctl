//! End-to-end tests: the real binary against a temp home, a fake `claude` and the real `git`.
//! Each test is named after the acceptance criterion (AC-NN) or edge case (EC-NN) it proves; see
//! docs/claude-profile-sync/acceptance-criteria.md and edge-cases.md.

mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use support::{Env, KILO};

const GENERATED: &str = "<!-- Managed by colctl. Edit ~/.colctl/AGENTS.md instead; this file is regenerated. -->\n@~/.colctl/AGENTS.md\n";

/// A machine with both profiles and a fresh source.
fn two_profiles() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.ok(&["init"]);
    env
}

fn add_marketplace_and_plugins(env: &Env) {
    env.write_config(&format!(
        "{}\n[marketplaces]\nsadr = \"github:muradmadi/sadr\"\n\n[plugins]\nenabled = [\"sadr@sadr\", \"superpowers@anthropic-plugin-directory\"]\n",
        env.config().split("\n[marketplaces]").next().unwrap()
    ));
}

// ---------------------------------------------------------------- init

#[test]
fn ac_04_first_init_creates_the_source() {
    let env = Env::new();
    env.native();
    let out = env.ok(&["init"]);
    for path in [
        "config.toml",
        "skills",
        "agents",
        ".state",
        ".gitignore",
        ".git",
    ] {
        assert!(env.source.join(path).exists(), "{path}");
    }
    assert!(!env.source.join("AGENTS.md").exists());
    assert!(
        fs::read_to_string(env.source.join(".gitignore"))
            .unwrap()
            .contains(".state/")
    );
    assert_eq!(env.commits(), vec!["colctl: init"]);
    assert!(out.contains("created ~/.colctl"), "{out}");
    assert!(
        out.contains("claude (~/.claude)") && out.contains("in sync"),
        "status follows: {out}"
    );
}

#[test]
fn ac_05_init_registers_the_harnesses_it_finds() {
    let env = Env::new();
    env.native();
    env.kilo();
    env.ok(&["init"]);
    let config = env.config();
    assert!(
        config.contains("name = \"claude\"\nkind = \"claude\"\npath = \"~/.claude\""),
        "{config}"
    );
    assert!(config.contains(&format!(
        "name = \"kilo-claude\"\nkind = \"claude\"\npath = \"~/{KILO}\""
    )));

    let only_native = Env::new();
    only_native.native();
    only_native.ok(&["init"]);
    assert!(!only_native.config().contains("kilo-claude"));
}

#[test]
fn ac_06_running_init_again_only_adds() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let edited = format!("# my note\n{}", env.config());
    env.write_config(&edited);
    env.kilo();
    let out = env.ok(&["init"]);
    assert!(out.contains("registered harness kilo-claude"), "{out}");
    let config = env.config();
    assert!(config.starts_with(&edited), "{config}");
    assert!(config.contains("kilo-claude"));
    assert_eq!(env.commits(), vec!["colctl: init", "colctl: init"]);
    // Nothing new: nothing changes.
    env.ok(&["init"]);
    assert_eq!(env.commits().len(), 2);
    assert!(env.ok(&["init", "--dry-run"]).contains("nothing to add"));
}

#[test]
fn ac_06_name_clash_gets_a_suffix() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    env.write_config("version = 1\n\n[[harness]]\nname = \"kilo-claude\"\nkind = \"claude\"\npath = \"/elsewhere\"\n");
    env.kilo();
    let out = env.ok(&["init", "--dry-run"]);
    assert!(
        out.contains("would register harness kilo-claude-2"),
        "{out}"
    );
}

#[test]
fn ac_07_init_dry_run_changes_nothing() {
    let env = Env::new();
    env.native();
    let before = env.snapshot();
    let out = env.ok(&["init", "--dry-run"]);
    assert!(
        out.contains("would create ~/.colctl") && out.contains("would register harness claude"),
        "{out}"
    );
    assert_eq!(env.snapshot(), before);
    assert!(!env.source.exists());
    let bare = Env::new();
    assert!(
        bare.ok(&["init", "--dry-run"])
            .contains("found no known harnesses")
    );
}

#[test]
fn ec_01_commands_before_init_say_run_init() {
    let env = Env::new();
    for args in [
        &["status"][..],
        &["sync"],
        &["adopt"],
        &["rm", "skill", "x"],
    ] {
        let (code, _, err) = env.code(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(err.contains("run colctl init"), "{err}");
    }
}

#[test]
fn ec_02_no_harnesses_registered() {
    let env = Env::new();
    env.ok(&["init"]);
    let out = env.ok(&["status"]);
    assert!(out.contains("no harnesses registered"), "{out}");
    let out = env.ok(&["sync"]);
    assert!(out.contains("in sync"), "{out}");
}

// ---------------------------------------------------------------- status

#[test]
fn ac_08_status_reports_every_kind_of_drift() {
    let env = two_profiles();
    let native = env.home.join(".claude");
    env.source_skill("ok");
    env.source_skill("broken");
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);

    // Make one item of each kind of trouble.
    env.source_skill("missing");
    fs::remove_file(native.join("skills/broken")).unwrap();
    symlink(
        env.source.join("skills/elsewhere"),
        native.join("skills/broken"),
    )
    .unwrap();
    env.skill_dir(&native.join("skills"), "foreign", "mine");
    fs::write(native.join("CLAUDE.md"), "hand edited").unwrap();
    fs::create_dir_all(env.source.join("skills/no-skill-md")).unwrap();
    add_marketplace_and_plugins(&env);
    fs::remove_dir_all(env.home.join(KILO)).unwrap();

    let before = env.snapshot();
    let (code, out, _) = env.code(&["status"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(env.snapshot(), before, "status writes nothing");
    assert!(!env.source.join(".state/lock").exists());
    for needle in [
        "ok                   skill         ok",
        "missing              skill         missing  → colctl sync",
        "broken               skill         broken  → colctl sync",
        "foreign              skill         foreign  → colctl adopt claude skills/foreign",
        "edited               instructions  CLAUDE.md  → move your text into ~/.colctl/AGENTS.md",
        "plugin-missing       plugin        sadr@sadr",
        "marketplace-missing  marketplace   sadr",
        "invalid              skill         no-skill-md  → no SKILL.md",
        "uncommitted",
        "harness-missing → start that profile once",
        "not in sync",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }

    let v = env.status_json();
    assert_eq!(v["version"], 1);
    assert_eq!(v["in_sync"], false);
    let kilo = v["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "kilo-claude")
        .unwrap();
    assert_eq!(kilo["state"], "harness-missing");
    assert!(v["source"]["uncommitted"].as_array().unwrap().len() >= 2);
    assert_eq!(v["source"]["invalid"][0]["name"], "no-skill-md");
    let item = |name: &str| env.item_state("claude", "skill", name);
    assert_eq!(item("foreign").as_deref(), Some("foreign"));
    assert_eq!(item("missing").as_deref(), Some("missing"));
}

#[test]
fn ac_08_status_is_zero_when_in_sync() {
    let env = two_profiles();
    env.source_skill("foo");
    env.ok(&["sync"]);
    let out = env.ok(&["status"]);
    assert!(out.trim_end().ends_with("in sync"), "{out}");
    assert_eq!(env.status_json()["in_sync"], true);
    let out = env.ok(&["status", "--harness", "kilo-claude"]);
    assert!(!out.contains("claude (~/.claude)"), "{out}");
}

// ---------------------------------------------------------------- sync

#[test]
fn ac_09_ac_10_ac_11_sync_links_skills_agents_and_imports_agents_md() {
    let env = two_profiles();
    env.source_skill("foo");
    fs::write(env.source.join("agents/reviewer.md"), "agent").unwrap();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);
    for harness in [env.home.join(".claude"), env.home.join(KILO)] {
        assert_eq!(
            fs::read_link(harness.join("skills/foo")).unwrap(),
            env.source.join("skills/foo")
        );
        assert_eq!(
            fs::read_link(harness.join("agents/reviewer.md")).unwrap(),
            env.source.join("agents/reviewer.md")
        );
        assert_eq!(
            fs::read_to_string(harness.join("CLAUDE.md")).unwrap(),
            GENERATED
        );
    }
    let state: Value =
        serde_json::from_str(&fs::read_to_string(env.source.join(".state/state.json")).unwrap())
            .unwrap();
    assert!(
        state["harnesses"]["kilo-claude"]["generated"]["CLAUDE.md"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(
        env.item_state("kilo-claude", "skill", "foo").as_deref(),
        Some("ok")
    );
    // SC-1: one copy on disk.
    assert!(
        fs::symlink_metadata(env.home.join(".claude/skills/foo"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn ac_11_colctl_home_is_written_as_an_absolute_import() {
    let env = Env::new();
    env.native();
    let custom = env.home.join("custom-source");
    let env = env.var("COLCTL_HOME", custom.to_str().unwrap());
    env.ok(&["init"]);
    fs::write(custom.join("AGENTS.md"), "x").unwrap();
    env.ok(&["sync"]);
    let text = fs::read_to_string(env.home.join(".claude/CLAUDE.md")).unwrap();
    assert!(
        text.contains(&format!("@{}/AGENTS.md", custom.display())),
        "{text}"
    );
}

#[test]
fn ac_12_plugins_are_installed_with_claudes_cli() {
    let env = two_profiles();
    add_marketplace_and_plugins(&env);
    env.ok(&["sync"]);
    let args = env.log_args();
    assert_eq!(
        args,
        vec![
            "plugin marketplace add muradmadi/sadr",
            "plugin install sadr@sadr",
            "plugin install superpowers@anthropic-plugin-directory",
            "plugin marketplace add muradmadi/sadr",
            "plugin install sadr@sadr",
            "plugin install superpowers@anthropic-plugin-directory",
        ]
    );
    for harness in [env.home.join(".claude"), env.home.join(KILO)] {
        assert_eq!(
            env.installed(&harness),
            vec!["sadr@sadr", "superpowers@anthropic-plugin-directory"]
        );
    }
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(state.contains("sadr@sadr") && state.contains("\"sadr\""));
    // A second sync has nothing to do.
    env.ok(&["sync"]);
    assert_eq!(env.log_args().len(), 6);
}

#[test]
fn ec_20_ec_28_each_profile_gets_exactly_its_own_env() {
    let env = two_profiles()
        .var("CLAUDE_CONFIG_DIR", "/wrong/place")
        .var("CLAUDE_SECURESTORAGE_CONFIG_DIR", "/wrong/too")
        .var("ANTHROPIC_API_KEY", "secret")
        .var("ANTHROPIC_BASE_URL", "http://127.0.0.1:8877");
    env.write_config(&format!(
        "{}\n[plugins]\nenabled = [\"p@anthropic-plugin-directory\"]\n",
        env.config().split("\n[plugins]").next().unwrap()
    ));
    env.ok(&["sync"]);
    let log = env.log();
    assert_eq!(log.len(), 2);
    assert_eq!(
        log[0]["config_dir"],
        Value::Null,
        "native runs without CLAUDE_CONFIG_DIR"
    );
    assert_eq!(
        log[1]["config_dir"],
        env.home.join(KILO).display().to_string()
    );
    for line in &log {
        assert_eq!(line["anthropic"], serde_json::json!([]));
        assert_eq!(line["secure_storage"], false);
    }
}

#[test]
fn ac_13_broken_owned_links_are_repaired() {
    let env = two_profiles();
    env.source_skill("foo");
    let link = env.home.join(".claude/skills/foo");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(env.source.join("skills/wrong"), &link).unwrap();
    env.ok(&["sync"]);
    assert_eq!(fs::read_link(&link).unwrap(), env.source.join("skills/foo"));
}

#[test]
fn ac_14_removed_source_items_are_removed_from_harnesses() {
    let env = two_profiles();
    env.source_skill("foo");
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);
    fs::remove_dir_all(env.source.join("skills/foo")).unwrap();
    fs::remove_file(env.source.join("AGENTS.md")).unwrap();
    env.ok(&["sync"]);
    let native = env.home.join(".claude");
    assert!(fs::symlink_metadata(native.join("skills/foo")).is_err());
    assert!(!native.join("CLAUDE.md").exists());
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(!state.contains("CLAUDE.md"), "{state}");
}

#[test]
fn ac_15_foreign_items_are_never_touched() {
    let env = two_profiles();
    env.source_skill("foo");
    let foreign = env.skill_dir(&env.home.join(".claude/skills"), "foo", "mine");
    let out = env.expect(1, &["sync"]);
    assert_eq!(
        fs::read_to_string(foreign.join("SKILL.md")).unwrap(),
        "mine"
    );
    assert!(out.contains("colctl adopt claude skills/foo"), "{out}");
    // The other profile still got its link.
    assert!(fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok());
}

#[test]
fn ac_16_ec_19_removed_plugins_are_uninstalled_only_where_colctl_installed_them() {
    let env = two_profiles();
    // The kilo profile already has the plugin, installed by hand.
    let kilo = env.home.join(KILO);
    fs::create_dir_all(kilo.join("plugins")).unwrap();
    fs::write(
        kilo.join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{"p@anthropic-plugin-directory":[]}}"#,
    )
    .unwrap();
    let base = env.config();
    env.write_config(&format!(
        "{}\n[plugins]\nenabled = [\"p@anthropic-plugin-directory\"]\n",
        base.split("\n[plugins]").next().unwrap()
    ));
    env.ok(&["sync"]);
    assert_eq!(
        env.log_args(),
        vec!["plugin install p@anthropic-plugin-directory"]
    );

    // The hand-installed copy in kilo is now foreign (not in config, not colctl's), so exit 1.
    let out = env.expect(1, &["rm", "plugin", "p@anthropic-plugin-directory"]);
    assert!(
        out.contains("colctl adopt kilo-claude plugin p@anthropic-plugin-directory"),
        "{out}"
    );
    let args = env.log_args();
    assert_eq!(
        args.last().unwrap(),
        "plugin uninstall p@anthropic-plugin-directory"
    );
    assert_eq!(
        env.log()[1]["config_dir"],
        Value::Null,
        "only the native profile"
    );
    assert_eq!(env.installed(&kilo), vec!["p@anthropic-plugin-directory"]);
    assert!(env.installed(&env.home.join(".claude")).is_empty());
}

#[test]
fn ac_17_sync_commits_pending_source_changes_first() {
    let env = two_profiles();
    env.source_skill("foo");
    env.ok(&["sync"]);
    assert_eq!(env.commits()[0], "colctl: sync");
    let n = env.commits().len();
    env.ok(&["sync"]);
    assert_eq!(env.commits().len(), n, "clean source, no commit");
}

#[test]
fn ac_18_sync_can_target_one_harness() {
    let env = two_profiles();
    env.source_skill("foo");
    env.ok(&["sync", "--harness", "kilo-claude"]);
    assert!(fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok());
    assert!(fs::symlink_metadata(env.home.join(".claude/skills/foo")).is_err());
    let (code, _, err) = env.code(&["sync", "--harness", "nope"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("known harnesses: claude, kilo-claude"),
        "{err}"
    );
    assert_eq!(env.code(&["status", "--harness", "nope"]).0, 2);
}

#[test]
fn ac_19_dry_run_never_changes_anything() {
    let env = two_profiles();
    env.source_skill("pending");
    add_marketplace_and_plugins(&env);
    env.skill_dir(&env.home.join(".claude/skills"), "foreign", "x");
    let skill = env.skill_dir(&env.home, "new-skill", "x");
    let agent = env.home.join("new-agent.md");
    fs::write(&agent, "x").unwrap();
    env.source_skill("old");
    let before = env.snapshot();
    let sync = env.ok(&["sync", "--dry-run"]);
    assert!(
        sync.contains("would link") && sync.contains("would claude plugin install sadr@sadr"),
        "{sync}"
    );
    env.ok(&["add", "skill", skill.to_str().unwrap(), "--dry-run"]);
    env.ok(&["add", "agent", agent.to_str().unwrap(), "--dry-run"]);
    env.ok(&["add", "plugin", "x@anthropic-plugin-directory", "--dry-run"]);
    env.ok(&["rm", "skill", "old", "--dry-run"]);
    let adopt = env.ok(&["adopt", "--dry-run"]);
    assert!(
        adopt.contains("would adopt skills/foreign from claude"),
        "{adopt}"
    );
    env.ok(&["init", "--dry-run"]);
    assert_eq!(env.snapshot(), before);
    assert!(env.log().is_empty());
    assert!(!env.source.join(".state/lock").exists());
}

#[test]
fn ac_19_dry_run_says_nothing_to_do() {
    let env = two_profiles();
    assert!(env.ok(&["sync", "--dry-run"]).contains("nothing to do"));
}

// ---------------------------------------------------------------- add / rm

#[test]
fn ac_20_add_skill_from_a_folder() {
    let env = two_profiles();
    let skill = env.skill_dir(&env.home, "foo", "body");
    fs::create_dir_all(skill.join(".git")).unwrap();
    env.ok(&["add", "skill", skill.to_str().unwrap()]);
    assert_eq!(
        fs::read_to_string(env.source.join("skills/foo/SKILL.md")).unwrap(),
        "body"
    );
    assert!(!env.source.join("skills/foo/.git").exists());
    assert!(env.commits().contains(&"colctl: add skill foo".to_string()));
    assert!(
        fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok(),
        "sync ran"
    );

    let (code, _, err) = env.code(&["add", "skill", skill.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.contains("already managed"));
    let empty = env.home.join("empty");
    fs::create_dir_all(&empty).unwrap();
    let (code, _, err) = env.code(&["add", "skill", empty.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.contains("no SKILL.md"));
}

#[test]
fn ac_20_add_skill_from_a_git_url_with_subdir() {
    let env = two_profiles();
    let repo = env.home.join("repo");
    env.skill_dir(&repo.join(".claude/skills"), "forge", "forge body");
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "x",
        ],
    ] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let url = format!("file://{}", repo.display());
    env.ok(&["add", "skill", &url, "--subdir", ".claude/skills/forge"]);
    assert_eq!(
        fs::read_to_string(env.source.join("skills/forge/SKILL.md")).unwrap(),
        "forge body"
    );
    assert!(!env.source.join("skills/forge/.git").exists());
    assert!(fs::read_link(env.home.join(".claude/skills/forge")).is_ok());

    let (code, _, err) = env.code(&["add", "skill", "file:///no/such/repo", "--name", "x"]);
    assert_eq!(code, 2);
    assert!(err.contains("cloning"), "{err}");
}

#[test]
fn ec_08_add_skill_rejects_bad_names() {
    let env = two_profiles();
    let skill = env.skill_dir(&env.home, "fine", "x");
    let (code, _, err) = env.code(&[
        "add",
        "skill",
        skill.to_str().unwrap(),
        "--name",
        "../escape",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("invalid skill name"), "{err}");
    let (code, _, _) = env.code(&["add", "agent", "/tmp/bad name.md"]);
    assert_eq!(code, 2);
}

#[test]
fn ac_21_add_agent() {
    let env = two_profiles();
    let agent = env.home.join("reviewer.md");
    fs::write(&agent, "agent").unwrap();
    env.ok(&["add", "agent", agent.to_str().unwrap()]);
    assert_eq!(
        fs::read_to_string(env.source.join("agents/reviewer.md")).unwrap(),
        "agent"
    );
    assert!(
        env.commits()
            .contains(&"colctl: add agent reviewer".to_string())
    );
    assert!(fs::read_link(env.home.join(".claude/agents/reviewer.md")).is_ok());
    assert_eq!(
        env.code(&["add", "agent", agent.to_str().unwrap()]).0,
        2,
        "already managed"
    );
    assert_eq!(env.code(&["add", "agent", "/tmp/notes.txt"]).0, 2);
    assert_eq!(env.code(&["add", "agent", "/no/such.md"]).0, 2);
}

#[test]
fn ac_22_add_plugin() {
    let env = two_profiles();
    let before = env.config();
    env.ok(&[
        "add",
        "plugin",
        "sadr@sadr",
        "--marketplace",
        "github:muradmadi/sadr",
    ]);
    let config = env.config();
    assert!(config.starts_with(before.split("[marketplaces]").next().unwrap()));
    assert!(
        config.contains("sadr = \"github:muradmadi/sadr\"") && config.contains("\"sadr@sadr\""),
        "{config}"
    );
    assert!(
        env.commits()
            .contains(&"colctl: add plugin sadr@sadr".to_string())
    );
    assert_eq!(env.installed(&env.home.join(KILO)), vec!["sadr@sadr"]);

    for (args, needle) in [
        (&["add", "plugin", "foo@unknown"][..], "unknown marketplace"),
        (&["add", "plugin", "sadr@sadr"], "already enabled"),
        (&["add", "plugin", "nomarket"], "name@marketplace"),
        (
            &[
                "add",
                "plugin",
                "x@sadr",
                "--marketplace",
                "github:other/repo",
            ],
            "already github:muradmadi/sadr",
        ),
        (
            &["add", "plugin", "x@new", "--marketplace", "relative"],
            "must be github:owner/repo",
        ),
        (
            &[
                "add",
                "plugin",
                "x@anthropic-plugin-directory",
                "--marketplace",
                "/x",
            ],
            "needs no --marketplace",
        ),
    ] {
        let (code, _, err) = env.code(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(err.contains(needle), "{args:?}: {err}");
    }
    env.ok(&["add", "plugin", "y@sadr"]);
}

#[test]
fn ac_23_rm_skill_agent_plugin() {
    let env = two_profiles();
    let skill = env.skill_dir(&env.home, "foo", "x");
    env.ok(&["add", "skill", skill.to_str().unwrap()]);
    let agent = env.home.join("rev.md");
    fs::write(&agent, "x").unwrap();
    env.ok(&["add", "agent", agent.to_str().unwrap()]);
    env.ok(&["add", "plugin", "p@anthropic-plugin-directory"]);

    env.ok(&["rm", "skill", "foo"]);
    env.ok(&["rm", "agent", "rev"]);
    env.ok(&["rm", "plugin", "p@anthropic-plugin-directory"]);
    assert!(!env.source.join("skills/foo").exists());
    assert!(fs::symlink_metadata(env.home.join(".claude/skills/foo")).is_err());
    assert!(fs::symlink_metadata(env.home.join(KILO).join("agents/rev.md")).is_err());
    assert!(!env.config().contains("p@anthropic-plugin-directory"));
    let commits = env.commits();
    for msg in [
        "colctl: rm skill foo",
        "colctl: rm agent rev",
        "colctl: rm plugin p@anthropic-plugin-directory",
    ] {
        assert!(commits.contains(&msg.to_string()), "{msg}");
    }
    let (code, _, err) = env.code(&["rm", "skill", "nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("no managed skill named \"nope\""));
}

// ---------------------------------------------------------------- adopt

#[test]
fn ac_24_adopt_a_skill() {
    let env = two_profiles();
    let item = env.skill_dir(&env.home.join(".claude/skills"), "foo", "mine");
    fs::write(item.join("helper.sh"), "echo").unwrap();
    env.ok(&["adopt", "claude", "skills/foo"]);
    assert_eq!(
        fs::read_to_string(env.source.join("skills/foo/SKILL.md")).unwrap(),
        "mine"
    );
    assert!(env.source.join("skills/foo/helper.sh").is_file());
    assert_eq!(fs::read_link(&item).unwrap(), env.source.join("skills/foo"));
    assert!(
        env.commits()
            .contains(&"colctl: adopt skill foo".to_string())
    );
    assert!(
        fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok(),
        "sync spread it"
    );
}

#[test]
fn ac_25_adopt_with_an_existing_source_item() {
    let env = two_profiles();
    env.skill_dir(&env.source.join("skills"), "foo", "same");
    let same = env.skill_dir(&env.home.join(".claude/skills"), "foo", "same");
    let different = env.skill_dir(&env.home.join(KILO).join("skills"), "foo", "different");
    let out = env.expect(1, &["adopt"]);
    assert!(
        out.contains("linked (already identical) skills/foo from claude"),
        "{out}"
    );
    assert!(
        out.contains("refused skills/foo in kilo-claude: differs from the source copy"),
        "{out}"
    );
    assert!(fs::read_link(&same).is_ok());
    assert_eq!(
        fs::read_to_string(different.join("SKILL.md")).unwrap(),
        "different"
    );
}

#[test]
fn ac_26_adopt_claude_md() {
    let env = two_profiles();
    let native = env.home.join(".claude/CLAUDE.md");
    fs::write(&native, "my rules").unwrap();
    env.ok(&["adopt", "claude", "CLAUDE.md"]);
    assert_eq!(
        fs::read_to_string(env.source.join("AGENTS.md")).unwrap(),
        "my rules"
    );
    assert_eq!(fs::read_to_string(&native).unwrap(), GENERATED);
    assert_eq!(
        fs::read_to_string(env.home.join(KILO).join("CLAUDE.md")).unwrap(),
        GENERATED
    );
    assert!(
        env.commits()
            .contains(&"colctl: adopt instructions claude".to_string())
    );
    assert_eq!(
        env.item_state("claude", "instructions", "CLAUDE.md")
            .as_deref(),
        Some("ok")
    );

    // Different text with an AGENTS.md already in place is refused.
    let kilo_md = env.home.join(KILO).join("CLAUDE.md");
    fs::remove_file(&kilo_md).unwrap();
    let state = env.source.join(".state/state.json");
    let text = fs::read_to_string(&state)
        .unwrap()
        .replace("kilo-claude", "gone");
    fs::write(&state, text).unwrap();
    fs::write(&kilo_md, "other rules").unwrap();
    let out = env.expect(1, &["adopt", "kilo-claude", "CLAUDE.md"]);
    assert!(out.contains("already exists with different text"), "{out}");
    assert_eq!(fs::read_to_string(&kilo_md).unwrap(), "other rules");
}

#[test]
fn ac_27_adopt_a_plugin_and_its_marketplace() {
    // Claude names a marketplace after its marketplace.json, not its repo.
    let env = two_profiles().var("FAKE_CLAUDE_MARKET_NAME", "m");
    let kilo = env.home.join(KILO);
    fs::create_dir_all(kilo.join("plugins")).unwrap();
    fs::write(
        kilo.join("plugins/known_marketplaces.json"),
        r#"{"m":{"source":{"source":"github","repo":"owner/repo"}}}"#,
    )
    .unwrap();
    fs::write(
        kilo.join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{"p@m":[]}}"#,
    )
    .unwrap();
    env.ok(&["adopt", "kilo-claude", "plugin", "p@m"]);
    let config = env.config();
    assert!(
        config.contains("m = \"github:owner/repo\"") && config.contains("\"p@m\""),
        "{config}"
    );
    assert!(
        env.commits()
            .contains(&"colctl: adopt plugin p@m".to_string())
    );
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    let state: Value = serde_json::from_str(&state).unwrap();
    assert!(
        state["harnesses"]["kilo-claude"]["plugins"]
            .as_array()
            .is_none_or(|a| a.is_empty())
    );
    assert_eq!(
        env.installed(&env.home.join(".claude")),
        vec!["p@m"],
        "sync installed it in the other profile"
    );
}

#[test]
fn ac_27_unknown_marketplace_origin_is_refused() {
    let env = two_profiles();
    let kilo = env.home.join(KILO);
    fs::create_dir_all(kilo.join("plugins")).unwrap();
    fs::write(
        kilo.join("plugins/known_marketplaces.json"),
        r#"{"m":{"source":{"source":"npm"}}}"#,
    )
    .unwrap();
    fs::write(
        kilo.join("plugins/installed_plugins.json"),
        r#"{"plugins":{"p@m":[]}}"#,
    )
    .unwrap();
    let out = env.expect(1, &["adopt", "kilo-claude", "plugin", "p@m"]);
    assert!(
        out.contains("can't tell where marketplace \"m\" comes from"),
        "{out}"
    );
}

#[test]
fn ac_28_adopt_everything() {
    let env = two_profiles();
    let native = env.home.join(".claude");
    env.skill_dir(&native.join("skills"), "a", "a");
    env.skill_dir(&env.home.join(KILO).join("skills"), "a", "a");
    env.skill_dir(&native.join("skills"), "b", "b");
    fs::create_dir_all(native.join("agents")).unwrap();
    fs::write(native.join("agents/rev.md"), "rev").unwrap();
    fs::write(native.join("CLAUDE.md"), "rules").unwrap();
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        r#"{"plugins":{"sp@anthropic-plugin-directory":[]}}"#,
    )
    .unwrap();
    let out = env.ok(&["adopt"]);
    for line in [
        "adopted skills/a from claude",
        "adopted skills/b from claude",
        "adopted agents/rev.md from claude",
        "adopted CLAUDE.md from claude",
        "adopted plugin sp@anthropic-plugin-directory from claude",
        "linked (already identical) skills/a from kilo-claude",
    ] {
        assert!(out.contains(line), "{line:?} in:\n{out}");
    }
    // SC-6: after init + adopt, status is clean.
    env.ok(&["status"]);
}

#[test]
fn ec_14_ec_31_foreign_links_are_refused() {
    let env = two_profiles();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("skills")).unwrap();
    symlink("/somewhere/else", native.join("skills/mine")).unwrap();
    symlink("/somewhere/CLAUDE.md", native.join("CLAUDE.md")).unwrap();
    let out = env.expect(1, &["status"]);
    assert!(
        out.contains("remove or replace this link yourself"),
        "{out}"
    );
    // Changed in cycle 2 (AC2-25, AC2-28): adopt now takes over links into other folders, but a
    // dangling one like this is still refused.
    let out = env.expect(1, &["adopt", "claude", "skills/mine"]);
    assert!(
        out.contains(
            "refused skills/mine in claude: it links to /somewhere/else, which doesn't exist"
        ),
        "{out}"
    );
    let out = env.expect(1, &["adopt", "claude", "CLAUDE.md"]);
    assert!(out.contains("refused CLAUDE.md"), "{out}");
    assert_eq!(
        fs::read_link(native.join("skills/mine")).unwrap().to_str(),
        Some("/somewhere/else")
    );
}

#[test]
fn ec_30_adopt_a_plugin_directory_plugin() {
    let env = two_profiles();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        r#"{"plugins":{"superpowers@anthropic-plugin-directory":[]}}"#,
    )
    .unwrap();
    env.ok(&[
        "adopt",
        "claude",
        "plugin",
        "superpowers@anthropic-plugin-directory",
    ]);
    let config = env.config();
    assert!(config.contains("\"superpowers@anthropic-plugin-directory\""));
    assert!(!config.contains("anthropic-plugin-directory ="), "{config}");
}

#[test]
fn adopt_argument_errors() {
    let env = two_profiles();
    assert_eq!(env.code(&["adopt", "nope"]).0, 2);
    assert_eq!(env.code(&["adopt", "claude", "bogus"]).0, 2);
    let out = env.expect(1, &["adopt", "claude", "skills/absent"]);
    assert!(
        out.contains("refused skills/absent: not found in claude"),
        "{out}"
    );
    env.source_skill("ok");
    env.ok(&["sync"]);
    let out = env.expect(1, &["adopt", "claude", "skills/ok"]);
    assert!(out.contains("not a foreign item (state: ok)"), "{out}");
}

// ---------------------------------------------------------------- safety and environment

#[test]
fn ac_29_one_colctl_at_a_time() {
    let env = two_profiles();
    // Our own (running) test process holds the lock.
    fs::write(
        env.source.join(".state/lock"),
        std::process::id().to_string(),
    )
    .unwrap();
    let (code, _, err) = env.code(&["sync"]);
    assert_eq!(code, 3);
    assert!(
        err.contains(&format!("pid {}", std::process::id())),
        "{err}"
    );
    env.ok(&["status"]);
}

#[test]
fn ec_27_stale_lock_is_reclaimed() {
    let env = two_profiles();
    fs::write(env.source.join(".state/lock"), "999999999").unwrap();
    let (code, _, err) = env.code(&["sync"]);
    assert_eq!(code, 0);
    assert!(
        err.contains("reclaimed a stale lock from pid 999999999"),
        "{err}"
    );
    assert!(!env.source.join(".state/lock").exists());
}

#[test]
fn ac_30_claude_failures_are_reported_in_full() {
    let env = two_profiles().var("FAKE_CLAUDE_FAIL", "install sadr@sadr");
    add_marketplace_and_plugins(&env);
    env.source_skill("foo");
    let out = env.expect(1, &["sync"]);
    assert!(
        out.contains("✗ claude plugin install sadr@sadr — claude plugin install sadr@sadr exited 1: fake failure for: plugin install sadr@sadr"),
        "{out}"
    );
    assert!(out.contains("✓ claude plugin install superpowers@anthropic-plugin-directory"));
    assert!(fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok());
    let state = fs::read_to_string(env.source.join(".state/state.json")).unwrap();
    assert!(!state.contains("sadr@sadr"), "{state}");
}

#[test]
fn ac_31_claude_missing_still_syncs_files() {
    let env = Env::without_claude();
    env.native();
    env.ok(&["init"]);
    env.source_skill("foo");
    add_marketplace_and_plugins(&env);
    let out = env.expect(1, &["sync"]);
    assert!(
        out.contains("claude plugin install sadr@sadr — skipped: claude not found"),
        "{out}"
    );
    assert!(fs::read_link(env.home.join(".claude/skills/foo")).is_ok());
}

#[test]
fn ac_32_git_missing() {
    let env = Env::without_git();
    env.native();
    for args in [
        &["init"][..],
        &["sync"],
        &["adopt"],
        &["add", "plugin", "p@m"],
        &["rm", "skill", "x"],
    ] {
        let (code, _, err) = env.code(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(err.contains("colctl needs git"), "{err}");
    }
    assert!(!env.source.exists());
}

#[test]
fn ac_32_status_works_without_git() {
    let with_git = two_profiles();
    let env = Env::without_git();
    // Reuse a source made with git by pointing at it.
    let env = env.var("COLCTL_HOME", with_git.source.to_str().unwrap());
    let (code, out, err) = env.code(&["status"]);
    assert!(code == 0 || code == 1, "{out}{err}");
}

#[test]
fn ec_06_missing_harness_is_never_created() {
    let env = two_profiles();
    fs::remove_dir_all(env.home.join(KILO)).unwrap();
    env.source_skill("foo");
    let out = env.expect(1, &["sync"]);
    assert!(out.contains("harness-missing"), "{out}");
    assert!(!env.home.join(KILO).exists());
    assert!(fs::read_link(env.home.join(".claude/skills/foo")).is_ok());
}

#[test]
fn ec_07_harness_path_that_is_a_file_is_invalid() {
    let env = two_profiles();
    fs::remove_dir_all(env.home.join(KILO)).unwrap();
    fs::write(env.home.join(KILO), "not a folder").unwrap();
    let out = env.expect(1, &["status"]);
    assert!(
        out.contains("invalid → the path must be a real folder"),
        "{out}"
    );
}

#[test]
fn ec_11_marketplace_name_mismatch() {
    let env = two_profiles().var("FAKE_CLAUDE_MARKET_NAME", "official-name");
    add_marketplace_and_plugins(&env);
    let out = env.expect(1, &["sync", "--harness", "kilo-claude"]);
    assert!(
        out.contains("config calls it \"sadr\", the marketplace calls itself \"official-name\""),
        "{out}"
    );
    assert!(
        out.contains(
            "claude plugin install sadr@sadr — skipped: marketplace \"sadr\" wasn't added"
        ),
        "{out}"
    );
}

#[test]
fn ec_12_ec_13_plugin_files() {
    let env = two_profiles();
    env.ok(&["status"]);
    let kilo = env.home.join(KILO);
    fs::create_dir_all(kilo.join("plugins")).unwrap();
    fs::write(kilo.join("plugins/installed_plugins.json"), "{ nope").unwrap();
    add_marketplace_and_plugins(&env);
    let out = env.expect(1, &["sync"]);
    assert!(out.contains("doesn't parse"), "{out}");
    assert!(
        env.log().iter().all(|l| l["config_dir"].is_null()),
        "no plugin calls for the unreadable profile"
    );
}

#[test]
fn ec_15_edited_generated_claude_md_is_left_alone() {
    let env = two_profiles();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    env.ok(&["sync"]);
    let native = env.home.join(".claude/CLAUDE.md");
    fs::write(&native, "I edited this").unwrap();
    let out = env.expect(1, &["sync"]);
    assert_eq!(fs::read_to_string(&native).unwrap(), "I edited this");
    assert!(
        out.contains("move your text into ~/.colctl/AGENTS.md"),
        "{out}"
    );
}

#[test]
fn ec_17_missing_skills_folder_is_created() {
    let env = two_profiles();
    assert!(!env.home.join(KILO).join("skills").exists());
    env.source_skill("foo");
    env.ok(&["sync"]);
    let mode = fs::metadata(env.home.join(KILO).join("skills"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777 & !0o022, 0o755 & !0o022);
}

#[test]
fn ec_18_excluded_items() {
    let env = two_profiles();
    env.source_skill("foo");
    env.ok(&["sync"]);
    let config = env.config().replace(
        &format!("path = \"~/{KILO}\"\n"),
        &format!("path = \"~/{KILO}\"\nexclude = [\"skills/foo\", \"skills/mine\"]\n"),
    );
    env.write_config(&config);
    env.skill_dir(&env.home.join(KILO).join("skills"), "mine", "x");
    env.ok(&["sync"]);
    assert!(
        fs::symlink_metadata(env.home.join(KILO).join("skills/foo")).is_err(),
        "owned link removed"
    );
    assert!(env.home.join(KILO).join("skills/mine").is_dir());
    assert!(fs::read_link(env.home.join(".claude/skills/foo")).is_ok());
    env.ok(&["status"]);
}

#[test]
fn ec_21_leftover_temps_are_removed() {
    let env = two_profiles();
    let temp = env.home.join(".claude/.colctl-tmp-1-x");
    fs::write(&temp, "").unwrap();
    let src_temp = env.source.join("skills/.colctl-tmp-adopt-foo");
    fs::create_dir_all(&src_temp).unwrap();
    env.ok(&["sync"]);
    assert!(!temp.exists() && !src_temp.exists());
}

#[test]
fn ec_22_one_unwritable_harness_does_not_stop_the_other() {
    let env = two_profiles();
    let skills = env.home.join(".claude/skills");
    fs::create_dir_all(&skills).unwrap();
    fs::set_permissions(&skills, fs::Permissions::from_mode(0o555)).unwrap();
    env.source_skill("foo");
    let out = env.expect(1, &["sync"]);
    fs::set_permissions(&skills, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(out.contains("✗ link"), "{out}");
    assert!(fs::read_link(env.home.join(KILO).join("skills/foo")).is_ok());
}

#[test]
fn ec_23_corrupt_state() {
    let env = two_profiles();
    fs::write(env.source.join(".state/state.json"), "{ nope").unwrap();
    let (code, _, err) = env.code(&["status"]);
    assert_eq!(code, 2);
    assert!(err.contains("state is corrupt"), "{err}");
}

#[test]
fn ec_25_hung_claude_is_killed() {
    let env = two_profiles()
        .var("FAKE_CLAUDE_SLEEP", "5")
        .var("COLCTL_CLAUDE_TIMEOUT_SECS", "1");
    env.write_config(&format!(
        "{}\n[plugins]\nenabled = [\"p@anthropic-plugin-directory\"]\n",
        env.config().split("\n[plugins]").next().unwrap()
    ));
    let started = std::time::Instant::now();
    let out = env.expect(1, &["sync", "--harness", "kilo-claude"]);
    assert!(out.contains("timed out after 1s"), "{out}");
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
}

#[test]
fn ec_26_commits_without_git_identity() {
    let env = two_profiles();
    assert_eq!(
        env.git(&["log", "-1", "--format=%an <%ae>"]).trim(),
        "colctl <colctl@localhost>"
    );
}

#[test]
fn version_and_usage() {
    let env = Env::new();
    assert_eq!(
        env.ok(&["version"]).trim(),
        format!("colctl {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(env.ok(&["--version"]).contains(env!("CARGO_PKG_VERSION")));
    assert_eq!(env.code(&["bogus"]).0, 2);
    assert!(env.ok(&["--help"]).contains("Usage"));
    let mut cmd = env.cmd();
    cmd.env_remove("HOME");
    let out = cmd.arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
