//! Cycle 7: building a Codex package from a Claude-only plugin folder. Named after AC7-NN / EC7-NN.

use colctl::repackage;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The qdrant fixture: Claude manifests and two skills.
fn qdrant(root: &Path) -> PathBuf {
    let p = root.join("qdrant");
    write(
        &p.join(".claude-plugin/plugin.json"),
        &json!({"name": "qdrant", "version": "1.0.0", "description": "Qdrant skills",
                "author": {"name": "Qdrant"}, "homepage": "https://qdrant.tech",
                "repository": "https://github.com/qdrant/skills", "license": "Apache-2.0",
                "keywords": ["vector", "search"]})
        .to_string(),
    );
    write(
        &p.join(".claude-plugin/marketplace.json"),
        &json!({"name": "qdrant", "plugins": [{"name": "qdrant", "source": "./"}]}).to_string(),
    );
    write(
        &p.join("skills/edge/SKILL.md"),
        "---\nname: edge\n---\nEdge.\n",
    );
    write(
        &p.join("skills/scaling/SKILL.md"),
        "---\nname: scaling\n---\n",
    );
    write(&p.join("skills/scaling/refs/sizing.md"), "sizes");
    write(&p.join("README.md"), "readme");
    write(&p.join(".git/HEAD"), "ref: refs/heads/main\n");
    p
}

/// Every file under `dir`, relative, sorted, with its bytes.
fn tree(dir: &Path) -> Vec<(String, Vec<u8>, u32)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let meta = fs::symlink_metadata(e.path()).unwrap();
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                let rel = e.path().strip_prefix(dir).unwrap().display().to_string();
                out.push((
                    rel,
                    fs::read(e.path()).unwrap(),
                    meta.permissions().mode() & 0o777,
                ));
            }
        }
    }
    out.sort();
    out
}

fn manifest(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join(".codex-plugin/plugin.json")).unwrap())
        .unwrap()
}

#[test]
fn ac7_01_a_codex_manifest_from_claudes() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = repackage::build(&qdrant(tmp.path()), "fallback").unwrap();
    assert_eq!(
        pkg.manifest,
        json!({"name": "qdrant", "version": "1.0.0", "description": "Qdrant skills",
               "author": {"name": "Qdrant"}, "homepage": "https://qdrant.tech",
               "repository": "https://github.com/qdrant/skills", "license": "Apache-2.0",
               "keywords": ["vector", "search"], "skills": "./skills/",
               "interface": {"displayName": "qdrant"}})
    );
    assert_eq!(pkg.skills, vec!["edge", "scaling"]);
    // Only what Claude's manifest has.
    let bare = tmp.path().join("bare");
    write(
        &bare.join(".claude-plugin/plugin.json"),
        r#"{"name": "bare"}"#,
    );
    write(&bare.join("skills/a/SKILL.md"), "a");
    let pkg = repackage::build(&bare, "x").unwrap();
    assert_eq!(
        pkg.manifest,
        json!({"name": "bare", "skills": "./skills/", "interface": {"displayName": "bare"}})
    );
}

#[test]
fn ac7_02_the_plugin_folder_is_carried_over_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    let before = tree(&src);
    let pkg = repackage::build(&src, "x").unwrap();
    let dest = tmp.path().join("out/qdrant");
    repackage::write(&pkg, &dest).unwrap();
    let files: Vec<String> = tree(&dest).into_iter().map(|(p, _, _)| p).collect();
    assert_eq!(
        files,
        vec![
            ".codex-plugin/plugin.json",
            "README.md",
            "skills/edge/SKILL.md",
            "skills/scaling/SKILL.md",
            "skills/scaling/refs/sizing.md",
        ]
    );
    assert_eq!(
        fs::read_to_string(dest.join("skills/edge/SKILL.md")).unwrap(),
        "---\nname: edge\n---\nEdge.\n"
    );
    assert_eq!(manifest(&dest)["name"], "qdrant");
    // The source folder is untouched.
    assert_eq!(tree(&src), before);
}

