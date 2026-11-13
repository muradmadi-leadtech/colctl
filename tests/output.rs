//! Cycle 6: the output layer. JSON goldens recorded from 2.1 (AC6-01), the one-output-layer guard
//! (AC6-02), errors, global flags. Named after AC6-NN / EC6-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::Env;

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}

/// A machine with a little of everything: a Claude marketplace and directory listing, a source plugin
/// with an update held for review, a taken-over skill, and a skill that's missing in one profile.
fn fixture() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.ok(&["init"]);
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    env.upstream_commit(&repo, "v2");
    env.ok(&["update"]);
    let native = env.home.join(".claude");
    let market = env.home.parent().unwrap().join("markets/official");
    write(
        &market.join(".claude-plugin/marketplace.json"),
        &json!({"name": "official", "plugins": [
            {"name": "tool", "description": "A tool", "author": {"name": "Ann"},
             "source": {"source": "github", "repo": "ann/tool"}, "keywords": ["x"]}]}),
    );
    let mut known: Value = serde_json::from_str(
        &fs::read_to_string(native.join("plugins/known_marketplaces.json")).unwrap(),
    )
    .unwrap();
    known["official"] = json!({"source": {"source": "github", "repo": "o/official"},
                               "installLocation": market.display().to_string()});
    write(&native.join("plugins/known_marketplaces.json"), &known);
    env.source_skill("alpha");
    env.ok(&["sync"]);
    fs::remove_file(native.join("skills/alpha")).unwrap();
    env
}

/// Masks what differs between runs: the temp root, commit hashes, timestamps.
fn normalise(env: &Env, text: &str) -> String {
    let root = env.home.parent().unwrap().display().to_string();
    let mut out = text.replace(&root, "[ROOT]");
    // 40- and 7-hex commit ids.
    let mut masked = String::with_capacity(out.len());
    let chars: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let run = chars[i..]
            .iter()
            .take_while(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            .count();
        let boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if boundary && (run == 40 || run == 7) && chars[i..i + run].iter().any(char::is_ascii_digit)
        {
            masked.push_str(if run == 40 { "[SHA]" } else { "[SHORT]" });
            i += run;
        } else {
            masked.push(chars[i]);
            i += 1;
        }
    }
    out = masked;
    // held_at: Unix seconds.
    let mut lines = Vec::new();
    for line in out.lines() {
        if line.trim_start().starts_with("\"held_at\"") {
            lines.push("      \"held_at\": [TIME],".to_string());
        } else {
            lines.push(line.to_string());
        }
    }
    lines.join("\n")
}

#[test]
fn ac6_01_json_output_is_unchanged() {
    let env = fixture();
    for (name, args) in [
        ("status", &["status", "--json"][..]),
        ("review", &["review", "--json"]),
        ("review_one", &["review", "superpowers", "--json"]),
        ("search", &["search", "--json"]),
        ("show", &["show", "tool@official", "--json"]),
        ("developers", &["developers", "--json"]),
        ("marketplaces", &["marketplace", "list", "--json"]),
    ] {
        let (_, out, _) = env.code(args);
        insta::assert_snapshot!(format!("json_{name}"), normalise(&env, &out));
    }
}

/// Files under src/ that print directly (outside `#[cfg(test)]`). AC6-02: this list only shrinks,
/// and is empty once every command goes through `ui` (T6).
const STILL_PRINTING: &[&str] = &[
    "src/cli.rs",
    "src/commands/discovery.rs",
    "src/commands/mod.rs",
    "src/commands/quarantine.rs",
    "src/state.rs",
];

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn ac6_02_one_output_layer() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rs_files(&root.join("src"), &mut files);
    let mut printing = Vec::new();
    for f in files {
        let rel = f.strip_prefix(&root).unwrap().display().to_string();
        if rel.starts_with("src/ui/") || rel == "src/main.rs" {
            continue;
        }
        let text = fs::read_to_string(&f).unwrap();
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        if ["println!", "print!(", "eprintln!", "eprint!("]
            .iter()
            .any(|m| code.contains(m))
        {
            printing.push(rel);
        }
    }
    printing.sort();
    for p in &printing {
        assert!(
            STILL_PRINTING.contains(&p.as_str()),
            "{p} prints directly; use ctx.out"
        );
    }
}

#[test]
fn ac6_06_errors_say_what_and_what_to_do() {
    let env = fixture();
    let (code, _, err) = env.code(&["review", "nope"]);
    assert_eq!(code, 2);
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines[0], "✗ error: no source named \"nope\"", "{err}");
    assert!(lines.last().unwrap().starts_with("  hint: "), "{err}");
    assert!(err.contains("hint: colctl review --help"), "{err}");
    // The lock error keeps exit 3 and gets its own hint.
    fs::write(
        env.source.join(".state/lock"),
        std::process::id().to_string(),
    )
    .unwrap();
    let (code, _, err) = env.code(&["sync"]);
    assert_eq!(code, 3);
    assert!(
        err.starts_with("✗ error: another colctl is running"),
        "{err}"
    );
    assert!(
        err.contains("hint: wait for the other colctl to finish"),
        "{err}"
    );
    fs::remove_file(env.source.join(".state/lock")).unwrap();
    // Argument errors use the same layout.
    let (code, _, err) = env.code(&["status", "--harness"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("✗ error: "), "{err}");
    assert!(err.contains("hint: colctl status --help"), "{err}");
}

#[test]
fn ac6_07_global_flags_work_anywhere() {
    let env = fixture();
    for args in [
        &["--color", "never", "status"][..],
        &["status", "--color", "never"],
        &["status", "-v"],
        &["status", "-q"],
        &["status", "--yes", "--no-pager"],
    ] {
        let (code, _, err) = env.code(args);
        assert_ne!(code, 2, "{args:?}: {err}");
    }
}

#[test]
fn ec6_17_quiet_and_verbose_together() {
    let env = fixture();
    let (code, _, err) = env.code(&["status", "-q", "-v"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("✗ error: "), "{err}");
}

#[test]
fn ec6_18_a_typo_gets_a_did_you_mean() {
    let env = fixture();
    let (code, _, err) = env.code(&["statuz"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("✗ error: unrecognized subcommand"), "{err}");
    assert!(err.contains("hint: did you mean status"), "{err}");
    let (code, _, err) = env.code(&["zzzz"]);
    assert_eq!(code, 2);
    assert!(err.contains("hint: colctl --help"), "{err}");
    // Help and version still print normally.
    let (code, out, _) = env.code(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("Usage: colctl"), "{out}");
}

#[test]
fn ec6_20_a_closed_pipe_still_ends_quietly() {
    let env = fixture();
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("colctl"))
        .args(["--help"])
        .env_clear()
        .env("HOME", &env.home)
        .env("PATH", &env.path)
        .current_dir(&env.home)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("panicked"), "{err}");
    assert!(matches!(out.status.code(), Some(0 | 141)), "{err}");
}
