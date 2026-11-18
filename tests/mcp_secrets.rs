//! Cycle 10, T4: secrets in MCP entries. Named after AC10-NN / EC10-NN.

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

/// A stand-in for macOS's `security`: item "gh" is "s3cret-value-123", anything else isn't there.
/// Each call is logged.
fn fake_security(env: &Env) -> PathBuf {
    let log = env.home.parent().unwrap().join("security.log");
    let script = format!(
        "#!/bin/sh\necho \"$*\" >> '{}'\nif [ \"$3\" = gh ]; then echo s3cret-value-123; exit 0; fi\necho 'security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain.' >&2\nexit 44\n",
        log.display()
    );
    let tmp = env.bin.join(".security.tmp");
    fs::write(&tmp, script).unwrap();
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).unwrap();
    fs::rename(&tmp, env.bin.join("security")).unwrap();
    log
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

fn codex_servers(profile: &Path) -> toml::Table {
    fs::read_to_string(profile.join("config.toml"))
        .ok()
        .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
        .and_then(|t| t.get("mcp_servers").and_then(|v| v.as_table()).cloned())
        .unwrap_or_default()
}

fn state(env: &Env, harness: &str, name: &str) -> Option<String> {
    env.item_state(harness, "mcp", name)
}

/// Every file under `dir`, read as text where it can be.
fn all_text(dir: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(t) = fs::read_to_string(&p) {
                out.push_str(&t);
            }
        }
    }
    out
}

const GH_ENV: &str = "\n[[mcp]]\nname = \"github\"\ncommand = \"gh-mcp\"\nenv = { GITHUB_TOKEN = { env = \"GH_TOKEN\" } }\n";
const GH_KEYCHAIN: &str = "\n[[mcp]]\nname = \"github\"\ncommand = \"gh-mcp\"\nenv = { GITHUB_TOKEN = { keychain = \"gh\" } }\n";