#[test]
fn ac7_03_the_same_commit_gives_the_same_package() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    let a = repackage::build(&src, "x").unwrap();
    let da = repackage::write(&a, &tmp.path().join("a")).unwrap();
    // Different mtimes and an odd mode in the source must not matter.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::set_permissions(src.join("README.md"), fs::Permissions::from_mode(0o600)).unwrap();
    let b = repackage::build(&src, "x").unwrap();
    let db = repackage::write(&b, &tmp.path().join("b")).unwrap();
    assert_eq!(da, db);
    assert_eq!(a.digest(), da);
    assert_eq!(da.len(), 64);
    assert_eq!(tree(&tmp.path().join("a")), tree(&tmp.path().join("b")));
    let text = fs::read_to_string(tmp.path().join("a/.codex-plugin/plugin.json")).unwrap();
    assert!(text.ends_with("}\n"), "{text}");
    assert!(
        text.contains("\n  \"author\""),
        "two-space indent, sorted: {text}"
    );
    // A change in content changes the digest.
    write(&src.join("README.md"), "changed");
    assert_ne!(repackage::build(&src, "x").unwrap().digest(), da);
}

#[test]
fn ac7_06_what_does_not_carry_over_is_named() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    write(&src.join("commands/setup.md"), "setup");
    write(&src.join("commands/doctor.md"), "doctor");
    write(&src.join("agents/helper.md"), "---\nname: helper\n---\n");
    write(
        &src.join(".mcp.json"),
        r#"{"mcpServers": {"github": {"command": "gh"}, "db": {"command": "db"}}}"#,
    );
    write(
        &src.join(".lsp.json"),
        r#"{"rust": {"command": "rust-analyzer"}}"#,
    );
    write(
        &src.join(".claude-plugin/plugin.json"),
        r#"{"name": "qdrant", "outputStyles": "./styles/"}"#,
    );
    write(&src.join("styles/terse.md"), "terse");
    let pkg = repackage::build(&src, "x").unwrap();
    let names: Vec<String> = pkg
        .report
        .not_carried
        .iter()
        .map(|n| format!("{} {}", n.kind, n.name))
        .collect();
    assert_eq!(
        names,
        vec![
            "command doctor",
            "command setup",
            "agent helper",
            "MCP server db",
            "MCP server github",
            "LSP server rust",
            "output style terse",
        ]
    );
}

#[test]
fn ec7_07_a_manifest_without_a_name_uses_the_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    write(
        &src.join(".claude-plugin/plugin.json"),
        r#"{"version": "2.0.0"}"#,
    );
    let pkg = repackage::build(&src, "qdrant-skills").unwrap();
    assert_eq!(pkg.manifest["name"], "qdrant-skills");
    assert_eq!(pkg.manifest["interface"]["displayName"], "qdrant-skills");
}

#[test]
fn ec7_08_manifest_paths_must_stay_inside() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    for skills in ["../elsewhere", "/etc", "./a/../../x"] {
        write(
            &src.join(".claude-plugin/plugin.json"),
            &json!({"name": "q", "skills": skills}).to_string(),
        );
        let err = repackage::build(&src, "x").unwrap_err();
        assert!(err.contains("outside the plugin"), "{skills}: {err}");
    }
    // A path inside is used as given.
    write(&src.join("more/one/SKILL.md"), "one");
    write(
        &src.join(".claude-plugin/plugin.json"),
        r#"{"name": "q", "skills": "./more/"}"#,
    );
    let pkg = repackage::build(&src, "x").unwrap();
    assert_eq!(pkg.manifest["skills"], "./more/");
    assert_eq!(pkg.skills, vec!["one"]);
}

#[test]
fn ec7_09_links_leaving_the_folder_are_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    write(&tmp.path().join("secret.txt"), "secret");
    std::os::unix::fs::symlink(tmp.path().join("secret.txt"), src.join("leak.txt")).unwrap();
    std::os::unix::fs::symlink("../..", src.join("skills/up")).unwrap();
    // A link that stays inside is copied as the file it points at.
    std::os::unix::fs::symlink("README.md", src.join("README-link.md")).unwrap();
    let pkg = repackage::build(&src, "x").unwrap();
    let dest = tmp.path().join("out");
    repackage::write(&pkg, &dest).unwrap();
    assert!(!dest.join("leak.txt").exists());
    assert!(!dest.join("skills/up").exists());
    assert_eq!(
        fs::read_to_string(dest.join("README-link.md")).unwrap(),
        "readme"
    );
    let names: Vec<String> = pkg
        .report
        .not_carried
        .iter()
        .map(|n| format!("{} {}", n.kind, n.name))
        .collect();
    assert!(
        names.contains(&"link leaving the plugin leak.txt".to_string()),
        "{names:?}"
    );
    assert!(
        names.contains(&"link leaving the plugin skills/up".to_string()),
        "{names:?}"
    );
    assert_eq!(pkg.skills, vec!["edge", "scaling"]);
}

