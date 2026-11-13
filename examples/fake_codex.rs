//! A stand-in for the `codex` CLI, used by colctl's integration tests (never shipped).
//!
//! It understands `plugin marketplace add|remove|upgrade`, `plugin add|remove` and
//! `plugin list --available --json` (from `$CODEX_HOME/fake-plugin-list.json`), keeps
//! `$CODEX_HOME/config.toml` (`[marketplaces.<name>]`, `[plugins."<id>"]`) the way Codex does,
//! and appends one JSON line per call to `$FAKE_CODEX_LOG`. `FAKE_CODEX_FAIL=<text>` makes any
//! call containing `<text>` exit 1; `FAKE_CODEX_MARKET_NAME` overrides the name a marketplace
//! records itself under; `FAKE_CODEX_SLEEP=<secs>` delays every call.

use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let codex_home = env::var_os("CODEX_HOME");
    let dir = codex_home
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap()).join(".codex"));

    if let Some(log) = env::var_os("FAKE_CODEX_LOG") {
        let openai: Vec<String> = env::vars()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with("OPENAI_"))
            .collect();
        let line = json!({
            "args": args,
            "codex_home": codex_home.map(|d| d.to_string_lossy().into_owned()),
            "openai": openai,
        });
        // One append per call, so calls running in parallel don't overwrite each other.
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .unwrap();
        file.write_all(format!("{line}\n").as_bytes()).unwrap();
    }
    if let Some(secs) = env::var("FAKE_CODEX_SLEEP")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    }
    let joined = args.join(" ");
    if let Ok(needle) = env::var("FAKE_CODEX_FAIL")
        && !needle.is_empty()
        && joined.contains(&needle)
    {
        eprintln!("fake codex failure for: {joined}");
        exit(1);
    }

    // Read-only calls: answered and done, without touching config.toml.
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["plugin", "list", "--available", "--json"] => {
            // The catalog a test put in the profile, or an empty one.
            let list = fs::read_to_string(dir.join("fake-plugin-list.json"))
                .unwrap_or_else(|_| r#"{"installed": [], "available": []}"#.into());
            println!("{list}");
            exit(0);
        }
        ["plugin", "marketplace", "upgrade"] => exit(0),
        _ => {}
    }

    let path = dir.join("config.toml");
    let mut config: toml::Table = fs::read_to_string(&path)
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    let table = |config: &mut toml::Table, key: &str| -> toml::Table {
        config
            .get(key)
            .and_then(|v| v.as_table())
            .cloned()
            .unwrap_or_default()
    };
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin", "marketplace", "add", src] => {
            let manifest = PathBuf::from(src).join(".agents/plugins/marketplace.json");
            let from_manifest = (src.starts_with('/') && manifest.is_file())
                .then(|| fs::read_to_string(&manifest).ok())
                .flatten()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v["name"].as_str().map(str::to_string));
            let name = from_manifest.unwrap_or_else(|| {
                env::var("FAKE_CODEX_MARKET_NAME").unwrap_or_else(|_| {
                    src.trim_end_matches(".git")
                        .rsplit('/')
                        .next()
                        .unwrap_or(src)
                        .to_string()
                })
            });
            let (source_type, source) = if src.starts_with('/') {
                ("local", src.to_string())
            } else if src.contains("://") {
                ("git", src.to_string())
            } else {
                ("git", format!("https://github.com/{src}.git"))
            };
            let mut markets = table(&mut config, "marketplaces");
            let mut entry = toml::Table::new();
            entry.insert("source_type".into(), source_type.into());
            entry.insert("source".into(), source.into());
            markets.insert(name, entry.into());
            config.insert("marketplaces".into(), markets.into());
        }
        ["plugin", "marketplace", "remove", name] => {
            let mut markets = table(&mut config, "marketplaces");
            markets.remove(*name);
            config.insert("marketplaces".into(), markets.into());
        }
        ["plugin", "add", id] => {
            let market = id.rsplit_once('@').map(|(_, m)| m).unwrap_or_default();
            if market != "openai-curated"
                && !table(&mut config, "marketplaces").contains_key(market)
            {
                eprintln!("marketplace `{market}` is not configured");
                exit(1);
            }
            // Like Codex: a plugin entry's `url` source is cloned with git, so a folder that isn't a
            // repo fails; a `local` source must exist.
            let plugin = id.rsplit_once('@').map(|(p, _)| p).unwrap_or_default();
            let root = table(&mut config, "marketplaces")
                .get(market)
                .and_then(|m| m.get("source"))
                .and_then(|s| s.as_str())
                .map(PathBuf::from);
            if let Some(root) = root.filter(|r| r.is_absolute()) {
                let manifest: serde_json::Value =
                    fs::read_to_string(root.join(".agents/plugins/marketplace.json"))
                        .ok()
                        .and_then(|t| serde_json::from_str(&t).ok())
                        .unwrap_or_default();
                let source = manifest["plugins"]
                    .as_array()
                    .and_then(|ps| ps.iter().find(|p| p["name"] == plugin))
                    .map(|p| p["source"].clone())
                    .unwrap_or_default();
                let rel = |k: &str| {
                    source[k]
                        .as_str()
                        .filter(|u| u.starts_with('.'))
                        .map(|u| root.join(u))
                };
                match source["source"].as_str() {
                    Some("url") => {
                        if let Some(dir) = rel("url").filter(|d| !d.join(".git").exists()) {
                            eprintln!(
                                "Error: git clone {} failed: repository does not exist",
                                dir.display()
                            );
                            exit(1);
                        }
                    }
                    Some("local") if rel("path").is_none_or(|d| !d.is_dir()) => {
                        eprintln!("Error: plugin path for `{plugin}` does not exist");
                        exit(1);
                    }
                    _ => {}
                }
            }
            let mut plugins = table(&mut config, "plugins");
            let mut entry = toml::Table::new();
            entry.insert("enabled".into(), true.into());
            plugins.insert(id.to_string(), entry.into());
            config.insert("plugins".into(), plugins.into());
        }
        ["plugin", "remove", id] => {
            let mut plugins = table(&mut config, "plugins");
            if plugins.remove(*id).is_none() {
                eprintln!("plugin `{id}` is not installed");
                exit(1);
            }
            config.insert("plugins".into(), plugins.into());
        }
        _ => {
            eprintln!("fake codex doesn't understand: {joined}");
            exit(2);
        }
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
}
