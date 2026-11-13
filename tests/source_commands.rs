//! Cycle 3, command side: add source, rm source, update, and adopt moving a plugin to its source.
//! "github.com" is mapped to local repos through the temp home's git config, so nothing touches
//! the network. Named after AC3-NN / EC3-NN.

mod support;

use serde_json::Value;
use std::fs;
use support::{Env, KILO};

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

fn head_of(dir: &std::path::Path) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn sources(env: &Env) -> Vec<Value> {
    let t: toml::Table = toml::from_str(&env.config()).unwrap();
    t.get("source")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().map(|v| serde_json::to_value(v).unwrap()).collect())
        .unwrap_or_default()
}

/// A Claude profile with a plugin installed from Anthropic's plugin directory at `sha`.
fn directory_plugin(profile: &std::path::Path, id: &str, key: &str, sha: &str) {
    fs::create_dir_all(profile.join("plugins")).unwrap();
    fs::write(
        profile.join("plugins/installed_plugins.json"),
        serde_json::json!({"version": 2, "plugins": {id: [{"scope": "user", "gitCommitSha": sha}]}}).to_string(),
    )
    .unwrap();
    fs::write(
        profile.join("plugins/plugin-directory-bindings.json"),
        serde_json::json!({"schemaVersion": 1, "bindings": {id: {"repositoryKey": key}}})
            .to_string(),
    )
    .unwrap();
}

// ---------------------------------------------------------------- add / rm

#[test]
fn ac3_17_add_source() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    let v2 = env.upstream_commit(&repo, "v2");
    env.ok(&["add", "source", &url]);
    let s = &sources(&env)[0];
    assert_eq!(s["name"], "superpowers");
    assert_eq!(s["rev"], v2.as_str(), "pinned to the default-branch head");
    assert!(
        env.commits()
            .contains(&"colctl: add source superpowers".to_string())
    );
    assert!(
        env.installed(&env.home.join(".claude"))
            .contains(&"superpowers@superpowers-dev".to_string())
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["superpowers@superpowers-dev"]
    );

    let (_, url2, _) = env.upstream("other", true, true, Some("other-m"));
    env.ok(&[
        "add",
        "source",
        &url2,
        "--rev",
        &head_of(&env.home.parent().unwrap().join("upstreams/other")),
        "--name",
        "pinned",
    ]);
    assert_eq!(sources(&env)[1]["name"], "pinned");
    let _ = v1;
}

#[test]
fn ac3_17_ec3_04_add_source_without_packaging_is_refused() {
    let env = machine();
    let (_, url, _) = env.upstream("bare", false, false, None);
    let (code, _, err) = env.code(&["add", "source", &url]);
    assert_eq!(code, 2);
    assert!(err.contains("no Claude or Codex packaging"), "{err}");
    assert!(!env.source.join("plugins/bare").exists());
    assert!(sources(&env).is_empty());
    assert_eq!(env.code(&["add", "source", "relative/path"]).0, 2);
}

#[test]
fn ac3_13_rm_source() {
    let env = machine();
    let (_, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    env.ok(&["rm", "source", "superpowers"]);
    assert!(
        env.commits()
            .contains(&"colctl: rm source superpowers".to_string())
    );
    assert!(sources(&env).is_empty());
    let claude = env.log_args();
    assert!(
        claude.contains(&"plugin uninstall superpowers@superpowers-dev".to_string()),
        "{claude:?}"
    );
    assert!(claude.contains(&"plugin marketplace remove superpowers-dev".to_string()));
    assert!(
        env.codex_log_args()
            .contains(&"plugin remove superpowers@superpowers-dev".to_string())
    );
    assert!(!env.source.join("plugins/superpowers").exists());
    assert!(env.codex_installed(&env.home.join(".codex")).is_empty());
    assert_eq!(state(&env)["harnesses"]["codex"]["sources"], Value::Null);
    assert_eq!(env.code(&["rm", "source", "nope"]).0, 2);
}

// ---------------------------------------------------------------- update

#[test]
fn ac3_14_update_check_reports_and_changes_nothing() {
    let env = machine();
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    let (_, url2, _) = env.upstream("other", true, true, Some("other-m"));
    env.ok(&["add", "source", &url]);
    env.ok(&["add", "source", &url2]);
    let pinned = head_of(&env.source.join("plugins/superpowers"));
    env.upstream_commit(&repo, "v2");
    let new = env.upstream_commit(&repo, "v3");
    let config = env.config();
    let calls = (env.log().len(), env.codex_log().len());
    let out = env.expect(1, &["update", "--check"]);
    assert!(
        out.contains(&format!(
            "superpowers: {} → {} (2 new commits)",
            &pinned[..7],
            &new[..7]
        )),
        "{out}"
    );
    assert!(out.contains("other: up to date"), "{out}");
    assert_eq!(env.config(), config);
    assert_eq!(head_of(&env.source.join("plugins/superpowers")), pinned);
    assert_eq!((env.log().len(), env.codex_log().len()), calls);
    env.ok(&["update", "--check", "other"]);
}

#[test]
fn ac3_15_update_moves_pins_and_every_harness() {
    // Cycle 4: update holds the new commit; the pin and the harnesses move only on approve.
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    let new = env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    assert_eq!(sources(&env)[0]["rev"], v1.as_str());
    assert_eq!(head_of(&env.source.join("plugins/superpowers")), v1);
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        new.as_str()
    );
    assert!(
        !env.log_args()
            .contains(&"plugin update superpowers@superpowers-dev".to_string())
    );
    let out = env.ok(&["update", "superpowers"]);
    assert!(out.contains("already held for review"), "{out}");
    // Approving it moves the pin and every harness, as update did in 1.2.
    env.ok(&["approve", "superpowers"]);
    assert_eq!(sources(&env)[0]["rev"], new.as_str());
    assert_eq!(head_of(&env.source.join("plugins/superpowers")), new);
    assert_eq!(
        state(&env)["harnesses"]["codex"]["sources"]["superpowers"]["rev"],
        new.as_str()
    );
    assert!(
        env.log_args()
            .contains(&"plugin update superpowers@superpowers-dev".to_string())
    );
    let out = env.ok(&["update", "superpowers"]);
    assert!(out.contains("up to date"), "{out}");
}

