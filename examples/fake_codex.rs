//! A stand-in for the `codex` CLI, used by colctl's integration tests (never shipped).
//!
//! It understands `plugin marketplace add|remove` and `plugin add|remove`, keeps
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
        let mut text = fs::read_to_string(&log).unwrap_or_default();
        text.push_str(&line.to_string());
        text.push('\n');
        fs::write(&log, text).unwrap();
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
            let name = env::var("FAKE_CODEX_MARKET_NAME").unwrap_or_else(|_| {
                src.trim_end_matches(".git")
                    .rsplit('/')
                    .next()
                    .unwrap_or(src)
                    .to_string()
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
