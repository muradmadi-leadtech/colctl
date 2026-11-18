//! Cycle 10, T6: hooks from `~/.colctl/hooks`, carried by colctl's own `colctl-setup` plugin. Named
//! after AC10-NN / EC10-NN.

mod support;

use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
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

const ID: &str = "setup@colctl-setup";

fn hooks(env: &Env, groups: serde_json::Value) {
    let dir = env.source.join("hooks");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("hooks.json"),
        serde_json::to_string_pretty(&json!({ "hooks": groups })).unwrap(),
    )
    .unwrap();
    let guard = dir.join("guard.sh");
    fs::write(&guard, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&guard, fs::Permissions::from_mode(0o755)).unwrap();
}

fn guard_hook() -> serde_json::Value {
    json!({"PreToolUse": [{"matcher": "Bash", "hooks": [
        {"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/guard.sh"}]}]})
}

fn state(env: &Env, harness: &str) -> Option<String> {
    env.item_state(harness, "source", "colctl-setup")
}

#[test]
fn ac10_50_hooks_reach_claude_through_colctls_plugin() {
    let env = machine();
    hooks(&env, guard_hook());
    env.ok(&["sync"]);
    let calls = env.log_args();
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("plugin marketplace add ") && c.contains("colctl-setup")),
        "{calls:?}"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| *c == &format!("plugin install {ID}"))
            .count(),
        2,
        "{calls:?}"
    );
    for p in [env.native(), env.kilo()] {
        assert!(
            env.installed(&p).contains(&ID.to_string()),
            "{}",
            p.display()
        );
    }
    // The package Claude installs: the hooks file and the script beside the plugin root.
    let pkg = env.source.join("plugins/.manifests/colctl-setup/plugin");
    let hooks_json = fs::read_to_string(pkg.join("hooks/hooks.json")).unwrap();
    assert!(hooks_json.contains("guard.sh"), "{hooks_json}");
    let mode = fs::metadata(pkg.join("guard.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0);
    for h in ["claude", "kilo-claude"] {
        assert_eq!(state(&env, h).as_deref(), Some("ok"), "{h}");
    }
    // Nothing generated is committed.
    let tracked = env.git(&["ls-files"]);
    assert!(!tracked.contains(".manifests"), "{tracked}");
    let n = env.log_args().len();
    env.ok(&["sync"]);
    assert_eq!(env.log_args().len(), n);
}

#[test]
fn ac10_51_codex_gets_the_mapped_hooks() {
    let env = machine();
    hooks(
        &env,
        json!({
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/guard.sh"}]}],
            "Notification": [{"hooks": [{"type": "command", "command": "say hi"}]}],
            "Stop": [{"hooks": [{"type": "prompt", "prompt": "check"}]}]
        }),
    );
    env.ok(&["sync"]);
    for p in [env.codex(), env.kilo_codex()] {
        assert!(
            env.codex_installed(&p).contains(&ID.to_string()),
            "{}",
            p.display()
        );
    }
    let codex_hooks = fs::read_to_string(
        env.source
            .join("plugins/.manifests/colctl-setup/plugin-codex/hooks/hooks.json"),
    )
    .unwrap();
    assert!(codex_hooks.contains("PreToolUse"), "{codex_hooks}");
    assert!(!codex_hooks.contains("Notification"), "{codex_hooks}");
    assert!(!codex_hooks.contains("prompt"), "{codex_hooks}");
    assert_eq!(state(&env, "codex").as_deref(), Some("ok"));
}

#[test]
fn ac10_51_hooks_codex_never_runs_are_unsupported_there() {
    let env = machine();
    hooks(
        &env,
        json!({"Notification": [{"hooks": [{"type": "command", "command": "say hi"}]}]}),
    );
    env.ok(&["sync"]);
    assert!(!env.codex_installed(&env.codex()).contains(&ID.to_string()));
    assert_eq!(state(&env, "codex").as_deref(), Some("unsupported"));
    assert_eq!(state(&env, "claude").as_deref(), Some("ok"));
}

#[test]
fn ac10_52_a_change_reinstalls() {
    let env = machine();
    hooks(&env, guard_hook());
    env.ok(&["sync"]);
    fs::write(env.source.join("hooks/guard.sh"), "#!/bin/sh\nexit 2\n").unwrap();
    assert_eq!(state(&env, "claude").as_deref(), Some("outdated"));
    let n = env.log_args().len();
    env.ok(&["sync"]);
    let calls = env.log_args()[n..].to_vec();
    assert!(calls.contains(&format!("plugin install {ID}")), "{calls:?}");
    assert_eq!(
        fs::read_to_string(
            env.source
                .join("plugins/.manifests/colctl-setup/plugin/guard.sh")
        )
        .unwrap(),
        "#!/bin/sh\nexit 2\n"
    );
    assert_eq!(state(&env, "claude").as_deref(), Some("ok"));
}

#[test]
fn ac10_53_adopt_hooks_from_settings_json() {
    let env = machine();
    let settings = env.native().join("settings.json");
    let text = serde_json::to_string_pretty(&json!({"model": "opus", "hooks": {
        "PostToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "fmt.sh"}]}]
    }}))
    .unwrap();
    fs::write(&settings, &text).unwrap();
    let out = env.ok(&["adopt", "claude", "hooks"]);
    assert_eq!(fs::read_to_string(&settings).unwrap(), text);
    let ours = fs::read_to_string(env.source.join("hooks/hooks.json")).unwrap();
    assert!(
        ours.contains("PostToolUse") && ours.contains("fmt.sh"),
        "{ours}"
    );
    assert!(out.contains("Remove these from"), "{out}");
    assert!(out.contains("run twice"), "{out}");
    assert!(env.installed(&env.kilo()).contains(&ID.to_string()));
    // Adopting again adds nothing twice.
    env.ok(&["adopt", "claude", "hooks"]);
    let again = fs::read_to_string(env.source.join("hooks/hooks.json")).unwrap();
    assert_eq!(again.matches("fmt.sh").count(), 1, "{again}");
}

#[test]
fn ac10_54_nothing_to_carry_no_plugin() {
    let env = machine();
    env.ok(&["sync"]);
    assert!(!env.log_args().iter().any(|c| c.contains("colctl-setup")));
    hooks(&env, guard_hook());
    env.ok(&["sync"]);
    fs::remove_dir_all(env.source.join("hooks")).unwrap();
    env.ok(&["sync"]);
    let calls = env.log_args();
    assert!(
        calls.contains(&format!("plugin uninstall {ID}")),
        "{calls:?}"
    );
    assert!(!env.installed(&env.native()).contains(&ID.to_string()));
    assert_eq!(state(&env, "claude"), None);
}

#[test]
fn ec10_20_a_broken_hooks_file_leaves_the_plugin_as_it_was() {
    let env = machine();
    hooks(&env, guard_hook());
    env.ok(&["sync"]);
    fs::write(env.source.join("hooks/hooks.json"), "{ not json").unwrap();
    let (code, _, _) = env.code(&["sync"]);
    assert_ne!(code, 0);
    assert_eq!(state(&env, "claude").as_deref(), Some("invalid"));
    assert!(env.installed(&env.native()).contains(&ID.to_string()));
    let text = env.expect(1, &["status"]);
    assert!(text.contains("hooks.json"), "{text}");
}
