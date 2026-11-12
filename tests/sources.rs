//! Cycle 3, sync side: source plugins cloned at their pin and installed from the clone into every
//! Claude and Codex profile. "Upstream" repos are local (file://). Named after AC3-NN / EC3-NN.

mod support;

use serde_json::Value;
use std::fs;
use support::{Env, KILO};

/// Native Claude, kilo Claude and native Codex, registered.
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

#[test]
fn ec3_01_no_sources_means_nothing_new() {
    let env = machine();
    env.ok(&["sync"]);
    assert!(!env.source.join("plugins").exists());
    assert!(env.log().is_empty() && env.codex_log().is_empty());
    assert_eq!(env.status_json()["sources"], serde_json::json!([]));
}

#[test]
fn ac3_04_sync_clones_at_the_pin() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.upstream_commit(&repo, "v2");
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let clone = env.source.join("plugins/superpowers");
    assert_eq!(head_of(&clone), v1);
    assert!(
        fs::read_to_string(env.source.join(".gitignore"))
            .unwrap()
            .lines()
            .any(|l| l == "plugins/")
    );
    assert!(
        !env.git(&["status", "--porcelain"]).contains("plugins"),
        "clones never enter the history"
    );
}

#[test]
fn ac3_05_a_moved_pin_moves_the_clone() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let v2 = env.upstream_commit(&repo, "v2");
    env.write_config(&env.config().replace(&v1, &v2));
    env.ok(&["sync"]);
    assert_eq!(head_of(&env.source.join("plugins/superpowers")), v2);
}

#[test]
fn ac3_06_a_dirty_clone_blocks_its_source() {
    let env = machine();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let calls = env.log().len() + env.codex_log().len();
    let clone = env.source.join("plugins/superpowers");
    fs::write(clone.join("README.md"), "my edit").unwrap();
    let out = env.expect(1, &["sync"]);
    assert!(out.contains("has local changes"), "{out}");
    assert_eq!(
        fs::read_to_string(clone.join("README.md")).unwrap(),
        "my edit"
    );
    assert_eq!(
        env.log().len() + env.codex_log().len(),
        calls,
        "nothing installed or updated"
    );
    assert_eq!(env.status_json()["sources"][0]["clone"], "dirty");
}

#[test]
fn ac3_07_ac3_10_installs_from_the_clone_with_the_repo_ids() {
    let env = machine();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let clone = env.source.join("plugins/superpowers").display().to_string();
    assert_eq!(
        env.log_args(),
        vec![
            format!("plugin marketplace add {clone}"),
            "plugin install superpowers@superpowers-dev".to_string(),
            format!("plugin marketplace add {clone}"),
            "plugin install superpowers@superpowers-dev".to_string(),
        ]
    );
    assert_eq!(
        env.codex_log_args(),
        vec![
            format!("plugin marketplace add {clone}"),
            "plugin add superpowers@superpowers-dev".to_string()
        ]
    );
    for h in ["claude", "kilo-claude", "codex"] {
        let s = &state(&env)["harnesses"][h]["sources"]["superpowers"];
        assert_eq!(s["id"], "superpowers@superpowers-dev", "{h}");
        assert_eq!(s["rev"], v1.as_str());
    }
    assert_eq!(
        env.item_state("codex", "source", "superpowers").as_deref(),
        Some("ok")
    );
    env.ok(&["sync"]);
    assert_eq!(env.log().len(), 4, "a second sync makes no calls");
    assert_eq!(env.codex_log().len(), 2);
}

#[test]
fn ac3_08_plugin_only_repos_install_through_a_colctl_manifest() {
    let env = machine();
    let (_, url, v1) = env.upstream("tool", true, true, None);
    env.add_source_entry("tool", &url, &v1, "");
    env.ok(&["sync"]);
    assert!(
        env.installed(&env.home.join(".claude"))
            .contains(&"tool@colctl-tool".to_string())
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["tool@colctl-tool"]
    );
    let manifests = env.source.join("plugins/.manifests/tool");
    assert!(manifests.join(".claude-plugin/marketplace.json").is_file());
    assert!(
        !env.source
            .join("plugins/tool/.claude-plugin/marketplace.json")
            .exists()
    );
}

