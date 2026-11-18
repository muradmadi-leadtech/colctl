//! Cycle 10, T2: MCP servers in status and sync. Named after AC10-NN / EC10-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::Env;

/// Four Claude/Codex profiles and `~/.agents`, initialised.
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

const TOOLPORT: &str = r#"
[[mcp]]
name = "toolport"
command = "/opt/toolport/gateway"
args = ["--stdio"]
env = { TOOLPORT_PROFILE = "trimmed" }

[mcp.profile.claude]
env = { TOOLPORT_CLIENT_ID = "claude-code" }
"#;

fn add_config(env: &Env, extra: &str) {
    env.write_config(&format!("{}{extra}", env.config()));
    env.git(&["add", "-A"]);
    env.git(&["commit", "-qm", "config"]);
}

/// The file a Claude profile keeps its user MCP servers in.
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

fn mcp_calls(log: &[String]) -> Vec<String> {
    log.iter()
        .filter(|l| l.starts_with("mcp "))
        .cloned()
        .collect()
}

fn state(env: &Env, harness: &str, name: &str) -> Option<String> {
    env.item_state(harness, "mcp", name)
}

#[test]
fn ac10_10_a_missing_server_is_added_through_the_tools_cli() {
    let env = machine();
    add_config(&env, TOOLPORT);
    let before_claude = fs::read_to_string(claude_json(&env, &env.home.join(".claude"))).ok();
    assert_eq!(
        state(&env, "claude", "toolport").as_deref(),
        Some("missing")
    );
    env.ok(&["sync"]);

    let claude = mcp_calls(&env.log_args());
    assert_eq!(
        claude,
        vec![
            "mcp add -s user toolport -e TOOLPORT_CLIENT_ID=claude-code -e TOOLPORT_PROFILE=trimmed -- /opt/toolport/gateway --stdio",
            "mcp add -s user toolport -e TOOLPORT_PROFILE=trimmed -- /opt/toolport/gateway --stdio",
        ]
    );
    // Each Claude call ran with its own profile: the native one without CLAUDE_CONFIG_DIR.
    let dirs: Vec<Value> = env
        .log()
        .iter()
        .filter(|l| l["args"][0] == "mcp")
        .map(|l| l["config_dir"].clone())
        .collect();
    assert_eq!(dirs[0], Value::Null);
    assert_eq!(dirs[1], json!(env.kilo().display().to_string()));

    let codex = mcp_calls(&env.codex_log_args());
    assert_eq!(
        codex,
        vec!["mcp add toolport --env TOOLPORT_PROFILE=trimmed -- /opt/toolport/gateway --stdio"; 2]
    );
    assert_eq!(
        claude_servers(&env, &env.native())["toolport"]["env"]["TOOLPORT_CLIENT_ID"],
        "claude-code"
    );
    assert_eq!(
        codex_servers(&env.kilo_codex())["toolport"]["command"].as_str(),
        Some("/opt/toolport/gateway")
    );
    assert!(before_claude.is_none());
    for h in ["claude", "kilo-claude", "codex", "kilo-codex"] {
        assert_eq!(state(&env, h, "toolport").as_deref(), Some("ok"), "{h}");
    }
    assert_eq!(state(&env, "agents", "toolport"), None);
    // A second sync calls nothing.
    let n = env.log_args().len() + env.codex_log_args().len();
    env.ok(&["sync"]);
    assert_eq!(env.log_args().len() + env.codex_log_args().len(), n);
}

#[test]
fn ac10_10_http_servers_and_references() {
    let env = machine();
    add_config(
        &env,
        r#"
[[mcp]]
name = "docs"
url = "https://docs.example/mcp"
bearer = { env = "DOCS_TOKEN" }

[[mcp]]
name = "gh"
command = "gh-mcp"
env = { GITHUB_TOKEN = { env = "GH_TOKEN" } }
tools = ["claude"]
"#,
    );
    env.ok(&["sync"]);
    let claude = mcp_calls(&env.log_args());
    assert!(
        claude.contains(
            &"mcp add -s user -t http docs https://docs.example/mcp -H Authorization: Bearer ${DOCS_TOKEN}"
                .to_string()
        ),
        "{claude:?}"
    );
    assert!(
        claude.contains(&"mcp add -s user gh -e GITHUB_TOKEN=${GH_TOKEN} -- gh-mcp".to_string()),
        "{claude:?}"
    );
    let codex = mcp_calls(&env.codex_log_args());
    assert_eq!(
        codex,
        vec!["mcp add docs --url https://docs.example/mcp --bearer-token-env-var DOCS_TOKEN"; 2]
    );
    assert_eq!(
        claude_servers(&env, &env.native())["gh"]["env"]["GITHUB_TOKEN"],
        "${GH_TOKEN}"
    );
}

