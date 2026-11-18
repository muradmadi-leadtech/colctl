//! Cycle 10, T8: the edge cases not covered with their features. Named after EC10-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.kilo_codex();
    env.agents_dir();
    env.ok(&["init"]);
    env
}

fn add_config(env: &Env, extra: &str) {
    env.write_config(&format!("{}{extra}", env.config()));
    env.git(&["add", "-A"]);
    env.git(&["commit", "-qm", "config"]);
}

fn claude_json(env: &Env, profile: &Path) -> PathBuf {
    if profile == env.home.join(".claude") {
        env.home.join(".claude.json")
    } else {
        profile.join(".claude.json")
    }
}

fn claude_servers(env: &Env, profile: &Path) -> Value {
    fs::read_to_string(claude_json(env, profile))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(|v| v["mcpServers"].clone())
        .unwrap_or(Value::Null)
}

fn kinds(env: &Env) -> Vec<String> {
    let v = env.status_json();
    let mut out: Vec<String> = v["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|h| h["items"].as_array().cloned().unwrap_or_default())
        .map(|i| i["kind"].as_str().unwrap().to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn ec10_01_nothing_new_declared_is_as_before() {
    let env = machine();
    env.ok(&["sync"]);
    let k = kinds(&env);
    assert!(!k.contains(&"mcp".to_string()), "{k:?}");
    assert!(!k.contains(&"command".to_string()), "{k:?}");
    assert!(
        !env.log_args()
            .iter()
            .any(|c| c.starts_with("mcp ") || c.contains("colctl-setup"))
    );
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
}

#[test]
fn ec10_07_a_literal_with_a_placeholder_is_passed_as_is() {
    let env = machine();
    add_config(
        &env,
        "\n[[mcp]]\nname = \"p\"\ncommand = \"p\"\nenv = { HOME_DIR = \"${HOME}/x\" }\ntools = [\"claude\"]\n",
    );
    env.ok(&["sync"]);
    assert_eq!(
        claude_servers(&env, &env.native())["p"]["env"]["HOME_DIR"],
        "${HOME}/x"
    );
}

#[test]
fn ec10_08_an_mcp_server_and_a_plugin_can_share_a_name() {
    let env = machine();
    env.ok(&["add", "mcp", "superpowers", "--only", "claude", "--", "sp"]);
    env.write_config(&env.config().replace(
        "enabled = []",
        "enabled = [\"superpowers@anthropic-plugin-directory\"]",
    ));
    env.git(&["commit", "-qam", "plugin"]);
    env.ok(&["sync"]);
    assert_eq!(
        env.item_state("claude", "mcp", "superpowers").as_deref(),
        Some("ok")
    );
    assert!(
        env.installed(&env.native())
            .contains(&"superpowers@anthropic-plugin-directory".to_string())
    );
}

#[test]
fn ec10_14_replace_mcp_re_adds_after_a_secret_changes() {
    let env = machine().var("GH_TOKEN", "old");
    env.ok(&[
        "add",
        "mcp",
        "gh",
        "--only",
        "codex",
        "--env-from",
        "T=GH_TOKEN",
        "--",
        "gh",
    ]);
    let env = env.var("GH_TOKEN", "new");
    // Status can't see the new value: it never reads secrets.
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
    env.ok(&["sync", "--replace-mcp", "gh"]);
    let text = fs::read_to_string(env.codex().join("config.toml")).unwrap();
    assert!(text.contains("T = \"new\""), "{text}");
}

#[test]
fn ec10_15_a_half_written_file_is_unreadable_for_that_run() {
    let env = machine();
    env.ok(&["add", "mcp", "x", "--only", "claude", "--", "x"]);
    let path = claude_json(&env, &env.kilo());
    let full = fs::read_to_string(&path).unwrap();
    fs::write(&path, &full[..full.len() / 2]).unwrap();
    assert_eq!(
        env.item_state("kilo-claude", "mcp", "x").as_deref(),
        Some("unreadable")
    );
    fs::write(&path, &full).unwrap();
    assert_eq!(
        env.item_state("kilo-claude", "mcp", "x").as_deref(),
        Some("ok")
    );
}

fn hooks(env: &Env, script_mode: u32) {
    let dir = env.source.join("hooks");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("hooks.json"),
        json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [
            {"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/guard.sh"}]}]}})
        .to_string(),
    )
    .unwrap();
    fs::write(dir.join("guard.sh"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(
        dir.join("guard.sh"),
        fs::Permissions::from_mode(script_mode),
    )
    .unwrap();
}

#[test]
fn ec10_21_a_script_keeps_its_mode() {
    let env = machine();
    hooks(&env, 0o644);
    env.ok(&["sync"]);
    let built = env
        .source
        .join("plugins/.manifests/colctl-setup/plugin/guard.sh");
    let mode = fs::metadata(&built).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0, "copied as it is, not made executable");
}

