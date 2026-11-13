//! Cycle 6, T0: a source plugin always installs from its pinned clone, even when the repo's own
//! marketplace manifest points somewhere else (databricks' manifests send both tools to GitHub `main`).
//! Named after AC6-31 – 33 and EC6-21 – 22.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.codex();
    env.ok(&["init"]);
    env
}

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Writes the manifests of a databricks-style repo: per-tool plugin folders, and marketplace entries
/// that are local (`./plugins/dbx/<tool>`) or remote (git-subdir on GitHub).
fn manifests(repo: &Path, remote: bool) {
    let entry = |tool: &str| {
        if remote {
            json!({"name": "dbx", "source": {"source": "git-subdir",
                "url": "https://github.com/databricks/dbx.git", "path": format!("plugins/dbx/{tool}"), "ref": "main"}})
        } else if tool == "claude" {
            json!({"name": "dbx", "source": "./plugins/dbx/claude"})
        } else {
            json!({"name": "dbx", "source": {"source": "url", "url": "./plugins/dbx/codex"}})
        }
    };
    write(
        &repo.join(".claude-plugin/marketplace.json"),
        &json!({"name": "dbx-skills", "plugins": [entry("claude")]}),
    );
    write(
        &repo.join(".agents/plugins/marketplace.json"),
        &json!({"name": "dbx-skills", "plugins": [entry("codex")]}),
    );
    write(
        &repo.join("plugins/dbx/claude/.claude-plugin/plugin.json"),
        &json!({"name": "dbx", "version": "1.0.0"}),
    );
    write(
        &repo.join("plugins/dbx/codex/.codex-plugin/plugin.json"),
        &json!({"name": "dbx", "version": "1.0.0"}),
    );
}

/// A local "upstream" repo with that layout; returns its file:// URL.
fn dbx_repo(env: &Env, remote: bool) -> (std::path::PathBuf, String) {
    let repo = env.home.parent().unwrap().join("upstreams/dbx");
    manifests(&repo, remote);
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "v1"]);
    let url = format!("file://{}", repo.display());
    (repo, url)
}

fn known_marketplaces(profile: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(profile.join("plugins/known_marketplaces.json")).unwrap_or("{}".into()),
    )
    .unwrap()
}

#[test]
fn ac6_31_a_repo_manifest_pointing_at_github_installs_from_the_clone() {
    let env = machine();
    let (_, url) = dbx_repo(&env, true);
    env.ok(&["add", "source", &url, "--name", "dbx"]);
    let claude = env.home.join(".claude");
    assert_eq!(env.installed(&claude), vec!["dbx@colctl-dbx"]);
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["dbx@colctl-dbx"]
    );
    let market = &known_marketplaces(&claude)["colctl-dbx"];
    assert!(
        market["installLocation"]
            .as_str()
            .unwrap()
            .ends_with("plugins/.manifests/dbx"),
        "{market}"
    );
    // Converged: a second sync has nothing to do and status is clean.
    let calls = (env.log().len(), env.codex_log().len());
    env.ok(&["sync"]);
    assert_eq!((env.log().len(), env.codex_log().len()), calls);
    env.ok(&["status"]);
}

#[test]
fn ac6_32_installs_made_from_the_repo_manifest_move_to_the_clone() {
    let env = machine();
    // 2.1 behaviour: a manifest pointing inside the repo is used as is.
    let (repo, url) = dbx_repo(&env, false);
    env.ok(&["add", "source", &url, "--name", "dbx"]);
    let claude = env.home.join(".claude");
    assert_eq!(env.installed(&claude), vec!["dbx@dbx-skills"]);
    // The repo then switches its manifests to GitHub, and the owner approves that version.
    manifests(&repo, true);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "v2"]);
    env.ok(&["update"]);
    env.ok(&["approve", "dbx"]);
    assert_eq!(env.installed(&claude), vec!["dbx@colctl-dbx"]);
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["dbx@colctl-dbx"]
    );
    assert!(known_marketplaces(&claude).get("dbx-skills").is_none());
    let log = env.log_args();
    assert!(
        log.contains(&"plugin uninstall dbx@dbx-skills".to_string()),
        "{log:?}"
    );
    assert!(
        log.contains(&"plugin marketplace remove dbx-skills".to_string()),
        "{log:?}"
    );
    env.ok(&["status"]);
}

#[test]
fn ac6_33_a_source_that_does_not_converge_fails() {
    let env = machine();
    let (_, url) = dbx_repo(&env, false);
    // Claude records some other commit than the clone's, as it did for databricks.
    let env = env.var("FAKE_CLAUDE_SHA", &"d".repeat(40));
    let (code, out, _) = env.code(&["add", "source", &url, "--name", "dbx"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("is still at ddddddd after installing"),
        "{out}"
    );
    // Once the tool installs the clone, the next sync converges.
    let mut env = env;
    env.vars.retain(|(k, _)| k != "FAKE_CLAUDE_SHA");
    env.ok(&["sync"]);
    env.ok(&["status"]);
}

#[test]
fn ec6_22_a_wrapper_marketplace_name_taken_by_something_else() {
    let env = machine();
    let claude = env.home.join(".claude");
    write(
        &claude.join("plugins/known_marketplaces.json"),
        &json!({"colctl-dbx": {"source": {"source": "github", "repo": "someone/else"},
                               "installLocation": "/elsewhere"}}),
    );
    let (_, url) = dbx_repo(&env, true);
    let (code, out, _) = env.code(&["add", "source", &url, "--name", "dbx"]);
    assert_eq!(code, 1, "{out}");
    let v = env.status_json();
    let item = v["harnesses"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "dbx")
        .unwrap()
        .clone();
    assert_eq!(item["state"], "invalid", "{item}");
    assert!(
        item["hint"]
            .as_str()
            .unwrap()
            .contains("marketplace colctl-dbx already comes from"),
        "{item}"
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["dbx@colctl-dbx"],
        "the other tool still installs"
    );
}
