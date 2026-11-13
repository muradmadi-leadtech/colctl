//! Cycle 7: `colctl convert`, and reviewing, approving and rejecting a conversion. Named after
//! AC7-NN / EC7-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::Env;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.codex();
    env.ok(&["init"]);
    env
}

/// A Claude-only upstream with two skills (and `extra` files), added as a source and synced.
fn claude_only(env: &Env, name: &str, extra: &[(&str, &str)]) -> PathBuf {
    let repo = env.home.parent().unwrap().join("upstreams").join(name);
    write(&repo.join("skills/edge/SKILL.md"), "---\nname: edge\n---\n");
    write(
        &repo.join("skills/scaling/SKILL.md"),
        "---\nname: scaling\n---\n",
    );
    for (path, text) in extra {
        write(&repo.join(path), text);
    }
    let (repo, url, head) = env.upstream(name, true, false, Some(name));
    env.add_source_entry(name, &url, &head, "");
    env.ok(&["sync"]);
    repo
}

const HOOKS: &str = r#"{"hooks": {
    "SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "python3 \"${CLAUDE_PLUGIN_ROOT}/hooks/run.py\""}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "say hi"}]}]
}}"#;

fn hooks_fixture(env: &Env) -> PathBuf {
    claude_only(
        env,
        "dbx",
        &[
            ("hooks/hooks.json", HOOKS),
            ("hooks/run.py", "print(1)"),
            ("commands/setup.md", "setup"),
            ("agents/helper.md", "---\nname: helper\n---\n"),
            (
                ".mcp.json",
                r#"{"mcpServers": {"github": {"command": "gh"}}}"#,
            ),
        ],
    )
}

fn state(env: &Env) -> Value {
    serde_json::from_str(
        &fs::read_to_string(env.source.join(".state/state.json")).unwrap_or("{}".into()),
    )
    .unwrap()
}

#[test]
fn ac7_07_convert_holds_a_package_for_review() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let config = env.config();
    let calls = env.codex_log().len();
    let (code, out, err) = env.code(&["convert", "qdrant"]);
    assert_eq!(code, 0, "{out}{err}");
    let words = Env::words(&out);
    assert!(
        out.starts_with("! Held qdrant's Codex package for review"),
        "{out}"
    );
    assert!(words.contains("Carries 2 skills"), "{out}");
    assert!(out.trim_end().ends_with("→ colctl review qdrant"), "{out}");
    assert_eq!(env.config(), config, "config unchanged");
    assert_eq!(env.codex_log().len(), calls, "nothing installed");
    let held = &state(&env)["quarantine"]["conversions/qdrant"];
    assert_eq!(held["kind"], "conversion", "{held}");
    assert!(
        env.source
            .join(".state/quarantine/conversions/qdrant/.codex-plugin/plugin.json")
            .is_file()
    );
    let (code, out, _) = env.code(&["status"]);
    assert_eq!(code, 1, "{out}");
    let words = Env::words(&out);
    assert!(words.contains("held conversion qdrant"), "{out}");
    assert!(words.contains("→ colctl review qdrant"), "{out}");
}