#[test]
fn ac10_11_every_state_is_reported() {
    let env = machine();
    add_config(
        &env,
        r#"
[[mcp]]
name = "fine"
command = "fine"

[[mcp]]
name = "edited"
command = "edited"

[[mcp]]
name = "gone"
command = "gone"

[[mcp]]
name = "web"
url = "https://w.example/mcp"
headers = { X-Team = "core" }
"#,
    );
    env.ok(&["sync"]);
    // Someone edits one in a profile, a server appears that colctl doesn't know, one is
    // removed from config, and one with the same name as a config entry was there first.
    let path = claude_json(&env, &env.native());
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    v["mcpServers"]["edited"]["args"] = json!(["--debug"]);
    v["mcpServers"]["orrery"] =
        json!({"type": "stdio", "command": "orrery", "args": [], "env": {}});
    v["mcpServers"]["mine"] = json!({"type": "stdio", "command": "mine", "args": [], "env": {}});
    fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let config = env.config().replace(
        "[[mcp]]\nname = \"gone\"\ncommand = \"gone\"\n",
        "[[mcp]]\nname = \"mine\"\ncommand = \"ours\"\n",
    );
    env.write_config(&config);
    env.git(&["commit", "-qam", "edit"]);

    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 1);
    let s = |n: &str| state(&env, "claude", n);
    assert_eq!(s("fine").as_deref(), Some("ok"));
    assert_eq!(s("edited").as_deref(), Some("differs"));
    assert_eq!(s("orrery").as_deref(), Some("foreign"));
    assert_eq!(s("mine").as_deref(), Some("taken"));
    assert_eq!(s("gone").as_deref(), Some("orphaned"));
    assert_eq!(s("web").as_deref(), Some("ok"));
    assert_eq!(state(&env, "codex", "web").as_deref(), Some("unsupported"));
    assert_eq!(state(&env, "codex", "mine").as_deref(), Some("missing"));

    let text = env.expect(1, &["status"]);
    assert!(text.contains("colctl adopt"), "{text}");
    assert!(text.contains("colctl sync --replace-mcp mine"), "{text}");
    assert!(text.contains("can't set headers"), "{text}");
}

#[test]
fn ac10_11_a_foreign_server_alone_keeps_status_in_sync() {
    let env = machine();
    let path = claude_json(&env, &env.native());
    fs::write(
        &path,
        json!({"mcpServers": {"orrery": {"type": "stdio", "command": "o", "args": [], "env": {}}}})
            .to_string(),
    )
    .unwrap();
    env.ok(&["sync"]);
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
    assert_eq!(state(&env, "claude", "orrery").as_deref(), Some("foreign"));
}

#[test]
fn ac10_12_a_change_is_remove_then_add() {
    let env = machine();
    add_config(&env, TOOLPORT);
    env.ok(&["sync"]);
    env.write_config(&env.config().replace("trimmed", "full"));
    env.git(&["commit", "-qam", "full"]);
    assert_eq!(
        state(&env, "kilo-codex", "toolport").as_deref(),
        Some("differs")
    );
    let n = env.codex_log_args().len();
    env.ok(&["sync"]);
    let codex: Vec<String> = env.codex_log_args()[n..].to_vec();
    assert_eq!(
        codex,
        vec![
            "mcp remove toolport",
            "mcp add toolport --env TOOLPORT_PROFILE=full -- /opt/toolport/gateway --stdio",
            "mcp remove toolport",
            "mcp add toolport --env TOOLPORT_PROFILE=full -- /opt/toolport/gateway --stdio",
        ]
    );
    assert!(
        env.log_args()
            .contains(&"mcp remove -s user toolport".to_string())
    );
    for h in ["claude", "kilo-claude", "codex", "kilo-codex"] {
        assert_eq!(state(&env, h, "toolport").as_deref(), Some("ok"), "{h}");
    }
}

#[test]
fn ac10_13_foreign_servers_are_left_alone() {
    let env = machine();
    let codex = env.codex();
    fs::write(
        codex.join("config.toml"),
        "[mcp_servers.orrery]\ncommand = \"orrery\"\n",
    )
    .unwrap();
    add_config(&env, TOOLPORT);
    env.ok(&["sync"]);
    assert!(!env.codex_log_args().iter().any(|a| a.contains("orrery")));
    assert!(codex_servers(&codex).contains_key("orrery"));
}

#[test]
fn ac10_14_a_taken_name_is_not_overwritten() {
    let env = machine();
    let path = claude_json(&env, &env.native());
    let theirs = json!({"mcpServers": {"toolport": {"type": "stdio", "command": "theirs", "args": [], "env": {}}}});
    fs::write(&path, theirs.to_string()).unwrap();
    add_config(&env, TOOLPORT);
    env.code(&["sync"]);
    assert_eq!(
        claude_servers(&env, &env.native())["toolport"]["command"],
        "theirs"
    );
    assert_eq!(state(&env, "claude", "toolport").as_deref(), Some("taken"));
    assert!(!env.log_args().iter().any(|a| a.contains("mcp remove")));
}

