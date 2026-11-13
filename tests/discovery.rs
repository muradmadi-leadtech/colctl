//! Cycle 5: discovery. Catalogs are fixture files in the temp profiles (marketplace manifests, a
//! plugin-directory cache, Codex marketplaces); nothing touches the network. Named after AC5-NN / EC5-NN.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::Env;

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.ok(&["init"]);
    env
}

/// A Claude marketplace in `profile`, stored under the temp root.
fn claude_market(env: &Env, profile: &Path, name: &str, repo: &str, plugins: Value) -> PathBuf {
    let dir = env.home.parent().unwrap().join("markets").join(name);
    write(
        &dir.join(".claude-plugin/marketplace.json"),
        &json!({"name": name, "plugins": plugins}),
    );
    let known = profile.join("plugins/known_marketplaces.json");
    let mut all: Value = fs::read_to_string(&known)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(json!({}));
    all[name] = json!({"source": {"source": "github", "repo": repo},
                       "installLocation": dir.display().to_string()});
    write(&known, &all);
    dir
}

fn directory(profile: &Path, listings: Vec<(&str, &str, &str, &str)>) {
    let mut names = json!({});
    let mut list = Vec::new();
    for (i, (slug, name, summary, repo)) in listings.into_iter().enumerate() {
        let id = format!("p{i}");
        names[&id] = json!(slug);
        list.push(json!({"id": id, "name": name, "summary": summary, "surfaces": ["claude_code"],
            "release": {"version": "1.0.0"},
            "source": {"type": "directory", "repository": {"url": repo, "reviewed_commit": "c0ffee1"}}}));
    }
    write(
        &profile.join("plugins/plugin-directory-cache-v2.json"),
        &json!({"schemaVersion": 2, "entryNames": names, "listings": list}),
    );
}

/// The standard catalogs: an official marketplace, the plugin directory, and a Codex marketplace.
fn catalogs(env: &Env) {
    let native = env.home.join(".claude");
    claude_market(
        env,
        &native,
        "official",
        "anthropics/official",
        json!([
            {"name": "42crunch-api-security-testing", "description": "Automate API security directly in Claude Code",
             "source": {"source": "git-subdir", "url": "https://github.com/42Crunch-AI/claude-plugins.git",
                        "path": "plugins/api-security-testing", "sha": "faf5305"},
             "keywords": ["openapi"]},
            {"name": "superpowers", "description": "Core skills library", "author": {"name": "Jesse Vincent"},
             "source": {"source": "url", "url": "https://github.com/obra/superpowers.git"}},
            {"name": "local-thing", "description": "Lives in the marketplace", "source": "./plugins/local-thing"}
        ]),
    );
    directory(
        &native,
        vec![
            (
                "superpowers",
                "Superpowers",
                "Skills: TDD, debugging",
                "https://github.com/obra/superpowers",
            ),
            (
                "security-guidance",
                "Security guidance",
                "Security reminders while editing an API",
                "https://github.com/anthropics/security",
            ),
            (
                "episodic-memory",
                "Episodic memory",
                "Remember past conversations",
                "https://github.com/obra/episodic-memory",
            ),
        ],
    );
    let market = env.home.parent().unwrap().join("codex-markets/tools");
    write(
        &market.join(".agents/plugins/marketplace.json"),
        &json!({"name": "tools", "plugins": [
            {"name": "api-linter", "description": "Lint API specs", "source": {"source": "url", "url": "./api-linter"}}
        ]}),
    );
    fs::write(
        env.home.join(".codex/config.toml"),
        format!(
            "[marketplaces.tools]\nsource_type = \"local\"\nsource = \"{}\"\n",
            market.display()
        ),
    )
    .unwrap();
    write(
        &native.join("plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {"security-guidance@anthropic-plugin-directory": [{"scope": "user"}]}}),
    );
}

fn json_of(env: &Env, args: &[&str]) -> (i32, Value) {
    let (code, out, _) = env.code(args);
    (code, serde_json::from_str(&out).unwrap_or(Value::Null))
}

// ---------------------------------------------------------------- search and show (T3)

#[test]
fn ac5_12_search_prints_results() {
    let env = machine();
    catalogs(&env);
    let out = env.ok(&["search", "api", "security"]);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines[0].starts_with("42crunch-api-security-testing@official"),
        "{out}"
    );
    assert!(lines[0].contains("42crunch-ai"), "{out}");
    assert!(
        lines[1].starts_with("security-guidance@anthropic-plugin-directory"),
        "{out}"
    );
    assert!(lines[1].ends_with("[installed: claude]"), "{out}");
    assert_eq!(lines.last().unwrap(), &"2 of 2 results");
    let out = env.ok(&["search"]);
    assert!(out.trim_end().ends_with("6 of 6 results"), "{out}");
    let out = env.ok(&["search", "--limit", "2"]);
    assert!(out.contains("2 of 6 results; --limit 0 shows all"), "{out}");
}