#[test]
fn ac7_07_convert_json() {
    let env = machine();
    hooks_fixture(&env);
    let out = env.ok(&["convert", "dbx", "--json"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["source"], "dbx");
    assert_eq!(v["held"], true);
    assert_eq!(v["dry_run"], false);
    assert_eq!(v["rev"].as_str().unwrap().len(), 40);
    assert_eq!(v["digest"].as_str().unwrap().len(), 64);
    assert_eq!(v["skills"], json!(["edge", "scaling"]));
    assert_eq!(v["hooks"][0]["event"], "SessionStart");
    assert_eq!(v["hooks"][0]["matcher"], "startup");
    assert_eq!(
        v["never_runs"],
        json!(["Notification hook (Codex has no such event)"])
    );
    assert_eq!(
        v["not_carried"],
        json!([{"kind": "command", "name": "setup"}, {"kind": "agent", "name": "helper"},
               {"kind": "MCP server", "name": "github"}])
    );
}

#[test]
fn ac7_07_convert_text_lists_hooks_and_what_does_not_carry() {
    let env = machine();
    hooks_fixture(&env);
    let out = env.ok(&["convert", "dbx"]);
    let words = Env::words(&out);
    assert!(
        words.contains("Carries 2 skills · 1 hook (SessionStart on startup)"),
        "{out}"
    );
    assert!(
        words.contains("Codex never runs Notification hook (Codex has no such event)"),
        "{out}"
    );
    assert!(
        words.contains("Not carried over command setup · agent helper · MCP server github"),
        "{out}"
    );
}

#[test]
fn ac7_08_review_shows_a_held_conversion() {
    let env = machine();
    hooks_fixture(&env);
    env.ok(&["convert", "dbx"]);
    let out = env.ok(&["review", "dbx", "--color", "never"]);
    let words = Env::words(&out);
    assert!(words.contains("dbx Codex package from"), "{out}");
    for want in [
        "Skills edge, scaling",
        "SessionStart startup python3 \"${CLAUDE_PLUGIN_ROOT}/hooks/run.py\"",
        "Codex never runs",
        "Notification hook (Codex has no such event)",
        "Not carried over",
        "command setup",
    ] {
        assert!(words.contains(want), "{want} in {out}");
    }
    assert!(words.contains("hook"), "risk flag: {out}");
    let v: Value = serde_json::from_str(&env.ok(&["review", "dbx", "--json"])).unwrap();
    let c = &v["candidates"][0];
    assert_eq!(c["kind"], "conversion", "{v}");
    assert_eq!(c["name"], "dbx");
    assert_eq!(c["skills"], json!(["edge", "scaling"]));
    assert_eq!(c["hooks"][0]["event"], "SessionStart");
    assert_eq!(c["never_runs"].as_array().unwrap().len(), 1);
    assert_eq!(c["not_carried"].as_array().unwrap().len(), 3);
    assert!(
        c["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "hook"),
        "{c}"
    );
}

#[test]
fn ac7_09_approve_records_the_conversion() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    env.ok(&["convert", "qdrant"]);
    env.ok(&["approve", "qdrant"]);
    assert!(
        env.config().contains("convert = [\"codex\"]"),
        "{}",
        env.config()
    );
    assert!(
        state(&env)["quarantine"]
            .get("conversions/qdrant")
            .is_none()
    );
    assert!(
        !env.source
            .join(".state/quarantine/conversions/qdrant")
            .exists()
    );
    // Committed in ~/.colctl.
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&env.source)
        .args(["status", "--porcelain", "config.toml"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());
}

#[test]
fn ac7_10_reject_drops_it() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let config = env.config();
    env.ok(&["convert", "qdrant"]);
    env.ok(&["reject", "qdrant"]);
    assert_eq!(env.config(), config);
    assert!(
        state(&env)["quarantine"]
            .get("conversions/qdrant")
            .is_none()
    );
    assert_eq!(
        env.item_state("codex", "source", "qdrant").as_deref(),
        Some("unsupported")
    );
    // Asking again holds it again.
    env.ok(&["convert", "qdrant"]);
    assert_eq!(
        state(&env)["quarantine"]["conversions/qdrant"]["kind"],
        "conversion"
    );
}

#[test]
fn ac7_12_a_dry_run_writes_nothing() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let before = state(&env);
    let out = env.ok(&["convert", "qdrant", "--dry-run"]);
    assert!(
        out.starts_with("· Would hold qdrant's Codex package"),
        "{out}"
    );
    assert_eq!(state(&env), before);
    assert!(!env.source.join(".state/quarantine/conversions").exists());
    let v: Value =
        serde_json::from_str(&env.ok(&["convert", "qdrant", "--dry-run", "--json"])).unwrap();
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["held"], false);
}

#[test]
fn ec7_01_an_unknown_source() {
    let env = machine();
    let (code, _, err) = env.code(&["convert", "nope"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no source named \"nope\""), "{err}");
    assert!(err.contains("hint: colctl status"), "{err}");
}

#[test]
fn ec7_02_a_source_that_packages_for_codex() {
    let env = machine();
    let (_, url, head) = env.upstream("both", true, true, Some("m"));
    env.add_source_entry("both", &url, &head, "");
    env.ok(&["sync"]);
    let out = env.ok(&["convert", "both"]);
    assert!(out.contains("both already packages for Codex"), "{out}");
    assert!(state(&env)["quarantine"].get("conversions/both").is_none());
}

#[test]
fn ec7_03_already_converted() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let config = env
        .config()
        .replace("rev = ", "convert = [\"codex\"]\nrev = ");
    env.write_config(&config);
    let out = env.ok(&["convert", "qdrant"]);
    assert!(out.contains("qdrant is already converted"), "{out}");
}

#[test]
fn ec7_04_converting_again_while_held() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    env.ok(&["convert", "qdrant"]);
    let first = state(&env)["quarantine"]["conversions/qdrant"]["digest"].clone();
    let out = env.ok(&["convert", "qdrant"]);
    assert!(out.contains("→ colctl review qdrant"), "{out}");
    assert_eq!(
        state(&env)["quarantine"]["conversions/qdrant"]["digest"],
        first
    );
    assert_eq!(state(&env)["quarantine"].as_object().unwrap().len(), 1);
}