#[test]
fn ac3_16_update_rev() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    let tag = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["tag", "v1.0", &v1])
        .status()
        .unwrap();
    assert!(tag.success());
    env.upstream_commit(&repo, "v2");
    env.ok(&["add", "source", &url]);
    env.ok(&["update", "superpowers", "--rev", "v1.0"]);
    // Cycle 4: --rev holds the commit instead of pinning it.
    assert_eq!(
        state(&env)["quarantine"]["sources/superpowers"]["to"],
        v1.as_str()
    );
    for args in [
        &["update", "--rev", "v1.0"][..],
        &["update", "a", "b", "--rev", "v1.0"],
        &["update", "superpowers", "--rev", "no-such"],
        &["update", "nope"],
    ] {
        assert_eq!(env.code(args).0, 2, "{args:?}");
    }
}

#[test]
fn ec3_15_update_cannot_reach_upstream() {
    let env = machine();
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    let pinned = sources(&env)[0]["rev"].clone();
    fs::remove_dir_all(&repo).unwrap();
    let out = env.expect(1, &["update"]);
    assert!(
        out.contains("superpowers:") && out.contains("fetch"),
        "{out}"
    );
    assert_eq!(sources(&env)[0]["rev"], pinned);
}

// ---------------------------------------------------------------- adopt → source

#[test]
fn ac3_18_adopt_a_plugin_directory_plugin_moves_it_to_its_source() {
    let env = machine();
    let (repo, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    env.upstream_commit(&repo, "v2");
    let native = env.home.join(".claude");
    directory_plugin(
        &native,
        "superpowers@anthropic-plugin-directory",
        "github.com/obra/superpowers#",
        &v1,
    );
    env.ok(&[
        "adopt",
        "claude",
        "plugin",
        "superpowers@anthropic-plugin-directory",
    ]);
    let s = &sources(&env)[0];
    assert_eq!(s["name"], "superpowers");
    assert_eq!(s["repo"], "https://github.com/obra/superpowers");
    assert_eq!(
        s["rev"],
        v1.as_str(),
        "pinned to the installed commit, not the newest"
    );
    assert_eq!(
        s["replaces"],
        serde_json::json!(["superpowers@anthropic-plugin-directory"])
    );
    assert!(env.commits().contains(
        &"colctl: adopt plugin superpowers@anthropic-plugin-directory from source".to_string()
    ));
    for profile in [native.clone(), env.home.join(KILO)] {
        let installed = env.installed(&profile);
        assert!(
            installed.contains(&"superpowers@superpowers-dev".to_string()),
            "{installed:?}"
        );
        assert!(!installed.contains(&"superpowers@anthropic-plugin-directory".to_string()));
    }
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["superpowers@superpowers-dev"]
    );
    assert_eq!(
        state(&env)["harnesses"]["codex"]["sources"]["superpowers"]["rev"],
        v1.as_str()
    );
    env.ok(&["status"]);
}