#[test]
fn ec7_17_bytes_and_executable_bits_are_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    fs::write(src.join("blob.bin"), [0u8, 159, 146, 150, 255]).unwrap();
    write(&src.join("bin/run.sh"), "#!/bin/sh\n");
    fs::set_permissions(src.join("bin/run.sh"), fs::Permissions::from_mode(0o775)).unwrap();
    let dest = tmp.path().join("out");
    repackage::write(&repackage::build(&src, "x").unwrap(), &dest).unwrap();
    assert_eq!(
        fs::read(dest.join("blob.bin")).unwrap(),
        vec![0u8, 159, 146, 150, 255]
    );
    let mode = |p: &str| fs::metadata(dest.join(p)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode("bin/run.sh"), 0o755);
    assert_eq!(mode("blob.bin"), 0o644);
}

#[test]
fn ec7_19_a_package_is_replaced_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    let dest = tmp.path().join("out/qdrant");
    write(&dest.join("stale.txt"), "old");
    repackage::write(&repackage::build(&src, "x").unwrap(), &dest).unwrap();
    assert!(!dest.join("stale.txt").exists());
    // No temp folder is left next to it.
    let left: Vec<_> = fs::read_dir(tmp.path().join("out"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(left, vec!["qdrant"]);
}

#[test]
fn ec7_05_no_claude_manifest_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    fs::remove_dir_all(src.join(".claude-plugin")).unwrap();
    let err = repackage::build(&src, "x").unwrap_err();
    assert!(err.contains("no Claude packaging"), "{err}");
}

// ---------------------------------------------------------------- hooks (T2)

fn hooks_fixture(root: &Path, hooks: &Value) -> PathBuf {
    let src = qdrant(root);
    write(&src.join("hooks/hooks.json"), &hooks.to_string());
    write(&src.join("hooks/run.py"), "print('hi')");
    src
}

fn group(matcher: Option<&str>, command: &str) -> Value {
    let mut g = json!({"hooks": [{"type": "command", "command": command}]});
    if let Some(m) = matcher {
        g["matcher"] = json!(m);
    }
    g
}

fn package_hooks(dest: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dest.join("hooks/hooks.json")).unwrap()).unwrap()
}

#[test]
fn ac7_04_hooks_codex_runs_are_kept_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let run = "python3 \"${CLAUDE_PLUGIN_ROOT}/hooks/run.py\"";
    let hooks = json!({"description": "d", "hooks": {
        "UserPromptSubmit": [group(None, run)],
        "SessionStart": [group(Some("startup|clear"), run)],
        "PreToolUse": [group(Some("Bash"), run), group(Some("mcp__github__.*"), run)],
        "PostToolUse": [{"matcher": "Edit|Write", "hooks": [{"type": "command", "command": run, "timeout": 30}]}],
    }});
    let src = hooks_fixture(tmp.path(), &hooks);
    let pkg = repackage::build(&src, "x").unwrap();
    let dest = tmp.path().join("out");
    repackage::write(&pkg, &dest).unwrap();
    assert_eq!(package_hooks(&dest), hooks);
    assert_eq!(
        fs::read_to_string(dest.join("hooks/run.py")).unwrap(),
        "print('hi')"
    );
    assert!(pkg.report.never_runs.is_empty(), "{:?}", pkg.report);
    let kept: Vec<String> = pkg
        .hooks
        .iter()
        .map(|h| format!("{} {}", h.event, h.matcher.clone().unwrap_or_default()))
        .collect();
    assert_eq!(
        kept,
        vec![
            "PostToolUse Edit|Write",
            "PreToolUse Bash",
            "PreToolUse mcp__github__.*",
            "SessionStart startup|clear",
            "UserPromptSubmit ",
        ]
    );
    assert!(
        pkg.hooks
            .iter()
            .all(|h| h.kind == "command" && h.command == run)
    );
}