/// A Codex-only source has nothing to convert from; it already packages for Codex, which is what's
/// said (the library's own error is `repackage::ec7_05_…`).
#[test]
fn ec7_05_nothing_to_convert_from() {
    let env = machine();
    let (_, url, head) = env.upstream("codexonly", false, true, Some("m"));
    env.add_source_entry("codexonly", &url, &head, "");
    env.ok(&["sync"]);
    let out = env.ok(&["convert", "codexonly"]);
    assert!(
        out.contains("codexonly already packages for Codex"),
        "{out}"
    );
}

#[test]
fn ec7_06_a_broken_hooks_file_holds_nothing() {
    let env = machine();
    claude_only(&env, "bad", &[("hooks/hooks.json", "{nope")]);
    let (code, _, err) = env.code(&["convert", "bad"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("hooks.json"), "{err}");
    assert!(state(&env)["quarantine"].get("conversions/bad").is_none());
}

#[test]
fn ec7_10_no_codex_profile() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    claude_only(&env, "qdrant", &[]);
    let (code, _, err) = env.code(&["convert", "qdrant"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no Codex profile is registered"), "{err}");
    assert!(err.contains("hint: colctl init"), "{err}");
}

#[test]
fn ec7_11_nothing_codex_can_use() {
    let env = machine();
    let repo = env.home.parent().unwrap().join("upstreams/cmds");
    write(&repo.join("commands/setup.md"), "setup");
    let (_, url, head) = env.upstream("cmds", true, false, Some("cmds"));
    env.add_source_entry("cmds", &url, &head, "");
    env.ok(&["sync"]);
    let (code, _, err) = env.code(&["convert", "cmds"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("cmds has nothing Codex can use"), "{err}");
    assert!(state(&env)["quarantine"].get("conversions/cmds").is_none());
}

// ---------------------------------------------------------------- sync and status (T5)

/// qdrant, converted and approved.
fn converted(env: &Env) -> PathBuf {
    let repo = claude_only(env, "qdrant", &[]);
    env.ok(&["convert", "qdrant"]);
    env.ok(&["approve", "qdrant"]);
    repo
}

fn codex_item(env: &Env, name: &str) -> Value {
    let v = env.status_json();
    v["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "codex")
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "source" && i["name"] == name)
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn ac7_09_approve_installs_it_in_codex() {
    let env = machine();
    converted(&env);
    let codex = env.home.join(".codex");
    assert_eq!(env.codex_installed(&codex), vec!["qdrant@colctl-qdrant"]);
    assert_eq!(
        env.installed(&env.home.join(".claude")),
        vec!["qdrant@qdrant"]
    );
    let package = env.source.join("plugins/.converted/qdrant");
    assert!(package.join(".codex-plugin/plugin.json").is_file());
    assert!(package.join("skills/edge/SKILL.md").is_file());
    let (code, out, _) = env.code(&["status", "--color", "never"]);
    assert_eq!(code, 0, "{out}");
    let line = out.lines().find(|l| l.contains(" codex ")).unwrap();
    assert!(Env::words(line).ends_with("1 plugin"), "{out}");
    assert!(!out.contains("not packaged"), "{out}");
    let item = codex_item(&env, "qdrant");
    assert_eq!(item["state"], "ok", "{item}");
    assert_eq!(item["converted"], true, "{item}");
    let v = env.status_json();
    assert_eq!(v["sources"][0]["convert"], json!(["codex"]));
    // Converged: nothing more to do.
    let calls = env.codex_log().len();
    env.ok(&["sync"]);
    assert_eq!(env.codex_log().len(), calls);
}

#[test]
fn ac7_09_approving_hooks_says_codex_asks_to_trust_them() {
    let env = machine();
    hooks_fixture(&env);
    env.ok(&["convert", "dbx"]);
    let out = env.ok(&["approve", "dbx"]);
    assert!(
        Env::words(&out).contains(
            "Codex asks you to trust its hooks before they run: open Codex and use /hooks"
        ),
        "{out}"
    );
    assert!(
        env.source
            .join("plugins/.converted/dbx/hooks/hooks.json")
            .is_file()
    );
}

#[test]
fn ac7_11_undo() {
    let env = machine();
    converted(&env);
    let before = env.config();
    let out = env.ok(&["convert", "--undo", "qdrant", "--dry-run"]);
    assert!(out.contains("Would stop converting qdrant"), "{out}");
    assert_eq!(env.config(), before);
    let out = env.ok(&["convert", "--undo", "qdrant"]);
    assert!(out.contains("Stopped converting qdrant"), "{out}");
    assert!(env.codex_installed(&env.home.join(".codex")).is_empty());
    assert!(!env.source.join("plugins/.converted/qdrant").exists());
    assert!(!env.config().contains("convert"), "{}", env.config());
    assert!(
        !env.source
            .join("plugins/.manifests/qdrant/.agents/plugins/marketplace.json")
            .exists()
            || !fs::read_to_string(
                env.source
                    .join("plugins/.manifests/qdrant/.agents/plugins/marketplace.json")
            )
            .unwrap()
            .contains("plugin-codex")
    );
    assert_eq!(codex_item(&env, "qdrant")["state"], "unsupported");
    let v: Value =
        serde_json::from_str(&env.ok(&["convert", "--undo", "qdrant", "--json"])).unwrap();
    assert_eq!(v["undone"], false, "{v}");
}

#[test]
fn ac7_13_status_points_at_convert() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let (code, out, _) = env.code(&["status", "--all", "--color", "never"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        Env::words(&out).contains("unsupported source qdrant → colctl convert qdrant"),
        "{out}"
    );
}

#[test]
fn ac7_14_sync_keeps_the_package_as_approved() {
    let env = machine();
    converted(&env);
    let package = env.source.join("plugins/.converted/qdrant");
    for damage in ["delete", "edit"] {
        if damage == "delete" {
            fs::remove_dir_all(&package).unwrap();
        } else {
            fs::write(package.join("skills/edge/SKILL.md"), "tampered").unwrap();
        }
        let (code, out, _) = env.code(&["status"]);
        assert_eq!(code, 1, "{damage}: {out}");
        assert_eq!(codex_item(&env, "qdrant")["state"], "outdated", "{damage}");
        env.ok(&["sync"]);
        assert_eq!(
            fs::read_to_string(package.join("skills/edge/SKILL.md")).unwrap(),
            "---\nname: edge\n---\n",
            "{damage}"
        );
        env.ok(&["status"]);
    }
}

#[test]
fn ac7_15_native_packaging_wins() {
    let env = machine();
    let repo = converted(&env);
    write(
        &repo.join(".codex-plugin/plugin.json"),
        &json!({"name": "qdrant"}).to_string(),
    );
    write(
        &repo.join(".agents/plugins/marketplace.json"),
        &json!({"name": "qdrant", "plugins": [{"name": "qdrant", "source": {"source": "local", "path": "./"}}]}).to_string(),
    );
    let head = env.upstream_commit(&repo, "v2");
    let old = env.config();
    let rev = old
        .lines()
        .find(|l| l.starts_with("rev = "))
        .unwrap()
        .to_string();
    env.write_config(&old.replace(&rev, &format!("rev = \"{head}\"")));
    let out = env.ok(&["sync"]);
    assert!(
        Env::words(&out).contains(
            "qdrant now packages for Codex itself; colctl's conversion is no longer used"
        ),
        "{out}"
    );
    assert_eq!(
        env.codex_installed(&env.home.join(".codex")),
        vec!["qdrant@qdrant"]
    );
    assert!(!env.source.join("plugins/.converted/qdrant").exists());
    assert!(!env.config().contains("convert"), "{}", env.config());
    env.ok(&["status"]);
}

#[test]
fn ec7_14_undo_when_not_converted() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    let out = env.ok(&["convert", "--undo", "qdrant"]);
    assert!(out.contains("qdrant isn't converted"), "{out}");
}

#[test]
fn ec7_15_undo_drops_a_held_conversion() {
    let env = machine();
    claude_only(&env, "qdrant", &[]);
    env.ok(&["convert", "qdrant"]);
    env.ok(&["convert", "--undo", "qdrant"]);
    assert!(
        state(&env)["quarantine"]
            .get("conversions/qdrant")
            .is_none()
    );
    assert!(
        !env.source
            .join(".state/quarantine/conversions/qdrant")
            .exists()
    );
}

#[test]
fn ec7_16_removing_the_source_removes_its_conversion() {
    let env = machine();
    converted(&env);
    env.ok(&["rm", "source", "qdrant", "-y"]);
    assert!(env.codex_installed(&env.home.join(".codex")).is_empty());
    assert!(!env.source.join("plugins/.converted/qdrant").exists());
}

#[test]
fn ec7_20_a_different_conversion_reinstalls_once() {
    let env = machine();
    converted(&env);
    // What a colctl that converted differently would have recorded.
    let path = env.source.join(".state/state.json");
    let text = fs::read_to_string(&path).unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    let rec = &mut v["harnesses"]["codex"]["sources"]["qdrant"]["rev"];
    let rev = rec.as_str().unwrap().to_string();
    assert!(rev.contains('+'), "{rev}");
    *rec = json!(format!("{}+000000000000", rev.split('+').next().unwrap()));
    fs::write(&path, v.to_string()).unwrap();
    assert_eq!(codex_item(&env, "qdrant")["state"], "outdated");
    let calls = env.codex_log().len();
    env.ok(&["sync"]);
    assert!(env.codex_log().len() > calls);
    let calls = env.codex_log().len();
    env.ok(&["sync"]);
    assert_eq!(env.codex_log().len(), calls);
}
