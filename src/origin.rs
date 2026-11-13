//! Where an installed plugin really comes from: its source repo, the plugin's folder in it, and the
//! commit that's installed (AC3-18…20). Read from what Claude and Codex recorded; no network.

use crate::adapter;
use crate::config::{self, PLUGIN_DIRECTORY, split_plugin_id};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// A plugin's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// What `git clone` gets.
    pub repo: String,
    /// The plugin's folder inside the repo, if not the root.
    pub subpath: Option<String>,
    /// The plugin's name.
    pub plugin: String,
    /// The installed commit, when the tool recorded it or the marketplace checkout knows it.
    pub rev: Option<String>,
}

/// Reads the commit a folder's git checkout is at.
pub type HeadOf<'a> = &'a dyn Fn(&Path) -> Option<String>;

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// A path inside a repo, or `None` for its root.
fn subpath(relative: &str) -> Option<String> {
    let clean = relative.trim_start_matches("./").trim_matches('/');
    (!clean.is_empty() && clean != ".").then(|| clean.to_string())
}

/// A plugin entry's `source` in a marketplace manifest → (repo, subpath), given the marketplace's own repo.
/// Shared with the discovery catalog (cycle 5).
pub(crate) fn plugin_source(
    entry: &Value,
    market_repo: Option<&str>,
) -> Option<(String, Option<String>)> {
    match entry {
        Value::String(rel) => Some((market_repo?.to_string(), subpath(rel))),
        Value::Object(o) => {
            let field = |k: &str| o.get(k).and_then(Value::as_str);
            match field("source")? {
                "github" => Some((format!("https://github.com/{}", field("repo")?), None)),
                // AC5-07: a folder inside another repo, at a recorded commit.
                "url" | "git" => {
                    let url = field("url")?;
                    if url.starts_with('.') {
                        Some((market_repo?.to_string(), subpath(url)))
                    } else {
                        Some((url.to_string(), None))
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// The commit a plugin entry's `source` records (`sha`), if any (AC5-07).
pub(crate) fn plugin_commit(entry: &Value) -> Option<String> {
    entry
        .get("sha")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// The entry for `plugin` in the marketplace manifest at `manifest`.
fn manifest_entry(manifest: &Path, plugin: &str) -> Option<Value> {
    read_json(manifest)?
        .get("plugins")?
        .as_array()?
        .iter()
        .find(|p| p.get("name").and_then(Value::as_str) == Some(plugin))?
        .get("source")
        .cloned()
}

/// The source of `id` installed in a Claude profile.
pub fn claude(harness: &Path, id: &str, head_of: HeadOf) -> Option<Found> {
    let (plugin, market) = split_plugin_id(id)?;
    let installed = read_json(&harness.join("plugins/installed_plugins.json"))?;
    let entry = installed
        .get("plugins")?
        .get(id)?
        .as_array()?
        .first()?
        .clone();
    let sha = entry
        .get("gitCommitSha")
        .and_then(Value::as_str)
        .map(str::to_string);
    if market == PLUGIN_DIRECTORY {
        let bindings = read_json(&harness.join("plugins/plugin-directory-bindings.json"))?;
        let key = bindings
            .get("bindings")?
            .get(id)?
            .get("repositoryKey")?
            .as_str()?;
        let (repo, sub) = key.split_once('#').unwrap_or((key, ""));
        return Some(Found {
            repo: format!("https://{}", repo.trim_end_matches('/')),
            subpath: subpath(sub),
            plugin: plugin.to_string(),
            rev: sha,
        });
    }
    let known = read_json(&harness.join("plugins/known_marketplaces.json"))?;
    let market_entry = known.get(market)?;
    let market_repo = adapter::marketplace_source(market_entry).map(|s| config::clone_url(&s));
    let dir = PathBuf::from(market_entry.get("installLocation")?.as_str()?);
    let source = manifest_entry(&dir.join(".claude-plugin/marketplace.json"), plugin)?;
    let (repo, sub) = plugin_source(&source, market_repo.as_deref())?;
    let inside = market_repo.as_deref() == Some(repo.as_str());
    Some(Found {
        rev: sha
            .or_else(|| plugin_commit(&source))
            .or_else(|| if inside { head_of(&dir) } else { None }),
        repo,
        subpath: sub,
        plugin: plugin.to_string(),
    })
}

/// The source of `id` installed in a Codex profile.
pub fn codex(harness: &Path, id: &str, head_of: HeadOf) -> Option<Found> {
    let (plugin, market) = split_plugin_id(id)?;
    let config: toml::Table =
        toml::from_str(&fs::read_to_string(harness.join("config.toml")).ok()?).ok()?;
    config.get("plugins")?.as_table()?.get(id)?;
    let m = config
        .get("marketplaces")?
        .as_table()?
        .get(market)?
        .as_table()?;
    let source_type = m.get("source_type")?.as_str()?;
    let source = m.get("source")?.as_str()?.to_string();
    let root = match source_type {
        "git" => harness.join(".tmp/marketplaces").join(market),
        "local" => PathBuf::from(&source),
        _ => return None,
    };
    let entry = manifest_entry(&root.join(".agents/plugins/marketplace.json"), plugin)?;
    let (repo, sub) = plugin_source(&entry, Some(&source))?;
    let inside = repo == source;
    Some(Found {
        rev: if inside { head_of(&root) } else { None },
        repo,
        subpath: sub,
        plugin: plugin.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SHA: &str = "8ca22dba9a94f28898bbce59f2537ff4d87c747d";
    const HEAD: &str = "1111111111111111111111111111111111111111";

    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value.to_string()).unwrap();
    }

    fn head(_: &Path) -> Option<String> {
        Some(HEAD.to_string())
    }

    fn no_head(_: &Path) -> Option<String> {
        None
    }

    fn installed(harness: &Path, id: &str, sha: Option<&str>) {
        let mut entry = json!({"scope": "user"});
        if let Some(sha) = sha {
            entry["gitCommitSha"] = json!(sha);
        }
        write(
            &harness.join("plugins/installed_plugins.json"),
            &json!({"version": 2, "plugins": {id: [entry]}}),
        );
    }

    #[test]
    fn ac3_18_a_plugin_directory_plugin() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path();
        installed(h, "superpowers@anthropic-plugin-directory", Some(SHA));
        write(
            &h.join("plugins/plugin-directory-bindings.json"),
            &json!({"schemaVersion": 1, "bindings": {"superpowers@anthropic-plugin-directory": {"repositoryKey": "github.com/obra/superpowers#"}}}),
        );
        assert_eq!(
            claude(h, "superpowers@anthropic-plugin-directory", &no_head),
            Some(Found {
                repo: "https://github.com/obra/superpowers".into(),
                subpath: None,
                plugin: "superpowers".into(),
                rev: Some(SHA.into()),
            })
        );
        assert_eq!(
            claude(h, "other@anthropic-plugin-directory", &no_head),
            None
        );
    }

    #[test]
    fn ec3_11_a_directory_binding_with_a_subpath() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path();
        installed(h, "x@anthropic-plugin-directory", None);
        write(
            &h.join("plugins/plugin-directory-bindings.json"),
            &json!({"bindings": {"x@anthropic-plugin-directory": {"repositoryKey": "github.com/o/r#plugins/x"}}}),
        );
        let f = claude(h, "x@anthropic-plugin-directory", &no_head).unwrap();
        assert_eq!(f.repo, "https://github.com/o/r");
        assert_eq!(f.subpath.as_deref(), Some("plugins/x"));
        assert_eq!(f.rev, None);
    }

    fn claude_marketplace(h: &Path, market_dir: &Path, source: Value, plugin_source: Value) {
        write(
            &h.join("plugins/known_marketplaces.json"),
            &json!({"m": {"source": source, "installLocation": market_dir}}),
        );
        write(
            &market_dir.join(".claude-plugin/marketplace.json"),
            &json!({"name": "m", "plugins": [{"name": "p", "source": plugin_source}]}),
        );
    }

    #[test]
    fn ac3_19_a_marketplace_plugin_pointing_at_another_repo() {
        let t = tempfile::tempdir().unwrap();
        let (h, dir) = (t.path().join("h"), t.path().join("market"));
        claude_marketplace(
            &h,
            &dir,
            json!({"source": "github", "repo": "agg/regator"}),
            json!({"source": "github", "repo": "author/plugin"}),
        );
        installed(&h, "p@m", Some(SHA));
        let f = claude(&h, "p@m", &head).unwrap();
        assert_eq!(f.repo, "https://github.com/author/plugin");
        assert_eq!(f.rev.as_deref(), Some(SHA));
        claude_marketplace(
            &h,
            &dir,
            json!({"source": "github", "repo": "agg/regator"}),
            json!({"source": "url", "url": "https://git.example/p.git"}),
        );
        assert_eq!(
            claude(&h, "p@m", &head).unwrap().repo,
            "https://git.example/p.git"
        );
    }

    #[test]
    fn ac3_19_a_marketplace_plugin_inside_the_marketplace_repo() {
        let t = tempfile::tempdir().unwrap();
        let (h, dir) = (t.path().join("h"), t.path().join("market"));
        claude_marketplace(
            &h,
            &dir,
            json!({"source": "github", "repo": "author/repo"}),
            json!("./plugins/p"),
        );
        installed(&h, "p@m", None);
        let f = claude(&h, "p@m", &head).unwrap();
        assert_eq!(f.repo, "https://github.com/author/repo");
        assert_eq!(f.subpath.as_deref(), Some("plugins/p"));
        assert_eq!(
            f.rev.as_deref(),
            Some(HEAD),
            "falls back to the marketplace checkout's commit"
        );
        claude_marketplace(
            &h,
            &dir,
            json!({"source": "git", "url": "https://git.example/r.git"}),
            json!("./"),
        );
        let f = claude(&h, "p@m", &head).unwrap();
        assert_eq!(
            (f.repo.as_str(), f.subpath),
            ("https://git.example/r.git", None)
        );
        claude_marketplace(
            &h,
            &dir,
            json!({"source": "npm", "package": "x"}),
            json!("./"),
        );
        assert_eq!(claude(&h, "p@m", &head), None, "unknown marketplace origin");
    }

    fn codex_marketplace(
        h: &Path,
        root: &Path,
        source_type: &str,
        source: &str,
        plugin_source: Value,
    ) {
        fs::create_dir_all(h).unwrap();
        fs::write(
            h.join("config.toml"),
            format!("[marketplaces.m]\nsource_type = \"{source_type}\"\nsource = \"{source}\"\n\n[plugins.\"p@m\"]\nenabled = true\n"),
        )
        .unwrap();
        write(
            &root.join(".agents/plugins/marketplace.json"),
            &json!({"name": "m", "plugins": [{"name": "p", "source": plugin_source}]}),
        );
    }

    #[test]
    fn ac3_20_a_codex_plugin() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path().join("codex");
        let root = h.join(".tmp/marketplaces/m");
        codex_marketplace(
            &h,
            &root,
            "git",
            "https://github.com/obra/superpowers.git",
            json!({"source": "url", "url": "./"}),
        );
        assert_eq!(
            codex(&h, "p@m", &head),
            Some(Found {
                repo: "https://github.com/obra/superpowers.git".into(),
                subpath: None,
                plugin: "p".into(),
                rev: Some(HEAD.into()),
            })
        );
        codex_marketplace(
            &h,
            &root,
            "git",
            "https://github.com/agg/x.git",
            json!({"source": "url", "url": "https://github.com/author/p.git"}),
        );
        assert_eq!(
            codex(&h, "p@m", &head).unwrap().repo,
            "https://github.com/author/p.git"
        );
        assert_eq!(codex(&h, "q@m", &head), None);
    }

    #[test]
    fn ec3_18_a_local_codex_marketplace_without_a_known_commit() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path().join("codex");
        let local: PathBuf = t.path().join("local-market");
        codex_marketplace(
            &h,
            &local,
            "local",
            local.to_str().unwrap(),
            json!({"source": "url", "url": "./"}),
        );
        let f = codex(&h, "p@m", &no_head).unwrap();
        assert_eq!(f.repo, local.display().to_string());
        assert_eq!(f.rev, None);
    }
}