#[test]
fn ac10_15_orphans_are_removed() {
    let env = machine();
    add_config(&env, TOOLPORT);
    env.ok(&["sync"]);
    let config = env.config();
    let cut = config.find("\n[[mcp]]").unwrap();
    env.write_config(&config[..cut]);
    env.git(&["commit", "-qam", "drop"]);
    env.ok(&["sync"]);
    assert!(
        env.log_args()
            .contains(&"mcp remove -s user toolport".to_string())
    );
    assert_eq!(
        env.codex_log_args()
            .iter()
            .filter(|a| *a == "mcp remove toolport")
            .count(),
        2
    );
    assert!(claude_servers(&env, &env.native())["toolport"].is_null());
    assert!(codex_servers(&env.codex()).is_empty());
    assert_eq!(state(&env, "claude", "toolport"), None);
}

#[test]
fn ec10_02_no_file_means_no_servers() {
    let env = machine();
    assert!(!claude_json(&env, &env.native()).exists());
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
}

#[test]
fn ec10_03_an_unreadable_profile_file_stops_only_its_mcp_items() {
    let env = machine();
    fs::write(claude_json(&env, &env.kilo()), "{ not json").unwrap();
    add_config(&env, TOOLPORT);
    env.code(&["sync"]);
    assert_eq!(
        state(&env, "kilo-claude", "toolport").as_deref(),
        Some("unreadable")
    );
    assert_eq!(state(&env, "claude", "toolport").as_deref(), Some("ok"));
    assert!(
        !env.log()
            .iter()
            .any(|l| l["args"][0] == "mcp" && l["config_dir"] != Value::Null)
    );
    let text = env.expect(1, &["status"]);
    assert!(text.contains(".claude.json"), "{text}");
}

#[test]
fn ec10_06_dash_args_come_after_the_separator() {
    let env = machine();
    add_config(
        &env,
        "\n[[mcp]]\nname = \"d\"\ncommand = \"srv\"\nargs = [\"-e\", \"--stdio\"]\ntools = [\"codex\"]\n",
    );
    env.ok(&["sync"]);
    assert!(
        env.codex_log_args()
            .contains(&"mcp add d -- srv -e --stdio".to_string())
    );
}

#[test]
fn ec10_10_a_failing_add_fails_only_that_item() {
    let env = machine().var("FAKE_CLAUDE_FAIL", "mcp add -s user broken");
    add_config(
        &env,
        "\n[[mcp]]\nname = \"broken\"\ncommand = \"b\"\n\n[[mcp]]\nname = \"fine\"\ncommand = \"f\"\n",
    );
    let (code, out, err) = env.code(&["sync"]);
    assert_ne!(code, 0, "{out}{err}");
    assert_eq!(state(&env, "claude", "fine").as_deref(), Some("ok"));
    assert_eq!(state(&env, "claude", "broken").as_deref(), Some("missing"));
    assert_eq!(state(&env, "codex", "broken").as_deref(), Some("ok"));
}

#[test]
fn ec10_11_a_server_removed_by_hand_is_forgotten() {
    let env = machine();
    add_config(&env, TOOLPORT);
    env.ok(&["sync"]);
    // Removed from the profile by hand, then from config.
    fs::write(claude_json(&env, &env.native()), "{}").unwrap();
    let config = env.config();
    let cut = config.find("\n[[mcp]]").unwrap();
    env.write_config(&config[..cut]);
    env.git(&["commit", "-qam", "drop"]);
    env.ok(&["sync"]);
    assert_eq!(state(&env, "claude", "toolport"), None);
    let (code, _, _) = env.code(&["status"]);
    assert_eq!(code, 0);
}

#[test]
fn ec10_11_remove_reporting_not_found_counts_as_removed() {
    let env = machine();
    add_config(&env, TOOLPORT);
    env.ok(&["sync"]);
    // The file still lists it, but the tool says it's gone.
    let env = env.var("FAKE_CLAUDE_MCP_GONE", "1");
    let config = env.config();
    let cut = config.find("\n[[mcp]]").unwrap();
    env.write_config(&config[..cut]);
    env.git(&["commit", "-qam", "drop"]);
    env.ok(&["sync"]);
    assert_eq!(
        state(&env, "kilo-claude", "toolport").as_deref(),
        Some("foreign")
    );
}

#[test]
fn ec10_30_codex_headers_are_unsupported_claude_still_gets_them() {
    let env = machine();
    add_config(
        &env,
        "\n[[mcp]]\nname = \"web\"\nurl = \"https://w.example/mcp\"\nheaders = { X-Team = \"core\" }\n",
    );
    env.ok(&["sync"]);
    assert!(mcp_calls(&env.codex_log_args()).is_empty());
    assert_eq!(
        claude_servers(&env, &env.native())["web"]["headers"]["X-Team"],
        "core"
    );
    let v = env.status_json();
    let codex = v["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "codex")
        .unwrap();
    let web = codex["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "mcp" && i["name"] == "web")
        .unwrap();
    assert!(
        web["hint"].as_str().unwrap().contains("can't set headers"),
        "{web}"
    );
}
