//! Cycle 10, T3: `add mcp`, `rm mcp`, `sync --replace-mcp` and `adopt` for MCP servers. Named after
//! AC10-NN / EC10-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
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

fn mcp_calls(env: &Env) -> Vec<String> {
    env.log_args()
        .into_iter()
        .chain(env.codex_log_args())
        .filter(|l| l.starts_with("mcp "))
        .collect()
}

fn state(env: &Env, harness: &str, name: &str) -> Option<String> {
    env.item_state(harness, "mcp", name)
}

const ALL: [&str; 4] = ["claude", "kilo-claude", "codex", "kilo-codex"];

/// Today's toolport, written into each profile the way the tools write it.
fn toolport_fixture(env: &Env) {
    let cmd = "/Applications/Toolport.app/gateway";
    for (profile, id) in [
        (env.native(), "claude-code"),
        (env.kilo(), "claude-code@f32399132c8e1a62"),
    ] {
        let doc = json!({"mcpServers": {"toolport": {"type": "stdio", "command": cmd, "args": [],
            "env": {"TOOLPORT_CLIENT_ID": id, "TOOLPORT_PROFILE": "trimmed"}}}});
        fs::write(claude_json(env, &profile), doc.to_string()).unwrap();
    }
    for (profile, id) in [
        (env.codex(), "codex"),
        (env.kilo_codex(), "codex@710cc12e3086d30b"),
    ] {
        fs::write(
            profile.join("config.toml"),
            format!(
                "[mcp_servers.toolport]\ncommand = \"{cmd}\"\n\n[mcp_servers.toolport.env]\nTOOLPORT_CLIENT_ID = \"{id}\"\nTOOLPORT_PROFILE = \"trimmed\"\n"
            ),
        )
        .unwrap();
    }
}

#[test]
fn ac10_20_add_mcp_adds_everywhere() {
    let env = machine().var("GH_TOKEN", "from-the-shell");
    let commits = env.commits().len();
    let out = env.ok(&[
        "add",
        "mcp",
        "github",
        "--env-from",
        "GITHUB_TOKEN=GH_TOKEN",
        "--",
        "npx",
        "-y",
        "@mcp/github",
    ]);
    assert!(out.contains("github"), "{out}");
    let config = env.config();
    assert!(
        config.contains("env = { GITHUB_TOKEN = { env = \"GH_TOKEN\" } }"),
        "{config}"
    );
    assert_eq!(env.commits().len(), commits + 1);
    assert_eq!(
        claude_servers(&env, &env.native())["github"]["env"]["GITHUB_TOKEN"],
        "${GH_TOKEN}"
    );
    assert_eq!(
        codex_servers(&env.kilo_codex())["github"]["env"]["GITHUB_TOKEN"].as_str(),
        Some("from-the-shell")
    );
    for h in ALL {
        assert_eq!(state(&env, h, "github").as_deref(), Some("ok"), "{h}");
    }
}

#[test]
fn ac10_21_only_and_url() {
    let env = machine();
    env.ok(&[
        "add",
        "mcp",
        "docs",
        "--only",
        "claude",
        "--url",
        "https://docs.example/mcp",
        "--bearer-from",
        "DOCS_TOKEN",
    ]);
    assert_eq!(
        claude_servers(&env, &env.kilo())["docs"]["headers"]["Authorization"],
        "Bearer ${DOCS_TOKEN}"
    );
    assert!(codex_servers(&env.codex()).is_empty());
    env.ok(&[
        "add",
        "mcp",
        "docs2",
        "--only",
        "codex",
        "--url",
        "https://docs.example/mcp",
        "--bearer-from",
        "DOCS_TOKEN",
    ]);
    assert_eq!(
        codex_servers(&env.codex())["docs2"]["bearer_token_env_var"].as_str(),
        Some("DOCS_TOKEN")
    );
    assert!(claude_servers(&env, &env.native())["docs2"].is_null());
}

#[test]
fn ac10_21_headers_and_plain_env() {
    let env = machine();
    env.ok(&[
        "add",
        "mcp",
        "web",
        "--only",
        "claude",
        "--url",
        "https://w.example/mcp",
        "--header",
        "X-Team=core",
    ]);
    assert_eq!(
        claude_servers(&env, &env.native())["web"]["headers"]["X-Team"],
        "core"
    );
    env.ok(&["add", "mcp", "plain", "--env", "MODE=fast", "--", "srv"]);
    assert_eq!(
        claude_servers(&env, &env.native())["plain"]["env"]["MODE"],
        "fast"
    );
}