#[test]
fn ac3_19_adopt_a_marketplace_plugin_from_inside_the_marketplace_repo() {
    let env = machine();
    let (repo, v1) = env.github_repo("author/tools", "tool", "tools-m");
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/known_marketplaces.json"),
        serde_json::json!({"tools-m": {"source": {"source": "github", "repo": "author/tools"}, "installLocation": repo}}).to_string(),
    )
    .unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        serde_json::json!({"version": 2, "plugins": {"tool@tools-m": [{"scope": "user"}]}})
            .to_string(),
    )
    .unwrap();
    env.ok(&["adopt", "claude", "plugin", "tool@tools-m"]);
    let s = &sources(&env)[0];
    assert_eq!(s["repo"], "https://github.com/author/tools");
    assert_eq!(s["rev"], v1.as_str(), "the marketplace checkout's commit");
    assert_eq!(s["plugin"].as_str().unwrap_or("tool"), "tool");
}

#[test]
fn ac3_20_adopt_a_codex_plugin() {
    let env = machine();
    let (repo, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    env.upstream_commit(&repo, "v2");
    std::os::unix::fs::symlink(&repo, repo.with_file_name("superpowers.git")).unwrap();
    let codex = env.home.join(".codex");
    let root = codex.join(".tmp/marketplaces/superpowers-dev");
    let clone = std::process::Command::new("git")
        .args(["clone", "-q", &format!("file://{}", repo.display())])
        .arg(&root)
        .status()
        .unwrap();
    assert!(clone.success());
    std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["checkout", "-q", "--detach", &v1])
        .status()
        .unwrap();
    fs::write(
        codex.join("config.toml"),
        "[marketplaces.superpowers-dev]\nsource_type = \"git\"\nsource = \"https://github.com/obra/superpowers.git\"\n\n[plugins.\"superpowers@superpowers-dev\"]\nenabled = true\n",
    )
    .unwrap();
    env.ok(&["adopt", "codex", "plugin", "superpowers@superpowers-dev"]);
    let s = &sources(&env)[0];
    assert_eq!(s["repo"], "https://github.com/obra/superpowers.git");
    assert_eq!(s["rev"], v1.as_str());
}

#[test]
fn ac3_21_the_same_source_from_another_aggregator_joins_replaces() {
    let env = machine();
    let (repo, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    directory_plugin(
        &env.home.join(".claude"),
        "superpowers@anthropic-plugin-directory",
        "github.com/obra/superpowers#",
        &v1,
    );
    // kilo-claude has it from another marketplace whose manifest points at the same repo.
    let kilo = env.home.join(KILO);
    let agg = env.home.parent().unwrap().join("aggregator");
    fs::create_dir_all(agg.join(".claude-plugin")).unwrap();
    fs::write(
        agg.join(".claude-plugin/marketplace.json"),
        serde_json::json!({"name": "agg", "plugins": [{"name": "superpowers", "source": {"source": "github", "repo": "obra/superpowers"}}]}).to_string(),
    )
    .unwrap();
    fs::create_dir_all(kilo.join("plugins")).unwrap();
    fs::write(
        kilo.join("plugins/known_marketplaces.json"),
        serde_json::json!({"agg": {"source": {"source": "directory", "path": agg}, "installLocation": agg}}).to_string(),
    )
    .unwrap();
    fs::write(
        kilo.join("plugins/installed_plugins.json"),
        serde_json::json!({"version": 2, "plugins": {"superpowers@agg": [{"scope": "user", "gitCommitSha": v1}]}}).to_string(),
    )
    .unwrap();
    env.ok(&["adopt"]);
    let all = sources(&env);
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(
        all[0]["replaces"],
        serde_json::json!(["superpowers@anthropic-plugin-directory", "superpowers@agg"])
    );
    assert!(
        !env.installed(&kilo)
            .contains(&"superpowers@agg".to_string())
    );
    let _ = repo;
}

#[test]
fn ac3_22_no_source_found_falls_back_to_the_list() {
    let env = machine();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{"mystery@anthropic-plugin-directory":[{"scope":"user"}]}}"#,
    )
    .unwrap();
    env.ok(&[
        "adopt",
        "claude",
        "plugin",
        "mystery@anthropic-plugin-directory",
    ]);
    assert!(sources(&env).is_empty());
    assert!(
        env.config()
            .contains("\"mystery@anthropic-plugin-directory\"")
    );
}

