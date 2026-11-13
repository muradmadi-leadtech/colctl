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

/// [`machine`] plus the kilo Codex profile, registered by init.
fn machine_with_kilo_codex() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.kilo_codex();
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

// ---------------------------------------------------------------- marketplaces (T5)

#[test]
fn ac5_19_marketplace_list() {
    let env = machine();
    catalogs(&env);
    env.write_config(&env.config().replacen(
        "[marketplaces]",
        "[marketplaces]\nofficial = \"github:anthropics/official\"",
        1,
    ));
    let out = env.ok(&["marketplace", "list"]);
    let line = |name: &str| {
        out.lines()
            .find(|l| l.starts_with(name))
            .unwrap_or_else(|| panic!("{name} in {out}"))
            .to_string()
    };
    let dir = line("anthropic-plugin-directory");
    assert!(
        dir.contains("built in") && dir.contains("3 plugins") && dir.contains("claude"),
        "{dir}"
    );
    let official = line("official");
    assert!(
        official.contains("github:anthropics/official"),
        "{official}"
    );
    assert!(
        official.contains("3 plugins") && official.ends_with("managed"),
        "{official}"
    );
    let tools = line("tools");
    assert!(
        tools.contains("codex") && tools.contains("1 plugin "),
        "{tools}"
    );
    let (code, v) = json_of(&env, &["marketplace", "list", "--json"]);
    assert_eq!(code, 0);
    let ms = v["marketplaces"].as_array().unwrap();
    assert_eq!(ms.len(), 3);
    let o = ms.iter().find(|m| m["name"] == "official").unwrap();
    assert_eq!(o["managed"], true);
    assert_eq!(o["profiles"], json!(["claude"]));
}

#[test]
fn ac5_20_marketplace_refresh() {
    let env = machine_with_kilo_codex();
    let out = env.ok(&["marketplace", "refresh", "--dry-run"]);
    assert!(
        out.contains("claude: would run claude plugin marketplace update"),
        "{out}"
    );
    assert!(
        out.contains("kilo-codex: would run codex plugin marketplace upgrade"),
        "{out}"
    );
    assert!(env.log().is_empty() && env.codex_log().is_empty());
    let out = env.ok(&["marketplace", "refresh"]);
    assert!(
        out.contains("kilo-claude: ✓ claude plugin marketplace update"),
        "{out}"
    );
    let claude = env.log();
    assert_eq!(claude.len(), 2, "native and kilo");
    assert!(
        claude.iter().any(|c| c["config_dir"]
            .as_str()
            .is_some_and(|d| d.ends_with("profiles/claude"))),
        "{claude:?}"
    );
    assert_eq!(env.codex_log_args(), vec!["plugin marketplace upgrade"; 2]);
    let env = env.var("FAKE_CODEX_FAIL", "upgrade");
    let (code, out, _) = env.code(&["marketplace", "refresh"]);
    assert_eq!(code, 1);
    assert!(
        out.contains("codex: ✗ codex plugin marketplace upgrade"),
        "{out}"
    );
}

#[test]
fn ec5_17_refresh_without_a_tool() {
    let env = Env::without_claude();
    env.native();
    env.codex();
    env.ok(&["init"]);
    let (code, out, _) = env.code(&["marketplace", "refresh"]);
    assert_eq!(code, 1);
    assert!(
        out.contains("claude: skipped: claude isn't on PATH"),
        "{out}"
    );
    assert!(out.contains("codex: ✓"), "{out}");
}

// ---------------------------------------------------------------- connectors (T6)

fn codex_list(profile: &Path, available: Value, installed: Value) {
    write(
        &profile.join("fake-plugin-list.json"),
        &json!({"installed": installed, "available": available}),
    );
}

fn connector(id: &str, name: &str) -> Value {
    json!({"pluginId": id, "name": name, "marketplaceName": "openai-curated-remote",
           "version": "0.1.0", "source": {"source": "remote", "id": "x"}})
}

