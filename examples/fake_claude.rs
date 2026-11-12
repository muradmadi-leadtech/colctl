//! A stand-in for the `claude` CLI, used by colctl's integration tests (never shipped).
//!
//! It understands `plugin marketplace add|remove` and `plugin install|uninstall`, keeps
//! `plugins/installed_plugins.json` and `plugins/known_marketplaces.json` the way Claude Code does,
//! and appends one JSON line per call to `$FAKE_CLAUDE_LOG`. `FAKE_CLAUDE_FAIL=<text>` makes any
//! call containing `<text>` exit 1; `FAKE_CLAUDE_MARKET_NAME` overrides the name a marketplace
//! records itself under; `FAKE_CLAUDE_SLEEP=<secs>` delays every call.

use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

fn read(path: &Path, default: Value) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(default)
}

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn source_entry(src: &str) -> Value {
    if src.contains("://") {
        json!({"source": "url", "url": src})
    } else if src.starts_with('/') {
        json!({"source": "directory", "path": src})
    } else {
        json!({"source": "github", "repo": src})
    }
}

/// The installed_plugins.json entry: a local (directory) marketplace records its checkout's commit.
fn install_entry(markets: &Value, market: &str) -> Value {
    let mut entry = json!({"scope": "user"});
    let location = markets[market]["installLocation"]
        .as_str()
        .unwrap_or_default();
    if markets[market]["source"]["source"] == "directory" {
        let out = std::process::Command::new("git")
            .args(["-C", location, "rev-parse", "HEAD"])
            .output()
            .unwrap();
        if out.status.success() {
            entry["gitCommitSha"] = json!(String::from_utf8_lossy(&out.stdout).trim());
        }
    }
    entry
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config_dir = env::var_os("CLAUDE_CONFIG_DIR");
    let dir = config_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap()).join(".claude"));

    if let Some(log) = env::var_os("FAKE_CLAUDE_LOG") {
        let anthropic: Vec<String> = env::vars()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with("ANTHROPIC_"))
            .collect();
        let line = json!({
            "args": args,
            "config_dir": config_dir.map(|d| d.to_string_lossy().into_owned()),
            "secure_storage": env::var_os("CLAUDE_SECURESTORAGE_CONFIG_DIR").is_some(),
            "anthropic": anthropic,
        });
        let mut text = fs::read_to_string(&log).unwrap_or_default();
        text.push_str(&line.to_string());
        text.push('\n');
        fs::write(&log, text).unwrap();
    }
    if let Some(secs) = env::var("FAKE_CLAUDE_SLEEP")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        std::thread::sleep(std::time::Duration::from_secs(secs));
    }
    let joined = args.join(" ");
    if let Ok(needle) = env::var("FAKE_CLAUDE_FAIL")
        && !needle.is_empty()
        && joined.contains(&needle)
    {
        eprintln!("fake failure for: {joined}");
        exit(1);
    }

    let installed_path = dir.join("plugins/installed_plugins.json");
    let markets_path = dir.join("plugins/known_marketplaces.json");
    let mut markets = read(&markets_path, json!({}));
    let mut installed = read(&installed_path, json!({"version": 2, "plugins": {}}));
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin", "marketplace", "add", src] => {
            let name = env::var("FAKE_CLAUDE_MARKET_NAME").unwrap_or_else(|_| {
                src.trim_end_matches(".git")
                    .rsplit('/')
                    .next()
                    .unwrap_or(src)
                    .to_string()
            });
            markets.as_object_mut().unwrap().insert(
                name,
                json!({"source": source_entry(src), "installLocation": "/fake"}),
            );
            write(&markets_path, &markets);
        }
        ["plugin", "marketplace", "remove", name] => {
            markets.as_object_mut().unwrap().remove(*name);
            write(&markets_path, &markets);
        }
        ["plugin", "install", id] => {
            let market = id.rsplit_once('@').map(|(_, m)| m).unwrap_or_default();
            if market != "anthropic-plugin-directory" && markets.get(market).is_none() {
                eprintln!("Plugin \"{id}\" not found: marketplace \"{market}\" is not known");
                exit(1);
            }
            let entry = install_entry(&markets, market);
            let plugins = installed["plugins"].as_object_mut().unwrap();
            plugins.insert(id.to_string(), json!([entry]));
            write(&installed_path, &installed);
        }
        ["plugin", "update", id] => {
            let market = id.rsplit_once('@').map(|(_, m)| m).unwrap_or_default();
            if installed["plugins"].get(*id).is_none() {
                eprintln!("Plugin \"{id}\" is not installed");
                exit(1);
            }
            let entry = install_entry(&markets, market);
            installed["plugins"][*id] = json!([entry]);
            write(&installed_path, &installed);
        }
        ["plugin", "uninstall", id] => {
            let plugins: &mut Map<String, Value> = installed["plugins"].as_object_mut().unwrap();
            if plugins.remove(*id).is_none() {
                eprintln!("Plugin \"{id}\" is not installed");
                exit(1);
            }
            write(&installed_path, &installed);
        }
        _ => {
            eprintln!("fake claude doesn't understand: {joined}");
            exit(2);
        }
    }
}