#[test]
fn ec3_18_unknown_installed_commit_pins_the_head() {
    let env = machine();
    let (repo, _) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    std::os::unix::fs::symlink(&repo, repo.with_file_name("superpowers.git")).unwrap();
    let head = env.upstream_commit(&repo, "v2");
    // A plain-folder (not git) Codex marketplace whose manifest points at the plugin's repo.
    let local = env.home.parent().unwrap().join("local-market");
    fs::create_dir_all(local.join(".agents/plugins")).unwrap();
    fs::write(
        local.join(".agents/plugins/marketplace.json"),
        serde_json::json!({"name": "loc", "plugins": [{"name": "superpowers", "source": {"source": "url", "url": "https://github.com/obra/superpowers.git"}}]}).to_string(),
    )
    .unwrap();
    fs::write(
        env.home.join(".codex/config.toml"),
        format!("[marketplaces.loc]\nsource_type = \"local\"\nsource = \"{}\"\n\n[plugins.\"superpowers@loc\"]\nenabled = true\n", local.display()),
    )
    .unwrap();
    let out = env.ok(&["adopt", "codex", "plugin", "superpowers@loc"]);
    assert!(out.contains("installed commit unknown; pinned to"), "{out}");
    assert_eq!(sources(&env)[0]["rev"], head.as_str());
}

/// superpowers in [plugins] (adopted in cycle 1), installed by hand in the native profile and by colctl in kilo.
fn listed_superpowers(env: &Env, v1: &str) {
    directory_plugin(
        &env.home.join(".claude"),
        "superpowers@anthropic-plugin-directory",
        "github.com/obra/superpowers#",
        v1,
    );
    env.write_config(&env.config().replace(
        "enabled = []",
        "enabled = [\"superpowers@anthropic-plugin-directory\"]",
    ));
    env.ok(&["sync"]);
    assert!(
        env.installed(&env.home.join(KILO))
            .contains(&"superpowers@anthropic-plugin-directory".to_string())
    );
}

#[test]
fn ac3_30_status_says_a_list_plugin_has_a_source() {
    let env = machine();
    let (_, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    listed_superpowers(&env, &v1);
    let out = env.ok(&["status", "--all"]);
    assert!(
        // Cycle 4: list plugins are "unreviewed"; the source hint stays. Cycle 6: listed with --all.
        Env::words(&out).contains("· unreviewed plugin superpowers@anthropic-plugin-directory → updates come from anthropic-plugin-directory, not through colctl review; source available: https://github.com/obra/superpowers; colctl adopt claude plugin superpowers@anthropic-plugin-directory to hold them"),
        "{out}"
    );
}

#[test]
fn ac3_29_adopt_converts_a_list_plugin_in_one_step() {
    let env = machine();
    let (_, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    listed_superpowers(&env, &v1);
    let commits = env.commits().len();
    env.ok(&[
        "adopt",
        "claude",
        "plugin",
        "superpowers@anthropic-plugin-directory",
    ]);
    assert_eq!(
        env.commits().len(),
        commits + 1,
        "one commit: source added and list entry removed together"
    );
    assert!(
        !env.config()
            .contains("enabled = [\"superpowers@anthropic-plugin-directory\"]")
    );
    let s = &sources(&env)[0];
    assert_eq!(s["rev"], v1.as_str());
    assert_eq!(
        s["replaces"],
        serde_json::json!(["superpowers@anthropic-plugin-directory"])
    );
    for profile in [env.home.join(".claude"), env.home.join(KILO)] {
        let installed = env.installed(&profile);
        assert!(
            installed.contains(&"superpowers@superpowers-dev".to_string()),
            "{installed:?}"
        );
        assert!(!installed.contains(&"superpowers@anthropic-plugin-directory".to_string()));
    }
    let uninstalls = env
        .log_args()
        .iter()
        .filter(|a| a.as_str() == "plugin uninstall superpowers@anthropic-plugin-directory")
        .count();
    assert_eq!(uninstalls, 2, "once per harness");
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["superpowers@superpowers-dev"]
    );
    env.ok(&["status"]);
}

#[test]
fn ac3_29_adopt_all_converts_list_plugins_too() {
    let env = machine();
    let (_, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    listed_superpowers(&env, &v1);
    let out = env.ok(&["adopt"]);
    assert!(
        out.contains("adopted plugin superpowers@anthropic-plugin-directory"),
        "{out}"
    );
    assert_eq!(sources(&env).len(), 1);
}

#[test]
fn ec3_19_a_list_plugin_without_a_known_source() {
    let env = machine();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{"mystery@anthropic-plugin-directory":[{"scope":"user"}]}}"#,
    )
    .unwrap();
    env.write_config(&env.config().replace(
        "enabled = []",
        "enabled = [\"mystery@anthropic-plugin-directory\"]",
    ));
    env.ok(&["sync"]);
    let out = env.ok(&["status"]);
    assert!(!out.contains("source available"), "{out}");
    let out = env.expect(
        1,
        &[
            "adopt",
            "claude",
            "plugin",
            "mystery@anthropic-plugin-directory",
        ],
    );
    assert!(
        out.contains("not a foreign item (state: unreviewed)"),
        "{out}"
    );
}