#[test]
fn ac3_09_unsupported_tools_are_visible_but_not_drift() {
    let env = machine();
    let (_, url, v1) = env.upstream("clonly", true, false, Some("m"));
    env.add_source_entry("clonly", &url, &v1, "");
    env.ok(&["sync"]);
    assert!(env.codex_log().is_empty(), "no codex calls");
    let out = env.ok(&["status"]);
    assert!(
        out.contains("unsupported") && out.contains("the repo has no Codex packaging"),
        "{out}"
    );
    assert_eq!(
        env.item_state("codex", "source", "clonly").as_deref(),
        Some("unsupported")
    );
}

#[test]
fn ac3_11_a_moved_pin_updates_every_harness() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let v2 = env.upstream_commit(&repo, "v2");
    env.write_config(&env.config().replace(&v1, &v2));
    assert_eq!(
        env.item_state("claude", "source", "superpowers").as_deref(),
        Some("missing"),
        "clone not moved yet"
    );
    let (claude_before, codex_before) = (env.log().len(), env.codex_log().len());
    env.ok(&["sync"]);
    let claude_new: Vec<String> = env.log_args()[claude_before..].to_vec();
    assert_eq!(
        claude_new,
        vec![
            "plugin marketplace update superpowers-dev",
            "plugin update superpowers@superpowers-dev",
            "plugin marketplace update superpowers-dev",
            "plugin update superpowers@superpowers-dev",
        ]
    );
    assert_eq!(
        env.codex_log_args()[codex_before..].to_vec(),
        vec!["plugin add superpowers@superpowers-dev"]
    );
    assert_eq!(
        state(&env)["harnesses"]["codex"]["sources"]["superpowers"]["rev"],
        v2.as_str()
    );
    env.ok(&["status"]);
}

#[test]
fn ec3_13_a_failed_update_keeps_the_old_rev() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    let v2 = env.upstream_commit(&repo, "v2");
    env.write_config(&env.config().replace(&v1, &v2));
    let env = env.var("FAKE_CODEX_FAIL", "plugin add");
    let out = env.expect(1, &["sync"]);
    assert!(
        out.contains("codex plugin add superpowers@superpowers-dev exited 1"),
        "{out}"
    );
    assert_eq!(
        state(&env)["harnesses"]["codex"]["sources"]["superpowers"]["rev"],
        v1.as_str()
    );
    assert_eq!(
        env.item_state("codex", "source", "superpowers").as_deref(),
        Some("outdated")
    );
}

#[test]
fn ac3_12_ec3_10_replaces_uninstalls_the_aggregator_copy() {
    let env = machine();
    for profile in [env.home.join(".claude"), env.home.join(KILO)] {
        fs::create_dir_all(profile.join("plugins")).unwrap();
        fs::write(
            profile.join("plugins/installed_plugins.json"),
            r#"{"version":2,"plugins":{"superpowers@anthropic-plugin-directory":[{"scope":"user"}]}}"#,
        )
        .unwrap();
        fs::write(profile.join("plugins/known_marketplaces.json"), r#"{"claude-plugins-official":{"source":{"source":"github","repo":"anthropics/claude-plugins-official"}}}"#).unwrap();
    }
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry(
        "superpowers",
        &url,
        &v1,
        "replaces = [\"superpowers@anthropic-plugin-directory\"]\n",
    );
    env.ok(&["sync"]);
    let uninstalls: Vec<String> = env
        .log_args()
        .into_iter()
        .filter(|a| a.contains("uninstall"))
        .collect();
    assert_eq!(
        uninstalls,
        vec!["plugin uninstall superpowers@anthropic-plugin-directory"; 2]
    );
    for profile in [env.home.join(".claude"), env.home.join(KILO)] {
        let installed = env.installed(&profile);
        assert!(!installed.contains(&"superpowers@anthropic-plugin-directory".to_string()));
        let known = fs::read_to_string(profile.join("plugins/known_marketplaces.json")).unwrap();
        assert!(
            known.contains("claude-plugins-official"),
            "the marketplace stays"
        );
    }
    assert!(
        env.codex_log_args().iter().all(|a| !a.contains("remove")),
        "EC3-10: nothing to replace in codex"
    );
    env.ok(&["status"]);
}

#[test]
fn ac3_03_a_source_can_be_excluded_from_a_harness() {
    let env = machine();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    let config = env.config().replace(
        "path = \"~/.codex\"\n",
        "path = \"~/.codex\"\nexclude = [\"sources/superpowers\"]\n",
    );
    env.write_config(&config);
    env.ok(&["sync"]);
    assert!(env.codex_log().is_empty());
    assert_eq!(env.item_state("codex", "source", "superpowers"), None);
}

#[test]
fn ac3_23_plugins_from_an_aggregator_are_flagged() {
    let env = machine();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/installed_plugins.json"),
        r#"{"version":2,"plugins":{"superpowers@anthropic-plugin-directory":[{"scope":"user","gitCommitSha":"8ca22dba9a94f28898bbce59f2537ff4d87c747d"}]}}"#,
    )
    .unwrap();
    fs::write(
        native.join("plugins/plugin-directory-bindings.json"),
        r#"{"schemaVersion":1,"bindings":{"superpowers@anthropic-plugin-directory":{"repositoryKey":"github.com/obra/superpowers#"}}}"#,
    )
    .unwrap();
    let out = env.expect(1, &["status"]);
    assert!(
        out.contains("from-aggregator") && out.contains("source: https://github.com/obra/superpowers; colctl adopt claude plugin superpowers@anthropic-plugin-directory"),
        "{out}"
    );
}

