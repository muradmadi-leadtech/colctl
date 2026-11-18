//! Cycle 10, T1: `[[mcp]]` entries in config.toml and what colctl records about them. Named after
//! AC10-NN / EC10-NN (docs/whole-setup/acceptance-criteria.md, edge-cases.md).

use colctl::config::{self, Config};
use colctl::mcp::{Server, Transport, Value};
use colctl::state::{HarnessState, McpRecord};
use std::collections::BTreeMap;
use std::path::Path;

const HARNESSES: &str = r#"version = 1

[[harness]]
name = "claude"
kind = "claude"
path = "~/.claude"

[[harness]]
name = "kilo-claude"
kind = "claude"
path = "~/kilo/claude"

[[harness]]
name = "codex"
kind = "codex"
path = "~/.codex"

[[harness]]
name = "kilo-codex"
kind = "codex"
path = "~/kilo/codex"

[[harness]]
name = "agents"
kind = "agents"
path = "~/.agents"
"#;

const TOOLPORT: &str = r#"
# the gateway
[[mcp]]
name = "toolport"
command = "/Applications/Toolport.app/gateway"
args = []
env = { TOOLPORT_PROFILE = "trimmed" }

[mcp.profile.claude]
env = { TOOLPORT_CLIENT_ID = "claude-code" }

[mcp.profile.kilo-claude]
env = { TOOLPORT_CLIENT_ID = "claude-code@f32399132c8e1a62" }

[mcp.profile.codex]
env = { TOOLPORT_CLIENT_ID = "codex" }

[mcp.profile.kilo-codex]
env = { TOOLPORT_CLIENT_ID = "codex@710cc12e3086d30b" }
"#;

fn parse(text: &str) -> Result<Config, String> {
    config::parse(text, Path::new("/home/u"))
}

fn with(extra: &str) -> String {
    format!("{HARNESSES}{extra}")
}

fn lit(s: &str) -> Value {
    Value::Literal(s.into())
}

fn err(extra: &str) -> String {
    parse(&with(extra)).expect_err("should not parse")
}

#[test]
fn ac10_01_an_entry_with_overrides_parses() {
    let c = parse(&with(TOOLPORT)).unwrap();
    assert_eq!(c.mcp.len(), 1);
    let s = &c.mcp[0];
    let ids = [
        ("claude", "claude-code"),
        ("kilo-claude", "claude-code@f32399132c8e1a62"),
        ("codex", "codex"),
        ("kilo-codex", "codex@710cc12e3086d30b"),
    ];
    for (h, id) in ids {
        let e = s.for_harness(c.harness(h).unwrap()).expect(h);
        assert_eq!(e.name, "toolport");
        assert_eq!(
            e.transport,
            Transport::Stdio {
                command: "/Applications/Toolport.app/gateway".into(),
                args: vec![],
            }
        );
        assert_eq!(e.env.get("TOOLPORT_PROFILE"), Some(&lit("trimmed")), "{h}");
        assert_eq!(e.env.get("TOOLPORT_CLIENT_ID"), Some(&lit(id)), "{h}");
    }
    assert!(s.for_harness(c.harness("agents").unwrap()).is_none());
}

#[test]
fn ac10_01_tools_limit_where_an_entry_goes() {
    let c = parse(&with(
        "[[mcp]]\nname = \"orrery\"\ncommand = \"orrery\"\ntools = [\"codex\"]\n",
    ))
    .unwrap();
    let s = &c.mcp[0];
    assert!(s.for_harness(c.harness("claude").unwrap()).is_none());
    assert!(s.for_harness(c.harness("codex").unwrap()).is_some());
    assert!(s.for_harness(c.harness("kilo-codex").unwrap()).is_some());
}