#[test]
fn ac7_05_hooks_codex_never_runs_are_reported_not_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let hooks = json!({"hooks": {
        "Notification": [group(None, "notify")],
        "PreToolUse": [group(Some("Read|Grep"), "a"), group(Some("Bash"), "b")],
        "PostToolUse": [group(Some("Bash|WebFetch"), "c")],
        "Stop": [{"hooks": [{"type": "prompt", "prompt": "check"}, {"type": "command", "command": "d"}]}],
    }});
    let src = hooks_fixture(tmp.path(), &hooks);
    let pkg = repackage::build(&src, "x").unwrap();
    let dest = tmp.path().join("out");
    repackage::write(&pkg, &dest).unwrap();
    assert_eq!(
        package_hooks(&dest),
        json!({"hooks": {
            "PreToolUse": [group(Some("Bash"), "b")],
            "PostToolUse": [group(Some("Bash"), "c")],
            "Stop": [group(None, "d")],
        }})
    );
    assert_eq!(
        pkg.report.never_runs,
        vec![
            "Notification hook (Codex has no such event)",
            "WebFetch in PostToolUse's matcher",
            "PreToolUse on Read|Grep (Codex has no such tools)",
            "prompt hook in Stop (Codex runs command hooks only)",
        ]
    );
}

#[test]
fn ac7_05_no_hook_left_means_no_hooks_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = hooks_fixture(
        tmp.path(),
        &json!({"hooks": {"Notification": [group(None, "n")]}}),
    );
    let pkg = repackage::build(&src, "x").unwrap();
    let dest = tmp.path().join("out");
    repackage::write(&pkg, &dest).unwrap();
    assert!(!dest.join("hooks/hooks.json").exists());
    assert!(pkg.hooks.is_empty());
}

#[test]
fn ec7_06_a_broken_hooks_file_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    write(&src.join("hooks/hooks.json"), "{not json");
    let err = repackage::build(&src, "x").unwrap_err();
    assert!(
        err.contains("hooks.json") && err.contains("doesn't parse"),
        "{err}"
    );
    for shape in [
        json!({"hooks": []}),
        json!({"hooks": {"Stop": {}}}),
        json!({"hooks": {"Stop": [{"hooks": "x"}]}}),
        json!({"hooks": {"Stop": [{"matcher": 3, "hooks": []}]}}),
    ] {
        write(&src.join("hooks/hooks.json"), &shape.to_string());
        let err = repackage::build(&src, "x").unwrap_err();
        assert!(err.contains("unknown shape"), "{shape}: {err}");
    }
}

#[test]
fn ec7_18_hooks_in_the_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let src = qdrant(tmp.path());
    // Inline.
    write(
        &src.join(".claude-plugin/plugin.json"),
        &json!({"name": "q", "hooks": {"hooks": {"Stop": [group(None, "s")]}}}).to_string(),
    );
    let dest = tmp.path().join("inline");
    repackage::write(&repackage::build(&src, "x").unwrap(), &dest).unwrap();
    assert_eq!(
        package_hooks(&dest),
        json!({"hooks": {"Stop": [group(None, "s")]}})
    );
    assert!(manifest(&dest).get("hooks").is_none());
    // A path: read from there, written to hooks/hooks.json, the original not copied twice.
    write(
        &src.join("config/my-hooks.json"),
        &json!({"hooks": {"Stop": [group(None, "t")]}}).to_string(),
    );
    write(
        &src.join(".claude-plugin/plugin.json"),
        r#"{"name": "q", "hooks": "./config/my-hooks.json"}"#,
    );
    let dest = tmp.path().join("path");
    repackage::write(&repackage::build(&src, "x").unwrap(), &dest).unwrap();
    assert_eq!(
        package_hooks(&dest),
        json!({"hooks": {"Stop": [group(None, "t")]}})
    );
    assert!(!dest.join("config/my-hooks.json").exists());
    // A path outside is refused.
    write(
        &src.join(".claude-plugin/plugin.json"),
        r#"{"name": "q", "hooks": "../h.json"}"#,
    );
    assert!(
        repackage::build(&src, "x")
            .unwrap_err()
            .contains("outside the plugin")
    );
}