#[test]
fn ac10_22_rm_mcp_removes_everywhere_it_was_added() {
    let env = machine();
    env.ok(&["add", "mcp", "github", "--", "gh-mcp"]);
    let commits = env.commits().len();
    env.ok(&["rm", "mcp", "github"]);
    assert!(!env.config().contains("[[mcp]]"));
    assert_eq!(env.commits().len(), commits + 1);
    let calls = mcp_calls(&env);
    assert!(calls.contains(&"mcp remove -s user github".to_string()));
    assert_eq!(
        calls.iter().filter(|c| *c == "mcp remove github").count(),
        2
    );
    for h in ALL {
        assert_eq!(state(&env, h, "github"), None, "{h}");
    }
}

#[test]
fn ac10_13_rm_refuses_a_server_colctl_did_not_add() {
    let env = machine();
    fs::write(
        env.codex().join("config.toml"),
        "[mcp_servers.orrery]\ncommand = \"orrery\"\n",
    )
    .unwrap();
    let (code, out, err) = env.code(&["rm", "mcp", "orrery"]);
    assert_ne!(code, 0);
    assert!(
        format!("{out}{err}").contains("orrery wasn't added by colctl"),
        "{out}{err}"
    );
    assert!(codex_servers(&env.codex()).contains_key("orrery"));
}

#[test]
fn ac10_14_replace_mcp_takes_over_a_taken_name() {
    let env = machine();
    let theirs = json!({"mcpServers": {"toolport": {"type": "stdio", "command": "theirs", "args": [], "env": {}}}});
    fs::write(claude_json(&env, &env.native()), theirs.to_string()).unwrap();
    env.code(&["add", "mcp", "toolport", "--", "ours"]);
    assert_eq!(state(&env, "claude", "toolport").as_deref(), Some("taken"));
    env.ok(&["sync", "--replace-mcp", "toolport"]);
    assert_eq!(
        claude_servers(&env, &env.native())["toolport"]["command"],
        "ours"
    );
    assert_eq!(state(&env, "claude", "toolport").as_deref(), Some("ok"));
}