#[test]
fn ac10_01_args_override_replaces_and_headers_merge() {
    let c = parse(&with(
        r#"[[mcp]]
name = "web"
url = "https://mcp.example/mcp"
headers = { X-Team = "core", X-Env = "prod" }
bearer = { env = "WEB_TOKEN" }

[mcp.profile.claude]
headers = { X-Env = "dev" }

[[mcp]]
name = "local"
command = "srv"
args = ["--stdio", "a"]

[mcp.profile.kilo-claude]
args = ["b"]
"#,
    ))
    .unwrap();
    let web = c.mcp[0].for_harness(c.harness("claude").unwrap()).unwrap();
    match web.transport {
        Transport::Http {
            url,
            headers,
            bearer,
        } => {
            assert_eq!(url, "https://mcp.example/mcp");
            assert_eq!(headers.get("X-Team"), Some(&lit("core")));
            assert_eq!(headers.get("X-Env"), Some(&lit("dev")));
            assert_eq!(bearer, Some(Value::Env("WEB_TOKEN".into())));
        }
        other => panic!("{other:?}"),
    }
    let local = |h: &str| {
        c.mcp[1]
            .for_harness(c.harness(h).unwrap())
            .unwrap()
            .transport
    };
    assert_eq!(
        local("claude"),
        Transport::Stdio {
            command: "srv".into(),
            args: vec!["--stdio".into(), "a".into()]
        }
    );
    assert_eq!(
        local("kilo-claude"),
        Transport::Stdio {
            command: "srv".into(),
            args: vec!["b".into()]
        }
    );
}

#[test]
fn ac10_02_references_are_kept_as_references() {
    let text = with(
        r#"[[mcp]]
name = "gh"
command = "npx"
args = ["-y", "@mcp/github"]
env = { A = { env = "X" }, B = { keychain = "k" }, C = "plain" }
"#,
    );
    let c = parse(&text).unwrap();
    let env = &c.mcp[0].env;
    assert_eq!(env["A"], Value::Env("X".into()));
    assert_eq!(env["B"], Value::Keychain("k".into()));
    assert_eq!(env["C"], lit("plain"));

    // Adding and removing another entry leaves this one, and every comment, as it was.
    let other = Server {
        name: "other".into(),
        transport: Transport::Stdio {
            command: "o".into(),
            args: vec!["--x".into()],
        },
        env: BTreeMap::new(),
        tools: vec![],
        profiles: BTreeMap::new(),
    };
    let added = config::add_mcp(&format!("{text}{TOOLPORT}"), &other).unwrap();
    assert!(added.contains("# the gateway"));
    let c2 = parse(&added).unwrap();
    assert_eq!(c2.mcp.len(), 3);
    assert_eq!(c2.mcp[0], c.mcp[0]);
    assert_eq!(c2.mcp[2], other);
    let removed = config::remove_mcp(&added, "other").unwrap();
    assert_eq!(removed, format!("{text}{TOOLPORT}"));
}

#[test]
fn ac10_02_add_mcp_writes_overrides_and_references_that_parse_back() {
    let c = parse(&with(TOOLPORT)).unwrap();
    let mut s = c.mcp[0].clone();
    s.name = "toolport2".into();
    s.env.insert("SECRET".into(), Value::Keychain("tp".into()));
    s.tools = vec![colctl::tool::Tool::Claude];
    let text = config::add_mcp(HARNESSES, &s).unwrap();
    let back = parse(&text).unwrap();
    assert_eq!(back.mcp, vec![s]);
}

#[test]
fn ac10_03_remove_mcp_names_a_missing_entry() {
    let e = config::remove_mcp(HARNESSES, "nope").unwrap_err();
    assert!(e.contains("no [[mcp]] named \"nope\""), "{e}");
}

#[test]
fn ac10_03_bad_entries_name_the_problem() {
    let cases = [
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nurl = \"https://x\"\n",
            "either command or url",
        ),
        ("[[mcp]]\nname = \"a\"\n", "either command or url"),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nheaders = { H = \"v\" }\n",
            "headers",
        ),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nbearer = { env = \"T\" }\n",
            "bearer",
        ),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\n[mcp.profile.nobody]\nenv = { A = \"b\" }\n",
            "\"nobody\"",
        ),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\n[[mcp]]\nname = \"a\"\ncommand = \"y\"\n",
            "duplicate",
        ),
        ("[[mcp]]\nname = \"a b\"\ncommand = \"x\"\n", "\"a b\""),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\ntools = [\"kilo\"]\n",
            "tools",
        ),
        (
            "[[mcp]]\nname = \"a\"\ncommand = \"x\"\ncolor = 1\n",
            "color",
        ),
    ];
    for (extra, needle) in cases {
        let e = err(extra);
        assert!(e.contains("mcp"), "{extra}: {e}");
        assert!(e.contains(needle), "{extra}: wanted {needle:?} in {e}");
        assert!(e.contains("line "), "{extra}: {e}");
    }
}