#[test]
fn ec10_22_only_commands_means_no_plugin() {
    let env = machine();
    let dir = env.source.join("commands");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("standup.md"), "Write the standup\n").unwrap();
    env.ok(&["sync"]);
    assert!(!env.log_args().iter().any(|c| c.contains("colctl-setup")));
    assert!(env.native().join("commands/standup.md").exists());
}

#[test]
fn ec10_23_a_foreign_colctl_setup_marketplace_is_not_replaced() {
    let env = machine();
    let theirs = env.home.join("theirs");
    fs::create_dir_all(theirs.join(".claude-plugin")).unwrap();
    fs::write(
        theirs.join(".claude-plugin/marketplace.json"),
        json!({"name": "colctl-setup", "plugins": []}).to_string(),
    )
    .unwrap();
    env.ok(&["sync"]);
    env.run(&["status"]);
    // Someone else's marketplace with that name, added straight through claude.
    let out = std::process::Command::new(env.bin.join("claude"))
        .args(["plugin", "marketplace", "add", theirs.to_str().unwrap()])
        .env("HOME", &env.home)
        .env("FAKE_CLAUDE_LOG", &env.log)
        .output()
        .unwrap();
    assert!(out.status.success());
    hooks(&env, 0o755);
    env.code(&["sync"]);
    assert_eq!(
        env.item_state("claude", "source", "colctl-setup")
            .as_deref(),
        Some("invalid")
    );
    let text = env.expect(1, &["status"]);
    assert!(text.contains("colctl-setup already comes from"), "{text}");
}

#[test]
fn ec10_26_adopting_a_relative_hook_path_warns() {
    let env = machine();
    fs::write(
        env.native().join("settings.json"),
        json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "scripts/notify.sh"}]}]}})
            .to_string(),
    )
    .unwrap();
    let out = env.ok(&["adopt", "claude", "hooks"]);
    assert!(
        out.contains("scripts/notify.sh is a relative path"),
        "{out}"
    );
}

#[test]
fn ac10_23_adopt_mcp_name_takes_one_server_from_every_profile() {
    let env = machine();
    for p in [env.codex(), env.kilo_codex()] {
        fs::write(
            p.join("config.toml"),
            "[mcp_servers.toolport]\ncommand = \"gw\"\n\n[mcp_servers.other]\ncommand = \"o\"\n",
        )
        .unwrap();
    }
    env.ok(&["adopt", "mcp", "toolport"]);
    let c = colctl::config::parse(&env.config(), &env.home).unwrap();
    assert_eq!(c.mcp.len(), 1);
    assert_eq!(c.mcp[0].name, "toolport");
    assert_eq!(
        env.item_state("kilo-codex", "mcp", "toolport").as_deref(),
        Some("ok")
    );
    assert_eq!(
        env.item_state("codex", "mcp", "other").as_deref(),
        Some("foreign")
    );
}
