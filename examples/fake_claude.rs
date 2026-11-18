//! A stand-in for the `claude` CLI, used by colctl's integration tests (never shipped).
//!
//! It understands `plugin marketplace add|remove` and `plugin install|uninstall`, keeps
//! `plugins/installed_plugins.json` and `plugins/known_marketplaces.json` the way Claude Code does,
//! and appends one JSON line per call to `$FAKE_CLAUDE_LOG`. `FAKE_CLAUDE_FAIL=<text>` makes any
//! call containing `<text>` exit 1; `FAKE_CLAUDE_MARKET_NAME` overrides the name a marketplace
//! records itself under; `FAKE_CLAUDE_SLEEP=<secs>` delays every call; `FAKE_CLAUDE_SHA` is the
//! commit recorded for every install, instead of the marketplace folder's. `mcp add|remove -s user`
//! keep `.claude.json` as Claude Code does.

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

/// The version a marketplace's plugin declares in its plugin.json ("0.0.0" when it says none).
fn plugin_version(markets: &Value, market: &str, name: &str) -> String {
    let location = Path::new(
        markets[market]["installLocation"]
            .as_str()
            .unwrap_or_default(),
    );
    let listing = read(&location.join(".claude-plugin/marketplace.json"), json!({}));
    let source = listing["plugins"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["name"] == name))
        .and_then(|p| p["source"].as_str())
        .unwrap_or("./");
    let manifest = read(
        &location.join(source).join(".claude-plugin/plugin.json"),
        json!({}),
    );
    manifest["version"].as_str().unwrap_or("0.0.0").to_string()
}

/// The installed_plugins.json entry: a local (directory) marketplace records its checkout's commit,
/// and every entry records the plugin's version, as Claude keys its cache by it.
fn install_entry(markets: &Value, market: &str, name: &str) -> Value {
    let mut entry = json!({"scope": "user", "version": plugin_version(markets, market, name)});
    let location = markets[market]["installLocation"]
        .as_str()
        .unwrap_or_default();
    // A tool that installed from somewhere else than the folder (e.g. a manifest pointing at GitHub).
    if let Ok(sha) = env::var("FAKE_CLAUDE_SHA") {
        entry["gitCommitSha"] = json!(sha);
        return entry;
    }
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
            "config_dir": config_dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
            "secure_storage": env::var_os("CLAUDE_SECURESTORAGE_CONFIG_DIR").is_some(),
            "anthropic": anthropic,
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

    if args.first().map(String::as_str) == Some("mcp") {
        mcp(&args[1..], config_dir.is_some(), &dir);
        return;
    }

    let installed_path = dir.join("plugins/installed_plugins.json");
    let markets_path = dir.join("plugins/known_marketplaces.json");
    let mut markets = read(&markets_path, json!({}));
    let mut installed = read(&installed_path, json!({"version": 2, "plugins": {}}));
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin", "marketplace", "add", src] => {
            let manifest = Path::new(src).join(".claude-plugin/marketplace.json");
            let local = src.starts_with('/') && manifest.is_file();
            let name = if local {
                read(&manifest, json!({}))["name"]
                    .as_str()
                    .unwrap()
                    .to_string()
            } else {
                env::var("FAKE_CLAUDE_MARKET_NAME").unwrap_or_else(|_| {
                    src.trim_end_matches(".git")
                        .rsplit('/')
                        .next()
                        .unwrap_or(src)
                        .to_string()
                })
            };
            // Like Claude: Anthropic's marketplace names only come from GitHub's anthropics org.
            if local && name == "claude-plugins-official" {
                eprintln!(
                    "✘ Failed to add marketplace: The name 'claude-plugins-official' is reserved for official Anthropic marketplaces and can only be used with GitHub sources from the 'anthropics' organization."
                );
                exit(1);
            }
            let location = if local {
                src.to_string()
            } else {
                "/fake".to_string()
            };
            markets.as_object_mut().unwrap().insert(
                name,
                json!({"source": source_entry(src), "installLocation": location}),
            );
            write(&markets_path, &markets);
        }
        ["plugin", "marketplace", "update"] => {}
        ["plugin", "marketplace", "update", name] => {
            if markets.get(*name).is_none() {
                eprintln!("marketplace {name} not found");
                exit(1);
            }
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
            let name = id.rsplit_once('@').map_or(*id, |(n, _)| n);
            let entry = install_entry(&markets, market, name);
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
            // Like Claude: the same version is "already at the latest version", whatever the commit.
            let name = id.rsplit_once('@').map_or(*id, |(n, _)| n);
            let now = &installed["plugins"][*id][0]["version"];
            if now.as_str() == Some(plugin_version(&markets, market, name).as_str()) {
                println!("✔ {name} is already at the latest version");
                return;
            }
            let entry = install_entry(&markets, market, name);
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

/// `mcp add|remove -s user`, kept in `.claude.json` the way Claude Code 2.1 does: in the profile
/// folder when `CLAUDE_CONFIG_DIR` is set, else `~/.claude.json`. `FAKE_CLAUDE_MCP_GONE` makes
/// `remove` say the server doesn't exist without touching the file.
fn mcp(args: &[String], configured: bool, dir: &Path) {
    let file = if configured {
        dir.join(".claude.json")
    } else {
        PathBuf::from(env::var_os("HOME").unwrap()).join(".claude.json")
    };
    let mut doc = read(&file, json!({}));
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["add", "-s", "user", rest @ ..] => {
            let mut http = false;
            let mut env_map = Map::new();
            let mut headers = Map::new();
            let mut positional: Vec<&str> = Vec::new();
            let mut command: Vec<&str> = Vec::new();
            let mut i = 0;
            while i < rest.len() {
                match rest[i] {
                    "-t" => {
                        http = rest[i + 1] == "http";
                        i += 2;
                    }
                    "-e" => {
                        let (k, v) = rest[i + 1].split_once('=').unwrap();
                        env_map.insert(k.into(), json!(v));
                        i += 2;
                    }
                    "-H" => {
                        let (k, v) = rest[i + 1].split_once(": ").unwrap();
                        headers.insert(k.into(), json!(v));
                        i += 2;
                    }
                    "--" => {
                        command = rest[i + 1..].to_vec();
                        i = rest.len();
                    }
                    other => {
                        positional.push(other);
                        i += 1;
                    }
                }
            }
            let name = positional[0];
            let entry = if http {
                let mut e = json!({"type": "http", "url": positional[1]});
                if !headers.is_empty() {
                    e["headers"] = Value::Object(headers);
                }
                e
            } else {
                json!({"type": "stdio", "command": command[0], "args": command[1..],
                       "env": Value::Object(env_map)})
            };
            if !doc["mcpServers"].is_object() {
                doc["mcpServers"] = json!({});
            }
            doc["mcpServers"][name] = entry;
            write(&file, &doc);
        }
        ["remove", "-s", "user", name] => {
            let gone = env::var_os("FAKE_CLAUDE_MCP_GONE").is_some()
                || doc["mcpServers"].get(*name).is_none();
            if gone {
                eprintln!("No MCP server named \"{name}\" in user scope");
                exit(1);
            }
            doc["mcpServers"].as_object_mut().unwrap().remove(*name);
            write(&file, &doc);
        }
        _ => {
            eprintln!("fake claude doesn't understand: mcp {}", args.join(" "));
            exit(2);
        }
    }
}