#[test]
fn ac10_23_adopt_turns_four_copies_into_one_entry() {
    let env = machine();
    toolport_fixture(&env);
    let before: Vec<String> = [env.native(), env.kilo()]
        .iter()
        .map(|p| fs::read_to_string(claude_json(&env, p)).unwrap())
        .collect();
    env.ok(&["adopt"]);

    let config = env.config();
    let c = colctl::config::parse(&config, &env.home).unwrap();
    assert_eq!(c.mcp.len(), 1, "{config}");
    let s = &c.mcp[0];
    assert_eq!(s.name, "toolport");
    assert!(s.tools.is_empty(), "{config}");
    assert_eq!(
        s.env.get("TOOLPORT_PROFILE"),
        Some(&colctl::mcp::Value::Literal("trimmed".into()))
    );
    assert!(!s.env.contains_key("TOOLPORT_CLIENT_ID"), "{config}");
    assert_eq!(s.profiles.len(), 4, "{config}");
    assert_eq!(
        s.profiles["kilo-codex"].env["TOOLPORT_CLIENT_ID"],
        colctl::mcp::Value::Literal("codex@710cc12e3086d30b".into())
    );
    assert!(mcp_calls(&env).is_empty(), "{:?}", mcp_calls(&env));
    for h in ALL {
        assert_eq!(state(&env, h, "toolport").as_deref(), Some("ok"), "{h}");
    }
    let n = env.log_args().len() + env.codex_log_args().len();
    env.ok(&["sync"]);
    assert_eq!(env.log_args().len() + env.codex_log_args().len(), n);
    let after: Vec<String> = [env.native(), env.kilo()]
        .iter()
        .map(|p| fs::read_to_string(claude_json(&env, p)).unwrap())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn ac10_24_adopt_a_server_in_only_some_profiles() {
    let env = machine();
    for p in [env.codex(), env.kilo_codex()] {
        fs::write(
            p.join("config.toml"),
            "[mcp_servers.orrery]\ncommand = \"orrery\"\nargs = [\"serve\"]\n",
        )
        .unwrap();
    }
    env.ok(&["adopt"]);
    let c = colctl::config::parse(&env.config(), &env.home).unwrap();
    let s = &c.mcp[0];
    assert_eq!(s.tools, vec![colctl::tool::Tool::Codex]);
    assert!(s.profiles.is_empty());
    assert_eq!(
        s.transport,
        colctl::mcp::Transport::Stdio {
            command: "orrery".into(),
            args: vec!["serve".into()]
        }
    );
    assert!(mcp_calls(&env).is_empty());
    assert_eq!(state(&env, "claude", "orrery"), None);
}

#[test]
fn ac10_24_adopt_one_profile_only() {
    let env = machine();
    toolport_fixture(&env);
    // The other Codex profile keeps its own, which needs attention.
    env.expect(1, &["adopt", "codex"]);
    let c = colctl::config::parse(&env.config(), &env.home).unwrap();
    assert_eq!(c.mcp.len(), 1);
    assert!(c.mcp[0].profiles.is_empty());
    assert_eq!(state(&env, "codex", "toolport").as_deref(), Some("ok"));
    // The other profiles had their own toolport first.
    assert_eq!(
        state(&env, "kilo-codex", "toolport").as_deref(),
        Some("taken")
    );
}

#[test]
fn ac10_25_dry_run_and_json() {
    let env = machine();
    let config = env.config();
    let out = env.ok(&["add", "mcp", "x", "--dry-run", "--", "srv"]);
    assert!(out.contains("x"), "{out}");
    assert_eq!(env.config(), config);
    env.ok(&["add", "mcp", "x", "--", "srv"]);
    let n = mcp_calls(&env).len();
    env.ok(&["rm", "mcp", "x", "--dry-run"]);
    assert!(env.config().contains("name = \"x\""));
    env.write_config(&env.config().replace("srv", "srv2"));
    env.git(&["commit", "-qam", "edit"]);
    let plan = env.ok(&["sync", "--dry-run"]);
    assert!(plan.contains("would add MCP server x"), "{plan}");
    assert_eq!(mcp_calls(&env).len(), n);

    let status = env.status_json();
    let claude = status["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "claude")
        .unwrap();
    assert!(
        claude["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["kind"] == "mcp" && i["name"] == "x")
    );
}

#[test]
fn ac10_20_add_mcp_refuses_a_name_already_in_config() {
    let env = machine();
    env.ok(&["add", "mcp", "x", "--", "srv"]);
    let (code, out, err) = env.code(&["add", "mcp", "x", "--", "srv"]);
    assert_ne!(code, 0);
    assert!(format!("{out}{err}").contains("already"), "{out}{err}");
}

#[test]
fn ac10_20_add_mcp_needs_a_command_or_url() {
    let env = machine();
    for args in [
        vec!["add", "mcp", "x"],
        vec!["add", "mcp", "x", "--url", "https://u", "--", "srv"],
        vec!["add", "mcp", "x", "--header", "A=b", "--", "srv"],
        vec!["add", "mcp", "x", "--env", "NOEQUALS", "--", "srv"],
        vec!["add", "mcp", "Bad", "--", "srv"],
    ] {
        let (code, out, err) = env.code(&args);
        assert_eq!(code, 2, "{args:?}: {out}{err}");
    }
}

#[test]
fn ec10_24_adopt_keeps_the_first_when_commands_differ() {
    let env = machine();
    toolport_fixture(&env);
    fs::write(
        env.kilo_codex().join("config.toml"),
        "[mcp_servers.toolport]\ncommand = \"/elsewhere/gateway\"\n",
    )
    .unwrap();
    let out = env.expect(1, &["adopt"]);
    let c = colctl::config::parse(&env.config(), &env.home).unwrap();
    assert_eq!(
        c.mcp[0].transport,
        colctl::mcp::Transport::Stdio {
            command: "/Applications/Toolport.app/gateway".into(),
            args: vec![]
        }
    );
    assert!(!c.mcp[0].profiles.contains_key("kilo-codex"));
    assert_eq!(
        state(&env, "kilo-codex", "toolport").as_deref(),
        Some("taken")
    );
    assert!(out.contains("kilo-codex"), "{out}");
}

#[test]
fn ec10_25_identical_everywhere_has_no_overrides() {
    let env = machine();
    for p in [env.native(), env.kilo()] {
        fs::write(
            claude_json(&env, &p),
            json!({"mcpServers": {"same": {"type": "stdio", "command": "s", "args": ["a"], "env": {"K": "v"}}}})
                .to_string(),
        )
        .unwrap();
    }
    for p in [env.codex(), env.kilo_codex()] {
        fs::write(
            p.join("config.toml"),
            "[mcp_servers.same]\ncommand = \"s\"\nargs = [\"a\"]\n\n[mcp_servers.same.env]\nK = \"v\"\n",
        )
        .unwrap();
    }
    env.ok(&["adopt"]);
    let c = colctl::config::parse(&env.config(), &env.home).unwrap();
    assert!(c.mcp[0].profiles.is_empty());
    assert!(c.mcp[0].tools.is_empty());
    for h in ALL {
        assert_eq!(state(&env, h, "same").as_deref(), Some("ok"), "{h}");
    }
}

#[test]
fn ec10_28_only_claude_removes_codex_copies() {
    let env = machine();
    env.ok(&["add", "mcp", "x", "--", "srv"]);
    env.write_config(
        &env.config()
            .replace("name = \"x\"\n", "name = \"x\"\ntools = [\"claude\"]\n"),
    );
    env.git(&["commit", "-qam", "only claude"]);
    env.ok(&["sync"]);
    assert!(codex_servers(&env.codex()).is_empty());
    assert_eq!(state(&env, "codex", "x"), None);
    assert_eq!(state(&env, "claude", "x").as_deref(), Some("ok"));
}
