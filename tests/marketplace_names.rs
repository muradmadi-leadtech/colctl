//! HIL fix 2.6.1: every source plugin installs from colctl's own `colctl-<source>` marketplace, also
//! when its repo ships one. Named after ACH2-NN / ECH2-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::Env;

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}

/// A repo with its own Claude and Codex marketplaces named `superpowers-dev`, the plugin at its root,
/// added as a source on a machine with a Claude and a Codex profile.
fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.codex();
    env.ok(&["init"]);
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    write(
        &repo.join(".claude-plugin/marketplace.json"),
        &json!({"name": "superpowers-dev", "owner": {"name": "obra"}, "plugins": [
            {"name": "superpowers", "source": "./", "description": "Skills", "category": "dev"}]}),
    );
    env.upstream_commit(&repo, "v2");
    env.ok(&["add", "source", &url]);
    env
}

fn claude(env: &Env, args: &[&str]) {
    let ok = std::process::Command::new(env.bin.join("claude"))
        .env("HOME", &env.home)
        .env("FAKE_CLAUDE_LOG", &env.log)
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "claude {args:?}");
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn ach2_01_every_source_installs_from_colctls_own_marketplace() {
    let env = machine();
    let claude = env.installed(&env.home.join(".claude"));
    assert!(
        claude.contains(&"superpowers@colctl-superpowers".to_string()),
        "{claude:?}"
    );
    assert!(
        !claude.iter().any(|i| i.ends_with("@superpowers-dev")),
        "{claude:?}"
    );
    let codex = env.codex_installed(&env.home.join(".codex"));
    assert!(
        codex.contains(&"superpowers@colctl-superpowers".to_string()),
        "{codex:?}"
    );
    let manifests = env.source.join("plugins/.manifests/superpowers");
    let m = read(&manifests.join(".claude-plugin/marketplace.json"));
    assert_eq!(m["name"], "colctl-superpowers");
    let entry = &m["plugins"][0];
    assert_eq!(entry["name"], "superpowers");
    assert_eq!(entry["source"], "./plugin");
    assert_eq!(entry["description"], "Skills", "the repo's fields are kept");
    assert_eq!(entry["category"], "dev");
    // ECH3-01: a copy of the clone, not a link into it, so no plugin path resolves outside.
    let plugin = manifests.join("plugin");
    assert!(!fs::symlink_metadata(&plugin).unwrap().is_symlink());
    assert!(plugin.join(".claude-plugin/marketplace.json").is_file());
    assert!(
        !plugin.join(".git").exists(),
        "the clone's git folder isn't copied"
    );
    assert_eq!(
        env.item_state("claude", "source", "superpowers").as_deref(),
        Some("ok")
    );
}

#[test]
fn ach2_02_an_install_under_the_repos_name_moves_once() {
    let env = machine();
    // Rewind the Claude profile to how 2.6.0 installed it: from the clone, under the repo's name.
    let clone = env.source.join("plugins/superpowers");
    claude(
        &env,
        &["plugin", "uninstall", "superpowers@colctl-superpowers"],
    );
    claude(
        &env,
        &["plugin", "marketplace", "remove", "colctl-superpowers"],
    );
    claude(
        &env,
        &["plugin", "marketplace", "add", clone.to_str().unwrap()],
    );
    claude(&env, &["plugin", "install", "superpowers@superpowers-dev"]);
    let state_path = env.source.join(".state/state.json");
    let mut state = read(&state_path);
    let h = &mut state["harnesses"]["claude"];
    h["sources"]["superpowers"]["id"] = json!("superpowers@superpowers-dev");
    h["sources"]["superpowers"]["marketplace"] = json!("superpowers-dev");
    h["marketplaces"] = json!(["superpowers-dev"]);
    fs::write(&state_path, state.to_string()).unwrap();

    env.ok(&["sync"]);
    let installed = env.installed(&env.home.join(".claude"));
    assert!(
        installed.contains(&"superpowers@colctl-superpowers".to_string()),
        "{installed:?}"
    );
    assert!(
        !installed.contains(&"superpowers@superpowers-dev".to_string()),
        "{installed:?}"
    );
    let known = read(&env.home.join(".claude/plugins/known_marketplaces.json"));
    assert!(known.get("superpowers-dev").is_none(), "{known}");
    assert_eq!(env.code(&["status"]).0, 0);
    let again = env.ok(&["sync"]);
    assert!(again.contains("Everything was already in sync"), "{again}");
}

#[test]
fn ach2_03_the_docs_say_so() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(root.join("docs/reference/config.md")).unwrap();
    assert!(config.contains("colctl-<name>"), "config.md");
    let why = fs::read_to_string(root.join("docs/explanation/plugins-from-source.md")).unwrap();
    assert!(
        why.contains("always `colctl-<name>`"),
        "plugins-from-source.md"
    );
}

#[test]
fn ech2_03_a_list_plugin_keeps_the_repos_marketplace() {
    let env = machine();
    let clone = env.source.join("plugins/superpowers");
    let config = env
        .config()
        .replace(
            "[plugins]\nenabled = []",
            "[plugins]\nenabled = [\"other@superpowers-dev\"]",
        )
        .replace(
            "[marketplaces]\n",
            &format!(
                "[marketplaces]\nsuperpowers-dev = \"{}\"\n",
                clone.display()
            ),
        );
    env.write_config(&config);
    let (_, out, err) = env.code(&["status", "--all", "--color", "never"]);
    assert!(out.contains("other@superpowers-dev"), "{out}{err}");
}

#[test]
fn ech2_04_the_old_marketplace_stays_while_a_list_plugin_uses_it() {
    let env = machine();
    let clone = env.source.join("plugins/superpowers");
    claude(
        &env,
        &["plugin", "uninstall", "superpowers@colctl-superpowers"],
    );
    claude(
        &env,
        &["plugin", "marketplace", "remove", "colctl-superpowers"],
    );
    claude(
        &env,
        &["plugin", "marketplace", "add", clone.to_str().unwrap()],
    );
    claude(&env, &["plugin", "install", "superpowers@superpowers-dev"]);
    let state_path = env.source.join(".state/state.json");
    let mut state = read(&state_path);
    let h = &mut state["harnesses"]["claude"];
    h["sources"]["superpowers"]["id"] = json!("superpowers@superpowers-dev");
    h["sources"]["superpowers"]["marketplace"] = json!("superpowers-dev");
    fs::write(&state_path, state.to_string()).unwrap();
    let config = env.config().replace(
        "[marketplaces]\n",
        &format!(
            "[marketplaces]\nsuperpowers-dev = \"{}\"\n",
            clone.display()
        ),
    );
    env.write_config(&config);
    env.run(&["sync"]);
    let known = read(&env.home.join(".claude/plugins/known_marketplaces.json"));
    assert!(
        known.get("superpowers-dev").is_some(),
        "kept for the list: {known}"
    );
}