#[test]
fn ec10_04_env_keys_and_header_names_with_equals_or_newlines() {
    for extra in [
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { \"A=B\" = \"v\" }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { \"A\\nB\" = \"v\" }\n",
        "[[mcp]]\nname = \"a\"\nurl = \"https://x\"\nheaders = { \"H\\nI\" = \"v\" }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { \"\" = \"v\" }\n",
    ] {
        let e = err(extra);
        assert!(e.contains("\"a\""), "{extra}: {e}");
    }
}

#[test]
fn ec10_05_empty_or_mixed_references() {
    for extra in [
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { A = { env = \"\" } }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { A = { keychain = \"\" } }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { A = { env = \"X\", keychain = \"k\" } }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { A = { vault = \"X\" } }\n",
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\nenv = { A = 3 }\n",
    ] {
        let e = err(extra);
        assert!(e.contains("A"), "{extra}: {e}");
    }
}

#[test]
fn ec10_09_names_must_suit_both_tools() {
    for bad in ["A", "-a", "a.b", "a/b", &"a".repeat(65)] {
        let e = err(&format!("[[mcp]]\nname = \"{bad}\"\ncommand = \"x\"\n"));
        assert!(e.contains("[a-z0-9]"), "{bad}: {e}");
    }
    for good in ["a", "a-b", "a_b", "0x"] {
        parse(&with(&format!(
            "[[mcp]]\nname = \"{good}\"\ncommand = \"x\"\n"
        )))
        .unwrap();
    }
}

#[test]
fn ec10_29_an_override_for_agents_is_an_error() {
    let e =
        err("[[mcp]]\nname = \"a\"\ncommand = \"x\"\n[mcp.profile.agents]\nenv = { A = \"b\" }\n");
    assert!(e.contains("agents doesn't take MCP servers"), "{e}");
}

#[test]
fn ec10_29_an_override_for_a_profile_outside_tools_is_an_error() {
    let e = err(
        "[[mcp]]\nname = \"a\"\ncommand = \"x\"\ntools = [\"codex\"]\n[mcp.profile.claude]\nenv = { A = \"b\" }\n",
    );
    assert!(e.contains("\"claude\""), "{e}");
}

#[test]
fn ac10_01_the_config_hash_follows_the_entry_not_secret_values() {
    let c = parse(&with(TOOLPORT)).unwrap();
    let h = |name: &str| {
        c.mcp[0]
            .for_harness(c.harness(name).unwrap())
            .unwrap()
            .config_hash()
    };
    assert!(h("claude").starts_with("sha256:"));
    assert_ne!(h("claude"), h("kilo-claude"));
    assert_eq!(h("claude"), h("claude"));
    let changed = parse(&with(&TOOLPORT.replace("trimmed", "full"))).unwrap();
    let h2 = changed.mcp[0]
        .for_harness(changed.harness("claude").unwrap())
        .unwrap()
        .config_hash();
    assert_ne!(h("claude"), h2);
}

#[test]
fn ac10_01_state_records_mcp_and_old_state_still_parses() {
    let old: HarnessState =
        serde_json::from_str(r#"{"generated":{},"marketplaces":[],"plugins":[]}"#).unwrap();
    assert!(old.mcp.is_empty());
    let mut s = HarnessState::default();
    s.mcp.insert(
        "toolport".into(),
        McpRecord {
            config: "sha256:aa".into(),
            written: "sha256:bb".into(),
        },
    );
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<HarnessState>(&json).unwrap(), s);
    assert!(!s.is_empty());
    let empty = serde_json::to_string(&HarnessState::default()).unwrap();
    assert!(!empty.contains("mcp"), "{empty}");
}
