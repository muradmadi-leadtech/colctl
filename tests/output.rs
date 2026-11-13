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
        let boundary = i == 0 || !chars[i - 1].is_ascii_hexdigit();
        if boundary && (run == 40 || run == 7) {
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
const STILL_PRINTING: &[&str] = &[];

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

// ---------------------------------------------------------------- status (T2)

/// Five profiles with ~100 items, all in sync, like the owner's machine.
fn calm_machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.kilo_codex();
    env.agents_dir();
    env.ok(&["init"]);
    for i in 0..32 {
        env.source_skill(&format!("skill-{i:02}"));
    }
    env.ok(&["sync"]);
    env
}

fn non_blank(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

#[test]
fn ac6_08_all_in_sync_fits_in_eight_lines() {
    let env = calm_machine();
    let (code, out, _) = env.code(&["status"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.starts_with("✓ All 5 profiles in sync · 0 sources · nothing held for review"),
        "{out}"
    );
    assert!(non_blank(&out) <= 8, "{out}");
    assert!(
        out.trim_end()
            .ends_with("→ colctl status --all lists every item"),
        "{out}"
    );
    let words = Env::words(&out);
    assert!(words.contains("✓ claude ~/.claude 32 skills"), "{out}");
    assert!(words.contains("✓ agents ~/.agents 32 skills"), "{out}");
    let all = env.ok(&["status", "--all"]);
    assert!(non_blank(&all) > 90, "{all}");
}

#[test]
fn ac6_09_problems_only_each_with_its_fix() {
    let env = calm_machine();
    fs::remove_file(env.home.join(".claude/skills/skill-03")).unwrap();
    fs::write(env.source.join("skills/skill-04/SKILL.md"), "edited").unwrap();
    let (code, out, _) = env.code(&["status"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("! 2 things need attention"), "{out}");
    let words = Env::words(&out);
    assert!(words.contains("Needs attention"), "{out}");
    assert!(
        words.contains("! claude skill skill-03 missing → colctl sync"),
        "{out}"
    );
    assert!(
        words.contains("! ~/.colctl file skills/skill-04/SKILL.md uncommitted → colctl sync"),
        "{out}"
    );
    assert!(
        words.contains("! claude ~/.claude 32 skills · 1 missing"),
        "{out}"
    );
    // Nothing that's fine is listed one by one.
    assert!(!words.contains("skill-05"), "{out}");
}

#[test]
fn ac6_10_all_lists_every_item() {
    let env = calm_machine();
    let out = env.ok(&["status", "--all"]);
    let words = Env::words(&out);
    for p in ["claude", "kilo-claude", "codex", "kilo-codex", "agents"] {
        assert!(
            out.lines().any(|l| l.starts_with(p)),
            "{p} heading in {out}"
        );
    }
    assert!(words.contains("✓ ok skill skill-31"), "{out}");
    assert!(!out.contains("status --all lists every item"), "{out}");
}

#[test]
fn ac6_11_an_unusable_profile_says_so_with_its_fix() {
    let env = calm_machine();
    fs::remove_dir_all(env.home.join(".codex")).unwrap();
    let (code, out, _) = env.code(&["status"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("! 1 thing needs attention"), "{out}");
    assert!(
        Env::words(&out).contains("✗ codex ~/.codex harness-missing → start that profile once"),
        "{out}"
    );
}

#[test]
fn ec6_10_ec6_11_empty_profiles_and_none_at_all() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let out = env.ok(&["status", "--all"]);
    assert!(Env::words(&out).contains("· 0 items"), "{out}");
    let none = Env::new();
    none.ok(&["init"]);
    let out = none.ok(&["status"]);
    assert!(out.starts_with("! No profiles registered"), "{out}");
    assert!(out.contains("→ colctl init"), "{out}");
}

#[test]
fn ec6_07_long_paths_are_cut_in_the_middle() {
    let env = calm_machine();
    let out = env.ok(&["status"]);
    assert!(out.contains("~/Library/…/profiles/claude"), "{out}");
    for l in out.lines() {
        assert!(console::measure_text_width(l) <= 80, "{l}");
    }
}

// ---------------------------------------------------------------- running commands (T3)

fn two_profiles_with_plugins() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.ok(&["init"]);
    env.source_skill("alpha");
    env.source_skill("beta");
    let config = env.config().replace(
        "enabled = []",
        "enabled = [\"tool@anthropic-plugin-directory\"]",
    );
    env.write_config(&config);
    env
}

#[test]
fn ac6_12_outcomes_not_calls() {
    let env = two_profiles_with_plugins();
    let out = env.ok(&["sync"]);
    let words = Env::words(&out);
    assert!(out.starts_with("✓ Synced 2 profiles · 6 changes"), "{out}");
    assert!(
        words.contains("✓ claude linked skills alpha, beta · installed tool"),
        "{out}"
    );
    assert!(!out.contains("claude plugin install"), "{out}");
    let again = env.ok(&["sync"]);
    assert!(
        again.starts_with("✓ Everything was already in sync"),
        "{again}"
    );
}

#[test]
fn ac6_13_informational_outcomes_do_not_contradict() {
    let env = Env::new();
    env.native();
    env.codex();
    env.ok(&["init"]);
    let (_, url, _) = env.upstream("clonly", true, false, Some("m"));
    let out = env.ok(&["add", "source", &url]);
    assert!(out.starts_with("✓ Added source clonly"), "{out}");
    let words = Env::words(&out);
    assert!(
        words.contains("· codex · clonly: the repo has no Codex packaging"),
        "{out}"
    );
    assert!(!out.to_lowercase().contains("needs attention"), "{out}");
    assert!(!out.contains("not in sync"), "{out}");
}

#[test]
fn ac6_14_failures_say_what_failed_and_what_to_do() {
    let env = two_profiles_with_plugins().var("FAKE_CLAUDE_FAIL", "install tool");
    let (code, out, _) = env.code(&["sync"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.starts_with("✗ Synced 0 of 2 profiles · 2 failed"),
        "{out}"
    );
    let words = Env::words(&out);
    assert!(
        words.contains(
            "✗ install tool failed: claude plugin install tool@anthropic-plugin-directory exited 1"
        ),
        "{out}"
    );
    assert!(
        words.contains("→ colctl sync retries · colctl sync -v shows each call"),
        "{out}"
    );
}

#[test]
fn ac6_15_no_control_characters_when_piped() {
    let env = two_profiles_with_plugins();
    let (_, out, err) = env.code(&["sync"]);
    for text in [&out, &err] {
        assert!(!text.contains('\u{1b}') && !text.contains('\r'), "{text:?}");
    }
}

#[test]
fn ac6_16_verbose_and_quiet() {
    let env = two_profiles_with_plugins();
    let out = env.ok(&["sync", "-v"]);
    assert!(
        Env::words(&out).contains("✓ claude plugin install tool@anthropic-plugin-directory"),
        "{out}"
    );
    assert!(
        out.lines()
            .any(|l| l.trim_end().ends_with('s') && l.contains("s") && l.contains("0.")),
        "{out}"
    );
    env.source_skill("gamma");
    let quiet = env.ok(&["sync", "-q"]);
    assert_eq!(quiet.lines().count(), 1, "{quiet}");
    assert!(
        quiet.starts_with("✓ Synced 2 profiles · 2 changes"),
        "{quiet}"
    );
    let status = env.ok(&["status", "-q"]);
    assert_eq!(status.lines().count(), 1, "{status}");
}

#[test]
fn ac6_17_commands_lead_with_their_result() {
    let env = two_profiles_with_plugins();
    env.ok(&["sync"]);
    let skill = env.skill_dir(&env.home, "delta", "x");
    let out = env.ok(&["add", "skill", skill.to_str().unwrap()]);
    assert!(out.starts_with("✓ Added skill delta"), "{out}");
    assert!(
        Env::words(&out).contains("✓ claude linked skill delta"),
        "{out}"
    );
    let out = env.ok(&["rm", "skill", "delta"]);
    assert!(out.starts_with("✓ Removed skill delta"), "{out}");
    let dry = env.ok(&["rm", "skill", "alpha", "--dry-run"]);
    assert!(
        dry.starts_with("· Would remove skill alpha, commit and sync"),
        "{dry}"
    );
    let plan = env.ok(&["sync", "--dry-run"]);
    assert!(plan.starts_with("✓ Nothing to change"), "{plan}");
    env.skill_dir(&env.home.join(".claude/skills"), "mine", "x");
    let out = env.ok(&["adopt", "claude", "skills/mine"]);
    assert!(out.starts_with("✓ Adopted 1 item"), "{out}");
    assert!(out.contains("✓ adopted skills/mine from claude"), "{out}");
}

// ---------------------------------------------------------------- interaction (T4)

#[test]
fn ac6_18_ec6_12_without_a_terminal_nothing_asks() {
    let env = two_profiles_with_plugins();
    env.ok(&["sync"]);
    // rm proceeds as in 2.1: no question without a terminal.
    let out = env.ok(&["rm", "skill", "alpha"]);
    assert!(out.starts_with("✓ Removed skill alpha"), "{out}");
    // --yes is accepted and changes nothing about that.
    let out = env.ok(&["rm", "skill", "beta", "--yes"]);
    assert!(out.starts_with("✓ Removed skill beta"), "{out}");
}

#[test]
fn ac6_19_ec6_05_without_a_terminal_a_missing_name_is_still_an_error() {
    let env = fixture();
    let (code, _, err) = env.code(&["approve"]);
    assert_eq!(code, 2);
    assert!(err.contains("or use --all"), "{err}");
    let (code, _, err) = env.code(&["reject"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = env.code(&["show"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("hint: colctl search lists plugin ids"),
        "{err}"
    );
    // review -i without a terminal lists, as review does.
    let (code, out, _) = env.code(&["review", "-i"]);
    assert_eq!(code, 1);
    assert!(out.contains("sources/superpowers"), "{out}");
}

#[test]
fn ac6_20_without_a_terminal_long_output_is_not_paged() {
    let env = fixture();
    let env = env.var("PAGER", "false");
    // A pager that fails would lose the text; without a terminal it's never started.
    let out = env.ok(&["review", "superpowers"]);
    assert!(out.contains("superpowers"), "{out}");
    assert!(out.contains("diff"), "{out}");
}

// ---------------------------------------------------------------- the rest (T5)

#[test]
fn ac6_21_update_and_refresh_lead_with_a_verdict() {
    let env = fixture();
    let (code, out, _) = env.code(&["update", "--check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("! 1 update available"), "{out}");
    assert!(out.contains("(held for review)"), "{out}");
    let out = env.ok(&["update"]);
    assert!(out.starts_with("! 1 update held for review"), "{out}");
    assert!(out.contains("! superpowers:"), "{out}");
    assert!(
        out.trim_end().ends_with("→ colctl review superpowers"),
        "{out}"
    );
    let out = env.ok(&["refresh"]);
    assert!(out.starts_with("· Nothing to refresh"), "{out}");
}

#[test]
fn ac6_22_review_puts_risk_first_and_colours_the_diff() {
    let env = fixture();
    let list = env.expect(1, &["review"]);
    assert!(list.starts_with("! 1 version held for review"), "{list}");
    assert!(
        list.trim_end().ends_with("→ colctl review superpowers"),
        "{list}"
    );
    let out = env.ok(&["review", "superpowers", "--color", "always"]);
    assert!(out.contains("Risk flags"), "{out}");
    assert!(
        out.contains("\u{1b}[32m+v2"),
        "added lines are green: {out:?}"
    );
    assert!(
        out.contains("\u{1b}[31m-v1"),
        "removed lines are red: {out:?}"
    );
    let plain = env.ok(&["review", "superpowers"]);
    assert!(!plain.contains('\u{1b}'), "{plain:?}");
    assert!(plain.find("Risk flags").unwrap() < plain.find("Diff").unwrap());
}

#[test]
fn ac6_23_discovery_output() {
    let env = fixture();
    let out = env.ok(&["search"]);
    assert!(out.contains("colctl show <id> for details"), "{out}");
    let show = env.ok(&["show", "tool@official"]);
    let lines: Vec<&str> = show.lines().collect();
    assert_eq!(lines[0], "tool");
    assert!(Env::words(&show).contains("developer ann"), "{show}");
    let dev = env.ok(&["developers"]);
    assert!(dev.contains("colctl developers <name>"), "{dev}");
    let markets = env.var("COLUMNS", "120").ok(&["marketplace", "list"]);
    assert!(markets.starts_with("NAME"), "{markets}");
    assert!(
        Env::words(&markets).contains("NAME TOOL SOURCE PLUGINS PROFILES"),
        "{markets}"
    );
}

#[test]
fn ac6_24_every_error_has_a_hint() {
    let env = fixture();
    for args in [
        &["review", "nope"][..],
        &["show", "nope@nowhere"],
        &["rm", "skill", "nope"],
        &["add", "source", "not a repo"],
        &["update", "--rev", "x"],
        &["status", "--harness", "nope"],
        &["approve"],
        &["nonsense"],
    ] {
        let (code, _, err) = env.code(args);
        assert_ne!(code, 0, "{args:?}");
        let last = err.lines().last().unwrap_or_default();
        assert!(last.starts_with("  hint: "), "{args:?}: {err}");
        assert!(err.starts_with("✗ error: "), "{args:?}: {err}");
    }
}

// ---------------------------------------------------------------- help (T6)

#[test]
fn ac6_25_ac6_26_help_is_grouped_with_examples() {
    let env = Env::new();
    let out = env.ok(&["--help"]);
    assert!(out.contains("Updates and review:"), "{out}");
    assert!(
        out.contains("Fetch new versions of source plugins and hold them for review"),
        "{out}"
    );
    let out = env.ok(&["update", "--help"]);
    assert!(out.contains("Examples:\n  colctl update --check"), "{out}");
}

#[test]
fn ac6_27_completions() {
    let env = Env::new();
    for shell in ["bash", "zsh", "fish"] {
        let out = env.ok(&["completions", shell]);
        for sub in ["status", "approve", "search", "completions"] {
            assert!(out.contains(sub), "{shell}: {sub}");
        }
    }
    let (code, _, err) = env.code(&["completions", "tcsh"]);
    assert_eq!(code, 2);
    assert!(err.starts_with("✗ error: "), "{err}");
}

// ---------------------------------------------------------------- snapshots (T7)

/// Runs colctl with `COLUMNS` set, as a terminal of that width would.
fn at_width(env: &Env, args: &[&str], columns: &str) -> (i32, String, String) {
    let out = env
        .cmd()
        .env("COLUMNS", columns)
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Every command's text output, as a person sees it (AC6-28): 60, 80 and 120 columns without colour,
/// and 80 with colour. Run with INSTA_UPDATE=always to record after an intended change.
#[test]
fn ac6_28_snapshots_of_every_command() {
    let env = fixture();
    let views: &[(&str, &[&str])] = &[
        ("status", &["status"]),
        ("status_all", &["status", "--all"]),
        ("sync_dry_run", &["sync", "--dry-run"]),
        ("update_check", &["update", "--check"]),
        ("review_list", &["review"]),
        ("review_one", &["review", "superpowers"]),
        ("search", &["search"]),
        ("show", &["show", "tool@official"]),
        ("developers", &["developers"]),
        ("marketplaces", &["marketplace", "list"]),
        ("rm_dry_run", &["rm", "skill", "alpha", "--dry-run"]),
        ("error", &["review", "nope"]),
    ];
    for (name, args) in views {
        for (suffix, columns, colour) in [
            ("60", "60", "never"),
            ("80", "80", "never"),
            ("120", "120", "never"),
            ("80_colour", "80", "always"),
        ] {
            let mut all: Vec<&str> = args.to_vec();
            all.extend(["--color", colour]);
            let (code, out, err) = at_width(&env, &all, columns);
            let text = format!("exit {code}\n--- stdout\n{out}--- stderr\n{err}");
            insta::assert_snapshot!(format!("text_{name}_{suffix}"), normalise(&env, &text));
            if colour == "never" {
                for line in out.lines() {
                    let w = console::measure_text_width(line);
                    let max: usize = columns.parse().unwrap();
                    // Diffs and file paths in review are content, not layout.
                    if *name != "review_one" {
                        assert!(w <= max, "{name} at {columns}: {line:?} is {w}");
                    }
                }
            }
        }
    }
    // A sync that changes things, in a fresh machine.
    let fresh = two_profiles_with_plugins();
    let (_, out, _) = fresh.code(&["sync", "--color", "never"]);
    insta::assert_snapshot!("text_sync_80", normalise(&fresh, &out));
}

#[test]
fn stacked_outcomes_start_on_the_profile_line() {
    let env = two_profiles_with_plugins();
    for i in 0..6 {
        env.source_skill(&format!("long-skill-name-number-{i}"));
    }
    let (_, out, _) = at_width(&env, &["sync"], "40");
    let line = out.lines().find(|l| l.contains("claude ")).unwrap();
    assert!(line.trim_end().len() > "  ✓  claude".len(), "{out}");
}

// ---------------------------------------------------------------- revise (T9, T10)

/// The text after a profile's path on its status line.
fn profile_line(out: &str, name: &str) -> String {
    out.lines()
        .map(Env::words)
        .find(|l| l.split(' ').nth(1) == Some(name))
        .unwrap_or_else(|| panic!("no {name} line in {out}"))
}

#[test]
fn ac6_34_profile_lines_say_what_each_profile_has() {
    let env = calm_machine();
    fs::write(env.source.join("AGENTS.md"), "rules").unwrap();
    let (_, url, v1) = env.upstream("clonly", true, false, Some("m"));
    env.add_source_entry("clonly", &url, &v1, "");
    env.ok(&["sync"]);
    let out = env.ok(&["status", "--color", "never"]);
    let claude = profile_line(&out, "claude");
    assert!(
        claude.ends_with("~/.claude 32 skills · 1 plugin · CLAUDE.md"),
        "{claude}"
    );
    let codex = profile_line(&out, "codex");
    assert!(
        codex.ends_with("~/.codex AGENTS.md · skills via ~/.agents · 1 not packaged"),
        "{codex}"
    );
    let agents = profile_line(&out, "agents");
    assert!(agents.ends_with("~/.agents 32 skills"), "{agents}");
}

#[test]
fn ac6_34_an_empty_profile_says_nothing_yet() {
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let out = env.ok(&["status"]);
    assert!(
        profile_line(&out, "claude").ends_with("~/.claude nothing yet"),
        "{out}"
    );
}

/// The path cell of each profile line: the word after the name.
fn paths(out: &str) -> Vec<String> {
    out.lines()
        .map(Env::words)
        .filter(|l| l.starts_with('✓') || l.starts_with('!') || l.starts_with('✗'))
        .filter_map(|l| l.split(' ').nth(2).map(str::to_string))
        .filter(|p| p.starts_with('~'))
        .collect()
}

#[test]
fn ac6_35_paths_are_shortened_alike_and_only_when_needed() {
    let env = calm_machine();
    let (_, wide, _) = at_width(&env, &["status", "--color", "never"], "160");
    let full = paths(&wide);
    assert!(
        full.iter().any(|p| p.contains("Application")),
        "full paths at 160: {wide}"
    );
    for columns in ["60", "80"] {
        let (_, out, _) = at_width(&env, &["status", "--color", "never"], columns);
        let kilo: Vec<String> = paths(&out)
            .into_iter()
            .filter(|p| p.contains('…'))
            .collect();
        assert_eq!(kilo.len(), 2, "both kilo paths shortened at {columns}: {out}");
        let shape = |p: &str| {
            let (head, tail) = p.split_once('…').unwrap();
            (head.to_string(), tail.matches('/').count())
        };
        assert_eq!(shape(&kilo[0]), shape(&kilo[1]), "{out}");
        for line in out.lines() {
            assert!(
                console::measure_text_width(line) <= columns.parse().unwrap(),
                "{line:?} at {columns}"
            );
        }
    }
}

#[test]
fn ac6_36_help_welcomes_someone_new() {
    let env = Env::new();
    for args in [&["--help"][..], &["help"], &["-h"], &[]] {
        let mut all = args.to_vec();
        all.extend(["--color", "never"]);
        let (code, out, err) = at_width(&env, &all, "80");
        assert_eq!(code, 0, "{args:?}: {out}{err}");
        let first = out.lines().next().unwrap();
        assert!(first.starts_with("colctl keeps every AI coding profile"), "{out}");
        let words = Env::words(&out);
        for want in [
            "Start here",
            "colctl init Find your profiles and create ~/.colctl",
            "colctl status See what's in sync and what needs you",
            "colctl sync Make every profile match",
            "More: colctl <command> --help",
        ] {
            assert!(words.contains(want), "{want} in {out}");
        }
        // Groups are separated by a blank line.
        let lines: Vec<&str> = out.lines().collect();
        for heading in ["Start here", "Options"] {
            let at = lines.iter().position(|l| *l == heading).unwrap();
            assert_eq!(lines[at - 1], "", "{heading} in {out}");
        }
        for line in out.lines() {
            assert!(console::measure_text_width(line) <= 80, "{line:?}");
        }
    }
    // Styled in colour: headings bold, commands cyan.
    let (_, out, _) = at_width(&env, &["--help", "--color", "always"], "80");
    assert!(out.contains("\u{1b}[1mStart here"), "{out:?}");
    assert!(out.contains("\u{1b}[36mcolctl init"), "{out:?}");
    // A command's help uses the same styles and fits the width.
    let (_, out, _) = at_width(&env, &["status", "--help", "--color", "always"], "80");
    assert!(out.contains("\u{1b}[1m"), "{out:?}");
    let (_, out, _) = at_width(&env, &["status", "--help", "--color", "never"], "80");
    for line in out.lines() {
        assert!(console::measure_text_width(line) <= 80, "{line:?}");
    }
}

#[test]
fn ec6_23_bare_colctl_before_init_creates_nothing() {
    let env = Env::new();
    let (code, out, _) = env.code(&[]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Start here"), "{out}");
    assert!(!env.source.exists());
}