#[test]
fn ac5_21_connectors() {
    let env = machine_with_kilo_codex();
    let codex = env.home.join(".codex");
    let kilo = env.kilo_codex();
    let gmail = connector("gmail@openai-curated-remote", "gmail");
    let slack = connector("slack@openai-curated-remote", "slack");
    let local =
        json!({"pluginId": "p@local", "name": "p", "source": {"source": "git", "url": "/x"}});
    codex_list(&codex, json!([gmail, slack, local]), json!([]));
    codex_list(&kilo, json!([gmail]), json!([gmail]));
    let out = env.ok(&["connectors", "gmail"]);
    assert!(out.starts_with("gmail@openai-curated-remote"), "{out}");
    assert!(out.contains("[installed: kilo-codex]"), "{out}");
    assert!(out.contains("1 of 1 connectors"), "{out}");
    assert_eq!(
        env.codex_log_args(),
        vec!["plugin list --available --json"; 2]
    );
    let out = env.ok(&["connectors"]);
    assert!(out.contains("2 of 2 connectors"), "{out}");
    assert!(!out.contains("p@local"), "{out}");
    let (code, v) = json_of(&env, &["connectors", "slack", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(v["results"][0]["kind"], "connector");
    assert_eq!(
        v["results"][0]["add"],
        "installed by codex: codex plugin add slack@openai-curated-remote"
    );
}

#[test]
fn ac5_22_connectors_stay_out_of_search() {
    let env = machine();
    catalogs(&env);
    codex_list(
        &env.home.join(".codex"),
        json!([connector("gmail@openai-curated-remote", "gmail")]),
        json!([]),
    );
    assert_eq!(env.code(&["search", "gmail"]).0, 1);
    assert!(env.codex_log().is_empty());
}

#[test]
fn ec5_15_connectors_need_codex_and_a_profile() {
    let env = Env::without_codex();
    env.native();
    env.codex();
    env.ok(&["init"]);
    let (code, _, err) = env.code(&["connectors"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("connectors come from codex, and it isn't on PATH"),
        "{err}"
    );
    let env = Env::new();
    env.native();
    env.ok(&["init"]);
    let (code, _, err) = env.code(&["connectors"]);
    assert_eq!(code, 2);
    assert!(err.contains("no Codex profiles registered"), "{err}");
}

#[test]
fn ec5_16_one_profile_failing_still_lists_the_rest() {
    let env = machine_with_kilo_codex();
    let kilo = env.kilo_codex();
    codex_list(
        &env.home.join(".codex"),
        json!([connector("gmail@openai-curated-remote", "gmail")]),
        json!([]),
    );
    fs::write(kilo.join("fake-plugin-list.json"), "not json").unwrap();
    let (code, out, err) = env.code(&["connectors"]);
    assert_eq!(code, 1);
    assert!(out.contains("gmail@openai-curated-remote"), "{out}");
    assert!(err.contains("warning: kilo-codex: invalid JSON"), "{err}");
    let env = env.var("FAKE_CODEX_FAIL", "list");
    let (code, _, err) = env.code(&["connectors"]);
    assert_eq!(code, 1);
    assert!(err.contains("warning: codex:"), "{err}");
}

// ---------------------------------------------------------------- add source <id> (T7)

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Marketplaces write GitHub URLs with `.git`; the local stand-in answers to that spelling too.
fn git_alias(repo: &Path) {
    let alias = repo.with_extension("git");
    std::os::unix::fs::symlink(repo, alias).unwrap();
}

fn sources(env: &Env) -> Vec<toml::Value> {
    let t: toml::Table = toml::from_str(&env.config()).unwrap();
    t.get("source")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default()
}

/// superpowers on "GitHub" at v1 and v2; the official marketplace lists it at v1.
fn listed_superpowers(env: &Env) -> (String, String) {
    let (repo, v1) = env.github_repo("obra/superpowers", "superpowers", "superpowers-dev");
    git_alias(&repo);
    let v2 = env.upstream_commit(&repo, "v2");
    claude_market(
        env,
        &env.home.join(".claude"),
        "official",
        "anthropics/official",
        json!([
            {"name": "superpowers", "source": {"source": "url",
             "url": "https://github.com/obra/superpowers.git", "sha": v1}},
            {"name": "xcode", "source": {"source": "command", "command": "xcrun agent plugin path"}},
            {"name": "missing", "source": {"source": "github", "repo": "none/missing"}}
        ]),
    );
    (v1, v2)
}

#[test]
fn ac5_23_add_source_from_a_result() {
    let env = machine();
    let (v1, v2) = listed_superpowers(&env);
    let out = env.ok(&["add", "source", "superpowers@official"]);
    assert!(
        out.contains(&format!("added source superpowers at {}", &v1[..7])),
        "{out}"
    );
    let s = &sources(&env)[0];
    assert_eq!(s["name"].as_str(), Some("superpowers"));
    assert_eq!(
        s["repo"].as_str(),
        Some("https://github.com/obra/superpowers.git")
    );
    assert_eq!(
        s["rev"].as_str(),
        Some(v1.as_str()),
        "the catalog's commit is pinned"
    );
    assert!(
        env.installed(&env.home.join(".claude"))
            .contains(&"superpowers@superpowers-dev".to_string())
    );
    // --rev wins over the catalog's commit.
    let env2 = machine();
    listed_superpowers(&env2);
    let v2b = head_of_github(&env2);
    env2.ok(&["add", "source", "superpowers@official", "--rev", &v2b]);
    assert_eq!(sources(&env2)[0]["rev"].as_str(), Some(v2b.as_str()));
    let _ = v2;
}

fn head_of_github(env: &Env) -> String {
    sh(
        &env.home.parent().unwrap().join("github/obra/superpowers"),
        &["rev-parse", "HEAD"],
    )
}

#[test]
fn ac5_24_a_plugin_in_a_subfolder() {
    let env = machine();
    let base = env.github_is_local();
    let repo = base.join("o/plugins");
    write(
        &repo.join("plugins/x/.claude-plugin/plugin.json"),
        &json!({"name": "x", "description": "in a folder"}),
    );
    fs::write(repo.join("README.md"), "r").unwrap();
    sh(&repo, &["init", "-q"]);
    sh(&repo, &["add", "-A"]);
    sh(&repo, &["commit", "-qm", "init"]);
    let head = sh(&repo, &["rev-parse", "HEAD"]);
    git_alias(&repo);
    claude_market(
        &env,
        &env.home.join(".claude"),
        "official",
        "anthropics/official",
        json!([{"name": "x", "source": {"source": "git-subdir",
                 "url": "https://github.com/o/plugins.git", "path": "plugins/x", "sha": head}}]),
    );
    env.ok(&["add", "source", "x@official"]);
    let s = &sources(&env)[0];
    assert_eq!(s["path"].as_str(), Some("plugins/x"));
    assert_eq!(s["rev"].as_str(), Some(head.as_str()));
    assert!(
        env.installed(&env.home.join(".claude"))
            .contains(&"x@colctl-x".to_string()),
        "{:?}",
        env.installed(&env.home.join(".claude"))
    );
}

#[test]
fn ac5_25_results_without_a_repo() {
    let env = machine();
    listed_superpowers(&env);
    let (code, _, err) = env.code(&["add", "source", "xcode@official"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("xcode@official has no repo to clone; colctl add plugin xcode@official keeps it on the marketplace list"),
        "{err}"
    );
    assert!(sources(&env).is_empty());
}

#[test]
fn ac5_26_already_a_source() {
    let env = machine();
    listed_superpowers(&env);
    env.ok(&["add", "source", "superpowers@official"]);
    let (code, _, err) = env.code(&["add", "source", "superpowers@official"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("superpowers@official is already source plugin superpowers"),
        "{err}"
    );
}

#[test]
fn ec5_18_a_git_at_repo_is_still_a_repo() {
    let env = machine();
    let out = env.ok(&["add", "source", "git@github.com:o/r.git", "--dry-run"]);
    assert!(
        out.contains("clone git@github.com:o/r.git as source r"),
        "{out}"
    );
}

#[test]
fn ec5_19_unknown_ids_and_bare_names() {
    let env = machine();
    listed_superpowers(&env);
    let (code, _, err) = env.code(&["add", "source", "nope@official"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("nope@official isn't a repo or a plugin id from colctl search"),
        "{err}"
    );
    let (code, _, err) = env.code(&["add", "source", "superpowers"]);
    assert_eq!(code, 2);
    assert!(err.contains("or a plugin id from colctl search"), "{err}");
    assert!(sources(&env).is_empty());
}

#[test]
fn ec5_20_an_unreachable_repo_writes_nothing() {
    let env = machine();
    listed_superpowers(&env);
    let config = env.config();
    let (code, _, _) = env.code(&["add", "source", "missing@official"]);
    assert_eq!(code, 2);
    assert_eq!(env.config(), config);
    assert!(!env.source.join("plugins/missing").exists());
}

#[test]
fn ec5_21_name_and_plugin_override_the_result() {
    let env = machine();
    listed_superpowers(&env);
    let out = env.ok(&[
        "add",
        "source",
        "superpowers@official",
        "--name",
        "sp",
        "--plugin",
        "superpowers",
        "--dry-run",
    ]);
    assert!(out.contains("as source sp"), "{out}");
    env.ok(&["add", "source", "superpowers@official", "--name", "sp"]);
    assert_eq!(sources(&env)[0]["name"].as_str(), Some("sp"));
}

// ---------------------------------------------------------------- found by the smoke test

#[test]
fn ec5_22_a_catalog_name_that_differs_from_the_plugins_own() {
    let env = machine();
    let base = env.github_is_local();
    let repo = base.join("acme/tools-plugin");
    write(
        &repo.join(".claude-plugin/plugin.json"),
        &json!({"name": "onepw", "version": "0.2.2"}),
    );
    write(
        &repo.join(".claude-plugin/marketplace.json"),
        &json!({"name": "acme", "plugins": [{"name": "onepw", "source": "./"}]}),
    );
    sh(&repo, &["init", "-q"]);
    sh(&repo, &["add", "-A"]);
    sh(&repo, &["commit", "-qm", "init"]);
    let head = sh(&repo, &["rev-parse", "HEAD"]);
    let native = env.home.join(".claude");
    write(
        &native.join("plugins/plugin-directory-cache-v2.json"),
        &json!({"schemaVersion": 2, "entryNames": {"p0": "acme-developer-tools"},
            "listings": [{"id": "p0", "name": "Acme Developer Tools", "summary": "Tools",
                "surfaces": ["claude_code"],
                "source": {"type": "directory", "repository": {
                    "url": "https://github.com/acme/tools-plugin", "reviewed_commit": head}}}]}),
    );
    env.ok(&[
        "add",
        "source",
        "acme-developer-tools@anthropic-plugin-directory",
    ]);
    let s = &sources(&env)[0];
    assert_eq!(s["name"].as_str(), Some("acme-developer-tools"));
    assert_eq!(s["plugin"].as_str(), Some("onepw"), "the plugin's own name");
    assert!(env.installed(&native).contains(&"onepw@acme".to_string()));
    // The directory listing and the source are now one result.
    let (code, _, err) = env.code(&[
        "add",
        "source",
        "acme-developer-tools@anthropic-plugin-directory",
    ]);
    assert_eq!(code, 2);
    assert!(
        err.contains("is already source plugin acme-developer-tools"),
        "{err}"
    );
    let (_, v) = json_of(
        &env,
        &[
            "show",
            "acme-developer-tools@anthropic-plugin-directory",
            "--json",
        ],
    );
    assert_eq!(
        v["result"]["version"], "0.2.2",
        "EC5-23: the source's own version"
    );
}

#[test]
fn ec5_24_a_closed_pipe_ends_quietly() {
    let env = machine();
    let listings: Vec<(String, String, String, String)> = (0..4000)
        .map(|i| {
            (
                format!("plugin-{i}"),
                format!("Plugin {i}"),
                "x".repeat(50),
                format!("https://github.com/o{i}/r"),
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
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("colctl"))
        .args(["search", "--limit", "0"])
        .env_clear()
        .env("HOME", &env.home)
        .env("PATH", &env.path)
        .current_dir(&env.home)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Read;
        let mut first = [0u8; 16];
        child.stdout.take().unwrap().read_exact(&mut first).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("panicked"), "{err}");
    assert_eq!(out.status.code(), Some(141), "{err}");
}