#[test]
fn ac5_13_search_json() {
    let env = machine();
    catalogs(&env);
    let (code, v) = json_of(&env, &["search", "superpowers", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(v["version"], 1);
    assert_eq!(v["matched"], 1);
    let r = &v["results"][0];
    assert_eq!(
        r["ids"],
        json!([
            "superpowers@official",
            "superpowers@anthropic-plugin-directory"
        ])
    );
    assert_eq!(r["developer"], "obra");
    assert_eq!(r["author"], "Jesse Vincent");
    assert_eq!(r["repo"], "https://github.com/obra/superpowers.git");
    assert_eq!(r["kind"], "plugin");
    assert_eq!(r["add"], "colctl add source superpowers@official");
    assert_eq!(r["catalogs"].as_array().unwrap().len(), 2);
    assert_eq!(v["warnings"], json!([]));
}

#[test]
fn ac5_14_show() {
    let env = machine();
    catalogs(&env);
    let out = env.ok(&["show", "42crunch-api-security-testing@official"]);
    for want in [
        "42crunch-api-security-testing",
        "  developer    42crunch-ai",
        "  repo         https://github.com/42Crunch-AI/claude-plugins.git",
        "  subpath      plugins/api-security-testing",
        "  version      commit faf5305",
        "  keywords     openapi",
        "  add          colctl add source 42crunch-api-security-testing@official",
    ] {
        assert!(out.contains(want), "{want:?} in:\n{out}");
    }
    // Any id of a merged result, or a unique bare name.
    let by_other_id = env.ok(&["show", "superpowers@anthropic-plugin-directory"]);
    assert!(by_other_id.starts_with("Superpowers"), "{by_other_id}");
    assert!(
        by_other_id.contains("version      1.0.0 (commit c0ffee1)"),
        "{by_other_id}"
    );
    env.ok(&["show", "episodic-memory"]);
    let (code, v) = json_of(&env, &["show", "local-thing", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(v["result"]["subpath"], "plugins/local-thing");
    assert_eq!(
        v["result"]["repo"],
        "https://github.com/anthropics/official"
    );
}

#[test]
fn ac5_15_search_is_fast() {
    let env = machine();
    let listings: Vec<(String, String, String, String)> = (0..4000)
        .map(|i| {
            (
                format!("plugin-{i}"),
                format!("Plugin {i}"),
                format!("Does thing number {i} with api and security"),
                format!("https://github.com/owner{}/repo{i}", i % 300),
            )
        })
        .collect();
    directory(
        &env.home.join(".claude"),
        listings
            .iter()
            .map(|(a, b, c, d)| (a.as_str(), b.as_str(), c.as_str(), d.as_str()))
            .collect(),
    );
    let started = std::time::Instant::now();
    let out = env.ok(&["search", "api", "security", "--limit", "5"]);
    let took = started.elapsed();
    assert!(out.contains("of 4000 results"), "{out}");
    assert!(took < std::time::Duration::from_secs(1), "took {took:?}");
}

#[test]
fn ac5_16_reading_writes_nothing_and_calls_nothing() {
    let env = machine();
    catalogs(&env);
    let before = env.snapshot();
    let calls = (env.log().len(), env.codex_log().len());
    for args in [
        &["search", "api"][..],
        &["show", "superpowers"],
        &["developers"],
        &["developers", "obra"],
    ] {
        env.code(args);
    }
    assert_eq!(env.snapshot(), before);
    assert_eq!((env.log().len(), env.codex_log().len()), calls);
    assert!(!env.source.join(".state/lock").exists());
}

// ---------------------------------------------------------------- developers (T4)

#[test]
fn ac5_17_listing_developers() {
    let env = machine();
    catalogs(&env);
    let out = env.ok(&["developers"]);
    let lines: Vec<&str> = out.lines().collect();
    // By plugin count, then name: anthropics and obra both have 2.
    assert!(lines[0].starts_with("anthropics"), "{out}");
    assert!(
        lines[1].starts_with("obra") && lines[1].contains("2 plugins"),
        "{out}"
    );
    assert!(
        lines[1].contains("anthropic-plugin-directory, official"),
        "{out}"
    );
    assert!(
        lines[2].starts_with("42crunch-ai") && lines[2].contains("1 plugin "),
        "{out}"
    );
    let (_, v) = json_of(&env, &["developers", "--json"]);
    assert_eq!(v["developers"][1]["name"], "obra");
    assert_eq!(v["developers"][1]["plugins"], 2);
}

#[test]
fn ac5_18_one_developer() {
    let env = machine();
    catalogs(&env);
    let one = env.ok(&["developers", "OBRA"]);
    let same = env.ok(&["search", "--developer", "obra", "--limit", "0"]);
    assert_eq!(one, same);
    assert!(
        one.contains("episodic-memory@anthropic-plugin-directory"),
        "{one}"
    );
    assert_eq!(env.code(&["developers", "nobody"]).0, 1);
}

// ---------------------------------------------------------------- edges

#[test]
fn ec5_01_no_catalogs() {
    let env = machine();
    for args in [&["search"][..], &["developers"]] {
        let (code, out, _) = env.code(args);
        assert_eq!(code, 1, "{args:?}");
        assert!(out.contains("no catalogs found"), "{out}");
    }
}

#[test]
fn ec5_10_nothing_matches() {
    let env = machine();
    catalogs(&env);
    let (code, out, _) = env.code(&["search", "zebra"]);
    assert_eq!(code, 1);
    assert!(out.contains("no plugins match zebra"), "{out}");
    let (code, v) = json_of(&env, &["search", "zebra", "--json"]);
    assert_eq!((code, v["matched"].clone()), (1, json!(0)));
}

#[test]
fn ec5_11_show_unknown_or_ambiguous() {
    let env = machine();
    catalogs(&env);
    let market = env.home.parent().unwrap().join("markets/other");
    claude_market(
        &env,
        &env.home.join(".claude"),
        "other",
        "o/other",
        json!([{"name": "episodic-memory", "source": {"source": "github", "repo": "someone/else"}}]),
    );
    let _ = market;
    let (code, _, err) = env.code(&["show", "nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("no plugin nope"), "{err}");
    let (code, _, err) = env.code(&["show", "episodic-memory"]);
    assert_eq!(code, 2);
    assert!(err.contains("episodic-memory is ambiguous:"), "{err}");
    assert!(err.contains("episodic-memory@other"), "{err}");
}

#[test]
fn ec5_12_limit_zero_and_a_bad_tool() {
    let env = machine();
    catalogs(&env);
    let (_, v) = json_of(&env, &["search", "--limit", "0", "--json"]);
    assert_eq!(v["results"].as_array().unwrap().len(), 6);
    assert_eq!(env.code(&["search", "--tool", "cursor"]).0, 2);
    let (_, v) = json_of(&env, &["search", "--tool", "codex", "--json"]);
    assert_eq!(v["results"][0]["ids"], json!(["api-linter@tools"]));
}

#[test]
fn ec5_13_control_characters_are_escaped() {
    let env = machine();
    claude_market(
        &env,
        &env.home.join(".claude"),
        "evil",
        "o/evil",
        json!([{"name": "trap", "description": "ok \u{1b}]0;pwned\u{07} here", "source": "./trap"}]),
    );
    let out = env.ok(&["search", "trap"]);
    assert!(
        !out.contains('\u{1b}') && !out.contains('\u{07}'),
        "{out:?}"
    );
    assert!(out.contains("\\x1b]0;pwned\\x07"), "{out}");
    let (_, v) = json_of(&env, &["search", "trap", "--json"]);
    assert_eq!(
        v["results"][0]["description"],
        "ok \u{1b}]0;pwned\u{07} here"
    );
}