#[test]
fn ac3_24_status_lists_sources() {
    let env = machine();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    let before = env.status_json();
    assert_eq!(before["sources"][0]["clone"], "missing");
    assert_eq!(
        env.item_state("claude", "source", "superpowers").as_deref(),
        Some("missing")
    );
    env.ok(&["sync"]);
    let v = env.status_json();
    assert_eq!(v["sources"][0]["name"], "superpowers");
    assert_eq!(v["sources"][0]["repo"], url.as_str());
    assert_eq!(v["sources"][0]["rev"], v1.as_str());
    assert_eq!(v["sources"][0]["clone"], "ok");
    assert_eq!(v["in_sync"], true);
}

#[test]
fn ac3_25_status_and_sync_need_no_upstream() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    fs::remove_dir_all(&repo).unwrap();
    env.ok(&["status"]);
    env.ok(&["sync"]);
}

#[test]
fn ec3_07_a_marketplace_name_taken_by_another_origin() {
    let env = machine();
    let native = env.home.join(".claude");
    fs::create_dir_all(native.join("plugins")).unwrap();
    fs::write(
        native.join("plugins/known_marketplaces.json"),
        r#"{"superpowers-dev":{"source":{"source":"github","repo":"obra/superpowers"},"installLocation":"/fake"}}"#,
    )
    .unwrap();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    let out = env.expect(1, &["sync"]);
    assert!(
        out.contains("marketplace superpowers-dev already comes from"),
        "{out}"
    );
    assert!(
        env.installed(&env.home.join(KILO))
            .contains(&"superpowers@superpowers-dev".to_string()),
        "other harnesses still get it"
    );
}

#[test]
fn ec3_12_a_source_id_that_is_also_a_list_plugin() {
    let env = machine();
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    let config = env
        .config()
        .replace(
            "[marketplaces]\n",
            "[marketplaces]\nsuperpowers-dev = \"github:obra/superpowers\"\n",
        )
        .replace(
            "enabled = []",
            "enabled = [\"superpowers@superpowers-dev\"]",
        );
    env.write_config(&config);
    let out = env.expect(1, &["sync"]);
    assert!(
        out.contains("superpowers@superpowers-dev is both a source plugin and a list plugin"),
        "{out}"
    );
}

#[test]
fn ac3_04_a_source_from_an_older_colctl_gets_plugins_ignored_and_committed() {
    let env = machine();
    // colctl 1.1 wrote only ".state/" into .gitignore.
    fs::write(env.source.join(".gitignore"), ".state/\n").unwrap();
    env.git(&["add", "-A"]);
    env.git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-qm",
        "old layout",
    ]);
    let (_, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.add_source_entry("superpowers", &url, &v1, "");
    env.ok(&["sync"]);
    assert!(
        fs::read_to_string(env.source.join(".gitignore"))
            .unwrap()
            .contains("plugins/")
    );
    assert!(
        env.git(&["status", "--porcelain"]).trim().is_empty(),
        "nothing left uncommitted"
    );
    env.ok(&["status"]);
}