#[test]
fn ac10_30_an_env_reference_in_claude_stays_a_reference() {
    let env = machine();
    add_config(&env, GH_ENV);
    let (code, out, err) = env.code(&["sync"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        claude_servers(&env, &env.native())["github"]["env"]["GITHUB_TOKEN"],
        "${GH_TOKEN}"
    );
    assert!(codex_servers(&env.codex()).is_empty());
    assert!(
        format!("{out}{err}").contains("set GH_TOKEN, then colctl sync"),
        "{out}{err}"
    );
    assert_eq!(state(&env, "claude", "github").as_deref(), Some("ok"));
    assert_eq!(state(&env, "codex", "github").as_deref(), Some("missing"));
}

#[test]
fn ac10_31_keychain_items_are_resolved_at_sync() {
    let env = machine();
    fake_security(&env);
    add_config(&env, GH_KEYCHAIN);
    let (code, out, err) = env.code(&["sync", "-v"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        claude_servers(&env, &env.native())["github"]["env"]["GITHUB_TOKEN"],
        "s3cret-value-123"
    );
    assert_eq!(
        codex_servers(&env.kilo_codex())["github"]["env"]["GITHUB_TOKEN"].as_str(),
        Some("s3cret-value-123")
    );
    assert!(
        !out.contains("s3cret") && !err.contains("s3cret"),
        "{out}{err}"
    );
    let source = all_text(&env.source);
    assert!(!source.contains("s3cret"));
    let status = env.ok(&["status", "--json"]);
    assert!(!status.contains("s3cret"));
}

#[test]
fn ec10_13_a_keychain_item_is_asked_for_once_per_run() {
    let env = machine();
    let log = fake_security(&env);
    add_config(&env, GH_KEYCHAIN);
    env.ok(&["sync"]);
    let calls = fs::read_to_string(&log).unwrap();
    assert_eq!(calls.lines().count(), 1, "{calls}");
    assert_eq!(calls.trim(), "find-generic-password -s gh -w");
}

#[test]
fn ec10_12_a_missing_item_or_no_security_skips_that_server() {
    let env = machine();
    add_config(
        &env,
        "\n[[mcp]]\nname = \"x\"\ncommand = \"x\"\nenv = { API_KEY = { keychain = \"nope\" } }\n\n[[mcp]]\nname = \"fine\"\ncommand = \"f\"\n",
    );
    // No `security` at all.
    let (code, out, err) = env.code(&["sync"]);
    assert_eq!(code, 1);
    assert!(
        format!("{out}{err}").contains("keychain item nope"),
        "{out}{err}"
    );
    assert_eq!(state(&env, "claude", "fine").as_deref(), Some("ok"));
    assert_eq!(state(&env, "claude", "x").as_deref(), Some("missing"));
    // With `security`, but no such item.
    fake_security(&env);
    let (code, out, err) = env.code(&["sync", "-v"]);
    assert_eq!(code, 1);
    assert!(
        format!("{out}{err}").contains("The specified item could not"),
        "{out}{err}"
    );
}

#[test]
fn ac10_32_add_refuses_a_literal_secret() {
    let env = machine();
    let config = env.config();
    let (code, out, err) = env.code(&[
        "add",
        "mcp",
        "x",
        "--env",
        "API_KEY=ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        "--",
        "cmd",
    ]);
    assert_eq!(code, 2);
    assert!(
        format!("{out}{err}").contains(
            "That looks like a secret. Use --env-from API_KEY=NAME or --keychain API_KEY=ITEM."
        ),
        "{out}{err}"
    );
    assert_eq!(env.config(), config);
}

#[test]
fn ac10_32_a_commit_with_a_literal_secret_is_refused() {
    let env = machine();
    let secret = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    env.write_config(&format!(
        "{}\n[[mcp]]\nname = \"x\"\ncommand = \"cmd\"\nenv = {{ API_KEY = \"{secret}\" }}\n",
        env.config()
    ));
    let commits = env.commits().len();
    let (code, out, err) = env.code(&["add", "mcp", "y", "--", "srv"]);
    assert_ne!(code, 0);
    let text = format!("{out}{err}");
    assert!(text.contains("config.toml"), "{text}");
    assert!(text.contains("mcp.x.env.API_KEY"), "{text}");
    assert!(!text.contains(secret), "{text}");
    assert_eq!(env.commits().len(), commits);
}

#[test]
fn ec10_17_the_not_secret_marker_silences_the_check() {
    let env = machine();
    env.write_config(&format!(
        "{}\n[[mcp]]\nname = \"x\"\ncommand = \"cmd\"\nenv = {{ BUILD_ID = \"a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6\" }} # colctl: not-secret\n",
        env.config()
    ));
    env.ok(&["add", "mcp", "y", "--", "srv"]);
    assert!(env.git(&["log", "-1", "--format=%s"]).contains("add mcp y"));
}

#[test]
fn ec10_27_a_refused_commit_restores_config() {
    let env = machine();
    let secret = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let with_secret = format!(
        "{}\n[[mcp]]\nname = \"x\"\ncommand = \"cmd\"\nenv = {{ API_KEY = \"{secret}\" }}\n",
        env.config()
    );
    env.write_config(&with_secret);
    let (code, _, _) = env.code(&["add", "mcp", "y", "--", "srv"]);
    assert_ne!(code, 0);
    assert_eq!(env.config(), with_secret);
    let status = env.status_json();
    assert!(!status.to_string().contains("\"y\""));
}

#[test]
fn ac10_33_adopt_keeps_secrets_out() {
    let env = machine();
    fs::write(
        claude_json(&env, &env.native()),
        json!({"mcpServers": {"github": {"type": "stdio", "command": "gh-mcp", "args": [],
            "env": {"GITHUB_TOKEN": "ghp_abcdefghijklmnopqrstuvwxyz0123456789"}}}})
        .to_string(),
    )
    .unwrap();
    let (_, out, err) = env.code(&["adopt"]);
    let config = env.config();
    assert!(
        config.contains("GITHUB_TOKEN = { env = \"GITHUB_TOKEN\" }"),
        "{config}"
    );
    assert!(!config.contains("ghp_"));
    let text = format!("{out}{err}");
    assert!(text.contains("Set GITHUB_TOKEN in your shell"), "{text}");
    assert!(
        text.contains("security add-generic-password -s colctl-github -w"),
        "{text}"
    );
    assert!(!text.contains("ghp_"), "{text}");
    // The profile keeps its value and isn't re-added.
    assert_eq!(state(&env, "claude", "github").as_deref(), Some("ok"));
}

#[test]
fn ac10_34_resolved_values_are_redacted() {
    let env = machine().var("FAKE_CODEX_FAIL", "mcp add github");
    fake_security(&env);
    add_config(&env, GH_KEYCHAIN);
    let (code, out, err) = env.code(&["sync", "-v"]);
    assert_ne!(code, 0);
    let text = format!("{out}{err}");
    assert!(!text.contains("s3cret"), "{text}");
    assert!(text.contains("•••"), "{text}");
}
