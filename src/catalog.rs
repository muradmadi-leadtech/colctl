//! Every plugin catalog the harnesses on this machine already keep (cycle 5, interfaces.md § Catalog
//! sources): Claude's marketplaces and plugin-directory cache, Codex's marketplaces on disk, and
//! colctl's own sources. Read only; one unreadable catalog becomes a warning, never a failure.

use crate::adapter;
use crate::config::{self, Config, Kind, PLUGIN_DIRECTORY};
use crate::origin;
use crate::packaging::{self, ToolPackaging};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The catalog name for colctl's own `[[source]]` plugins.
pub const COLCTL: &str = "colctl";

/// One plugin as one catalog lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Entry {
    /// `<name>@<marketplace>`.
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    /// The marketplace (or `anthropic-plugin-directory`, or `colctl`).
    pub catalog: String,
    /// `claude` or `codex`.
    pub tool: String,
    /// `plugin` or `connector`.
    pub kind: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    /// Where the code lives, when it's a git repo or a folder.
    pub repo: Option<String>,
    pub subpath: Option<String>,
    pub commit: Option<String>,
    /// Harnesses where this id is installed.
    pub installed: BTreeSet<String>,
    /// The `[[source]]` this plugin is, if any.
    pub source_plugin: Option<String>,
}

/// A marketplace some profile knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marketplace {
    pub name: String,
    pub tool: String,
    pub source: String,
    pub plugins: usize,
    pub profiles: BTreeSet<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub marketplaces: Vec<Marketplace>,
    pub warnings: Vec<String>,
}

fn read_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The author's name: a string, or `{"name": …}`.
fn author(v: &Value) -> Option<String> {
    match v.get("author")? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(o) => o
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        _ => None,
    }
}

/// Entries from one marketplace manifest. Nameless entries are skipped; a repeated name keeps the
/// first (EC5-04).
fn manifest_entries(
    manifest: &Path,
    market: &str,
    tool: &str,
    market_repo: Option<&str>,
) -> Result<Vec<Entry>, String> {
    let Some(doc) = read_json(manifest)? else {
        return Err(format!("{} is missing", manifest.display()));
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for p in doc
        .get("plugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = text(p, "name") else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }
        let source = p.get("source").cloned().unwrap_or(Value::Null);
        let (repo, subpath) = origin::plugin_source(&source, market_repo).unzip();
        let mut keywords = strings(p, "keywords");
        keywords.extend(strings(p, "tags"));
        out.push(Entry {
            id: format!("{name}@{market}"),
            display_name: text(p, "displayName"),
            catalog: market.to_string(),
            tool: tool.to_string(),
            kind: "plugin".into(),
            description: text(p, "description").unwrap_or_default(),
            keywords,
            author: author(p),
            category: text(p, "category"),
            homepage: text(p, "homepage"),
            version: text(p, "version"),
            repo,
            subpath: subpath.flatten(),
            commit: origin::plugin_commit(&source),
            name,
            ..Entry::default()
        });
    }
    Ok(out)
}

/// What the catalog gathers while it reads.
#[derive(Default)]
struct Builder {
    entries: BTreeMap<(String, String), Entry>,
    marketplaces: BTreeMap<(String, String), Marketplace>,
    installed: Vec<(String, String, BTreeSet<String>)>,
    warnings: Vec<String>,
}

impl Builder {
    fn add(&mut self, entries: Vec<Entry>) {
        for e in entries {
            self.entries
                .entry((e.tool.clone(), e.id.clone()))
                .or_insert(e);
        }
    }

    fn market(&mut self, name: &str, tool: &str, source: String, plugins: usize, profile: &str) {
        let m = self
            .marketplaces
            .entry((name.to_string(), tool.to_string()))
            .or_insert_with(|| Marketplace {
                name: name.to_string(),
                tool: tool.to_string(),
                source,
                plugins,
                profiles: BTreeSet::new(),
                managed: false,
            });
        m.plugins = m.plugins.max(plugins);
        m.profiles.insert(profile.to_string());
    }

    fn claude(&mut self, h: &config::Harness) {
        match adapter::claude::read_plugins(&h.path) {
            Ok(p) => self
                .installed
                .push((h.name.clone(), "claude".into(), p.installed)),
            Err(e) => self.warnings.push(format!("{}: {e}", h.name)),
        }
        let known_path = h.path.join("plugins/known_marketplaces.json");
        match read_json(&known_path) {
            Ok(Some(Value::Object(known))) => {
                for (market, entry) in known {
                    let source = adapter::claude::marketplace_source(&entry);
                    let market_repo = source.as_deref().map(config::clone_url);
                    let Some(dir) = entry.get("installLocation").and_then(Value::as_str) else {
                        continue;
                    };
                    let manifest = Path::new(dir).join(".claude-plugin/marketplace.json");
                    if !Path::new(dir).exists() {
                        self.warnings
                            .push(format!("marketplace {market}: {dir} is missing"));
                        continue;
                    }
                    match manifest_entries(&manifest, &market, "claude", market_repo.as_deref()) {
                        Ok(list) => {
                            self.market(
                                &market,
                                "claude",
                                source.unwrap_or_else(|| dir.to_string()),
                                list.len(),
                                &h.name,
                            );
                            self.add(list);
                        }
                        Err(e) => self.warnings.push(e),
                    }
                }
            }
            Ok(_) => {}
            Err(e) => self.warnings.push(e),
        }
        self.directory(h);
    }

    /// Anthropic's plugin directory, from Claude's cache (AC5-02, EC5-02).
    fn directory(&mut self, h: &config::Harness) {
        let path = h.path.join("plugins/plugin-directory-cache-v2.json");
        let doc = match read_json(&path) {
            Ok(Some(doc)) => doc,
            Ok(None) => return,
            Err(e) => {
                self.warnings.push(e);
                return;
            }
        };
        if doc.get("schemaVersion").and_then(Value::as_u64) != Some(2) {
            self.warnings.push(format!(
                "{}: unknown schemaVersion; skipped",
                path.display()
            ));
            return;
        }
        let names = doc.get("entryNames").cloned().unwrap_or(Value::Null);
        let mut list = Vec::new();
        for l in doc
            .get("listings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let on_claude_code = l
                .get("surfaces")
                .and_then(Value::as_array)
                .is_some_and(|s| s.iter().any(|v| v == "claude_code"));
            let Some(slug) = text(l, "id").and_then(|id| text(&names, &id)) else {
                continue;
            };
            if !on_claude_code {
                continue;
            }
            let repository = l
                .get("source")
                .and_then(|s| s.get("repository"))
                .cloned()
                .unwrap_or(Value::Null);
            list.push(Entry {
                id: format!("{slug}@{PLUGIN_DIRECTORY}"),
                name: slug,
                display_name: text(l, "name"),
                catalog: PLUGIN_DIRECTORY.into(),
                tool: "claude".into(),
                kind: "plugin".into(),
                description: text(l, "summary").unwrap_or_default(),
                version: l.get("release").and_then(|r| text(r, "version")),
                repo: text(&repository, "url"),
                subpath: text(&repository, "path"),
                commit: text(&repository, "reviewed_commit"),
                ..Entry::default()
            });
        }
        self.market(
            PLUGIN_DIRECTORY,
            "claude",
            "built in".into(),
            list.len(),
            &h.name,
        );
        self.add(list);
    }

    fn codex(&mut self, h: &config::Harness) {
        let plugins = match adapter::codex::read_plugins(&h.path) {
            Ok(p) => p,
            Err(e) => {
                self.warnings.push(format!("{}: {e}", h.name));
                return;
            }
        };
        self.installed
            .push((h.name.clone(), "codex".into(), plugins.installed.clone()));
        for (market, entry) in &plugins.marketplaces {
            let kind = entry.get("source_type").and_then(Value::as_str);
            let Some(source) = entry.get("source").and_then(Value::as_str) else {
                continue;
            };
            let root = match kind {
                Some("local") => PathBuf::from(source),
                Some("git") => h.path.join(".tmp/marketplaces").join(market),
                _ => continue,
            };
            if !root.exists() {
                self.warnings.push(format!(
                    "marketplace {market}: {} is missing",
                    root.display()
                ));
                continue;
            }
            let manifest = root.join(".agents/plugins/marketplace.json");
            match manifest_entries(&manifest, market, "codex", Some(source)) {
                Ok(list) => {
                    self.market(market, "codex", source.to_string(), list.len(), &h.name);
                    self.add(list);
                }
                Err(e) => self.warnings.push(e),
            }
        }
    }

    /// colctl's own sources, from their clones' packaging (AC5-04).
    fn sources(&mut self, config: &Config, source_dir: &Path) {
        let plugins = source_dir.join("plugins");
        for s in &config.sources {
            let clone = plugins.join(&s.name);
            if !clone.is_dir() {
                continue;
            }
            let packaging = packaging::inspect_with(
                false,
                &clone,
                &plugins.join(".manifests").join(&s.name),
                &s.name,
                &s.plugin,
                s.path.as_deref(),
            );
            let folder = s.path.as_ref().map_or(clone.clone(), |p| clone.join(p));
            let manifest = [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"]
                .iter()
                .find_map(|m| read_json(&folder.join(m)).ok().flatten())
                .unwrap_or(Value::Null);
            let description = text(&manifest, "description").unwrap_or_default();
            // EC5-23: the version at the pinned commit, not another catalog's.
            let version = text(&manifest, "version");
            for (tool, p) in [("claude", &packaging.claude), ("codex", &packaging.codex)] {
                if let ToolPackaging::Supported(t) = p {
                    self.add(vec![Entry {
                        id: t.id.clone(),
                        name: s.plugin.clone(),
                        catalog: COLCTL.into(),
                        tool: tool.into(),
                        kind: "plugin".into(),
                        description: description.clone(),
                        version: version.clone(),
                        display_name: text(&manifest, "displayName"),
                        repo: Some(s.clone_url()),
                        subpath: s.path.clone(),
                        commit: Some(s.rev.clone()),
                        source_plugin: Some(s.name.clone()),
                        ..Entry::default()
                    }]);
                }
            }
        }
    }

    fn finish(mut self, config: &Config, source_dir: &Path) -> Catalog {
        let mut entries: Vec<Entry> = std::mem::take(&mut self.entries).into_values().collect();
        // A plugin that a source installs is that source, whichever catalog lists it.
        let source_ids: BTreeMap<String, String> = entries
            .iter()
            .filter_map(|e| Some((e.id.clone(), e.source_plugin.clone()?)))
            .collect();
        for e in &mut entries {
            for (harness, tool, ids) in &self.installed {
                if tool == &e.tool && ids.contains(&e.id) {
                    e.installed.insert(harness.clone());
                }
            }
            if e.source_plugin.is_none() {
                e.source_plugin = source_ids.get(&e.id).cloned();
            }
        }
        let mut marketplaces: Vec<Marketplace> = self.marketplaces.into_values().collect();
        for m in &mut marketplaces {
            let declared = match m.tool.as_str() {
                "codex" => &config.codex_marketplaces,
                _ => &config.marketplaces,
            };
            m.managed = declared.contains_key(&m.name);
        }
        // Marketplaces colctl added for its sources (folders in ~/.colctl/plugins) are managed too.
        let clones = source_dir.join("plugins");
        for m in &mut marketplaces {
            if Path::new(&m.source).starts_with(&clones) {
                m.managed = true;
            }
        }
        Catalog {
            entries,
            marketplaces,
            warnings: self.warnings,
        }
    }
}

/// Reads every catalog the registered harnesses keep, plus colctl's sources (AC5-01…06).
pub fn build(config: &Config, source_dir: &Path) -> Catalog {
    let mut b = Builder::default();
    // Sources first: the same id under the local marketplace colctl added for a source is that
    // source, and the first entry for an id is the one kept.
    b.sources(config, source_dir);
    for h in &config.harnesses {
        if !h.path.is_dir() {
            continue;
        }
        match h.kind {
            Kind::Claude => b.claude(h),
            Kind::Codex => b.codex(h),
            Kind::Agents => {}
        }
    }
    b.finish(config, source_dir)
}

/// Codex connectors from `codex plugin list --available --json` output (AC5-21): available entries
/// with a remote source.
pub fn connectors(json: &str) -> Result<Vec<Entry>, String> {
    let doc: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut out = Vec::new();
    for list in ["installed", "available"] {
        for p in doc
            .get(list)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let remote =
                p.get("source").and_then(|s| s.get("source")) == Some(&Value::from("remote"));
            let Some(id) = text(p, "pluginId") else {
                continue;
            };
            if !remote {
                continue;
            }
            out.push(Entry {
                name: text(p, "name").unwrap_or_else(|| id.clone()),
                catalog: text(p, "marketplaceName").unwrap_or_default(),
                tool: "codex".into(),
                kind: "connector".into(),
                version: text(p, "version"),
                description: text(p, "description").unwrap_or_default(),
                id,
                ..Entry::default()
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A temp machine with one Claude profile, one Codex profile and a source folder.
    struct Fx {
        _t: tempfile::TempDir,
        root: PathBuf,
        config: Config,
    }

    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value.to_string()).unwrap();
    }

    fn harness(name: &str, kind: Kind, path: PathBuf) -> config::Harness {
        config::Harness {
            name: name.into(),
            kind,
            path,
            exclude: BTreeSet::new(),
        }
    }

    impl Fx {
        fn new() -> Self {
            let t = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(t.path()).unwrap();
            let claude = root.join("claude");
            let codex = root.join("codex");
            fs::create_dir_all(&claude).unwrap();
            fs::create_dir_all(&codex).unwrap();
            let config = Config {
                harnesses: vec![
                    harness("claude", Kind::Claude, claude),
                    harness("codex", Kind::Codex, codex),
                ],
                ..Config::default()
            };
            Fx {
                _t: t,
                root,
                config,
            }
        }

        fn claude(&self) -> PathBuf {
            self.root.join("claude")
        }

        fn codex(&self) -> PathBuf {
            self.root.join("codex")
        }

        /// A Claude marketplace `name` from github `repo` with these plugin entries.
        fn claude_market(&self, profile: &Path, name: &str, repo: &str, plugins: Value) -> PathBuf {
            let dir = self.root.join("markets").join(name);
            write(
                &dir.join(".claude-plugin/marketplace.json"),
                &json!({"name": name, "plugins": plugins}),
            );
            let known = profile.join("plugins/known_marketplaces.json");
            let mut all = read_json(&known).unwrap().unwrap_or(json!({}));
            all[name] = json!({"source": {"source": "github", "repo": repo},
                               "installLocation": dir.display().to_string()});
            write(&known, &all);
            dir
        }

        fn build(&self) -> Catalog {
            build(&self.config, &self.root.join("source"))
        }
    }

    fn find<'a>(c: &'a Catalog, id: &str) -> &'a Entry {
        c.entries
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("no {id} in {:?}", c.entries))
    }

    #[test]
    fn ac5_01_claude_marketplaces_are_read() {
        let f = Fx::new();
        f.claude_market(
            &f.claude(),
            "official",
            "anthropics/claude-plugins-official",
            json!([
                {"name": "local", "description": "in the repo", "source": "./plugins/local",
                 "author": {"name": "Ann"}, "keywords": ["a"], "tags": ["b"], "category": "dev",
                 "version": "1.0", "homepage": "https://h", "displayName": "Local"},
                {"name": "gh", "source": {"source": "github", "repo": "Obra/Superpowers"}, "author": "Jesse"},
                {"name": "url", "source": {"source": "url", "url": "https://gitlab.com/x/y.git", "sha": "abc"}},
                {"name": "sub", "source": {"source": "git-subdir", "url": "https://github.com/o/r.git",
                                            "path": "plugins/s", "sha": "def"}},
                {"name": "cmd", "source": {"source": "command", "command": "xcrun …"}}
            ]),
        );
        let c = f.build();
        let local = find(&c, "local@official");
        assert_eq!(
            local.repo.as_deref(),
            Some("https://github.com/anthropics/claude-plugins-official")
        );
        assert_eq!(local.subpath.as_deref(), Some("plugins/local"));
        assert_eq!(local.author.as_deref(), Some("Ann"));
        assert_eq!(local.keywords, vec!["a", "b"]);
        assert_eq!(
            (
                local.category.as_deref(),
                local.version.as_deref(),
                local.homepage.as_deref()
            ),
            (Some("dev"), Some("1.0"), Some("https://h"))
        );
        assert_eq!(local.display_name.as_deref(), Some("Local"));
        assert_eq!(
            (local.tool.as_str(), local.kind.as_str()),
            ("claude", "plugin")
        );
        assert_eq!(
            find(&c, "gh@official").repo.as_deref(),
            Some("https://github.com/Obra/Superpowers")
        );
        assert_eq!(find(&c, "gh@official").author.as_deref(), Some("Jesse"));
        let url = find(&c, "url@official");
        assert_eq!(
            (url.repo.as_deref(), url.commit.as_deref()),
            (Some("https://gitlab.com/x/y.git"), Some("abc"))
        );
        let sub = find(&c, "sub@official");
        assert_eq!(sub.subpath.as_deref(), Some("plugins/s"));
        assert_eq!(sub.commit.as_deref(), Some("def"));
        assert_eq!(find(&c, "cmd@official").repo, None);
        assert_eq!(c.marketplaces[0].plugins, 5);
        assert_eq!(
            c.marketplaces[0].source,
            "github:anthropics/claude-plugins-official"
        );
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    }

    fn directory_cache(profile: &Path, version: u64) {
        write(
            &profile.join("plugins/plugin-directory-cache-v2.json"),
            &json!({
                "schemaVersion": version,
                "entryNames": {"p1": "superpowers", "p2": "frontend-design", "p3": "web-only"},
                "listings": [
                    {"id": "p1", "name": "Superpowers", "summary": "Skills", "release": {"version": "6.4.2"},
                     "surfaces": ["claude_code"],
                     "source": {"type": "directory", "repository": {"url": "https://github.com/obra/superpowers",
                                "reviewed_commit": "8ca22db"}}},
                    {"id": "p2", "name": "frontend-design", "summary": "UI",
                     "surfaces": ["claude_code", "cowork"],
                     "source": {"type": "directory", "repository": {"url": "https://github.com/anthropics/claude-plugins-official",
                                "path": "plugins/frontend-design", "reviewed_commit": "e0a"}}},
                    {"id": "p3", "name": "Web only", "summary": "x", "surfaces": ["claude_ai"]},
                    {"id": "unnamed", "name": "No entry name", "surfaces": ["claude_code"]}
                ]
            }),
        );
    }

    #[test]
    fn ac5_02_the_plugin_directory_is_read() {
        let f = Fx::new();
        directory_cache(&f.claude(), 2);
        let c = f.build();
        let sp = find(&c, "superpowers@anthropic-plugin-directory");
        assert_eq!(sp.display_name.as_deref(), Some("Superpowers"));
        assert_eq!(sp.description, "Skills");
        assert_eq!(sp.version.as_deref(), Some("6.4.2"));
        assert_eq!(sp.commit.as_deref(), Some("8ca22db"));
        let fd = find(&c, "frontend-design@anthropic-plugin-directory");
        assert_eq!(fd.subpath.as_deref(), Some("plugins/frontend-design"));
        assert_eq!(
            c.entries.len(),
            2,
            "claude_code listings with an entry name only"
        );
        let m = &c.marketplaces[0];
        assert_eq!(
            (m.name.as_str(), m.source.as_str(), m.plugins),
            (PLUGIN_DIRECTORY, "built in", 2)
        );
    }

    #[test]
    fn ec5_02_a_directory_cache_with_another_schema_is_skipped_with_a_warning() {
        let f = Fx::new();
        directory_cache(&f.claude(), 3);
        let c = f.build();
        assert!(c.entries.is_empty());
        assert!(
            c.warnings[0].contains("unknown schemaVersion"),
            "{:?}",
            c.warnings
        );
        // Missing entirely: silent.
        assert!(Fx::new().build().warnings.is_empty());
    }

    fn codex_market(f: &Fx, profile: &Path, name: &str, plugins: Value, git: bool) {
        let root = if git {
            profile.join(".tmp/marketplaces").join(name)
        } else {
            f.root.join("codex-markets").join(name)
        };
        write(
            &root.join(".agents/plugins/marketplace.json"),
            &json!({"name": name, "plugins": plugins}),
        );
        let (kind, source) = if git {
            ("git", "https://github.com/o/codex-market".to_string())
        } else {
            ("local", root.display().to_string())
        };
        let config = profile.join("config.toml");
        let mut text = fs::read_to_string(&config).unwrap_or_default();
        text.push_str(&format!(
            "[marketplaces.{name}]\nsource_type = \"{kind}\"\nsource = \"{source}\"\n"
        ));
        fs::write(config, text).unwrap();
    }

    #[test]
    fn ac5_03_codex_marketplaces_on_disk_are_read() {
        let f = Fx::new();
        codex_market(
            &f,
            &f.codex(),
            "local-m",
            json!([{"name": "tool", "description": "d", "source": {"source": "url", "url": "./tool"}}]),
            false,
        );
        codex_market(
            &f,
            &f.codex(),
            "git-m",
            json!([{"name": "other", "source": {"source": "url", "url": "https://github.com/a/b"}}]),
            true,
        );
        let c = f.build();
        let tool = find(&c, "tool@local-m");
        assert_eq!(tool.tool, "codex");
        assert_eq!(tool.subpath.as_deref(), Some("tool"));
        assert_eq!(
            find(&c, "other@git-m").repo.as_deref(),
            Some("https://github.com/a/b")
        );
        assert_eq!(c.marketplaces.len(), 2);
    }

    #[test]
    fn ac5_04_ec5_23_colctl_sources_are_entries() {
        let mut f = Fx::new();
        let clone = f.root.join("source/plugins/superpowers");
        write(
            &clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "superpowers-dev", "plugins": [{"name": "superpowers", "source": "./"}]}),
        );
        write(
            &clone.join(".claude-plugin/plugin.json"),
            &json!({"name": "superpowers", "description": "Core skills", "version": "7.0.2"}),
        );
        f.config.sources.push(config::Source {
            name: "superpowers".into(),
            repo: "github:obra/superpowers".into(),
            rev: "a".repeat(40),
            plugin: "superpowers".into(),
            replaces: vec![],
            path: None,
            convert: Vec::new(),
        });
        // A declared source that isn't cloned yet contributes nothing.
        f.config.sources.push(config::Source {
            name: "later".into(),
            repo: "github:o/later".into(),
            rev: "b".repeat(40),
            plugin: "later".into(),
            replaces: vec![],
            path: None,
            convert: Vec::new(),
        });
        let c = f.build();
        let e = find(&c, "superpowers@superpowers-dev");
        assert_eq!(e.catalog, COLCTL);
        assert_eq!(e.source_plugin.as_deref(), Some("superpowers"));
        assert_eq!(
            e.repo.as_deref(),
            Some("https://github.com/obra/superpowers")
        );
        assert_eq!(e.description, "Core skills");
        // EC5-23: the version at the pin, from the clone itself.
        assert_eq!(e.version.as_deref(), Some("7.0.2"));
        assert_eq!(e.commit, Some("a".repeat(40)));
        assert_eq!(c.entries.len(), 1);
    }

    #[test]
    fn ac5_05_ec5_06_installed_markers_and_one_entry_per_id() {
        let mut f = Fx::new();
        let kilo = f.root.join("kilo");
        fs::create_dir_all(&kilo).unwrap();
        f.config
            .harnesses
            .push(harness("kilo-claude", Kind::Claude, kilo.clone()));
        let plugins = json!([{"name": "p", "source": "./p"}]);
        f.claude_market(&f.claude(), "m", "o/m", plugins.clone());
        f.claude_market(&kilo, "m", "o/m", plugins);
        for (profile, scope) in [(f.claude(), "user"), (kilo.clone(), "project")] {
            write(
                &profile.join("plugins/installed_plugins.json"),
                &json!({"version": 2, "plugins": {"p@m": [{"scope": scope}]}}),
            );
        }
        fs::write(
            f.codex().join("config.toml"),
            "[plugins.\"p@m\"]\nenabled = true\n",
        )
        .unwrap();
        let c = f.build();
        assert_eq!(c.entries.len(), 1);
        let e = find(&c, "p@m");
        assert_eq!(
            e.installed.iter().collect::<Vec<_>>(),
            vec!["claude"],
            "user scope only, and a Codex install of the same id is another tool's"
        );
        assert_eq!(
            c.marketplaces[0].profiles.iter().collect::<Vec<_>>(),
            vec!["claude", "kilo-claude"]
        );
    }

    #[test]
    fn ac5_06_ec5_03_one_bad_catalog_does_not_stop_the_rest() {
        let f = Fx::new();
        f.claude_market(
            &f.claude(),
            "good",
            "o/good",
            json!([{"name": "ok", "source": "./ok"}]),
        );
        let bad = f.claude_market(&f.claude(), "bad", "o/bad", json!([]));
        fs::write(bad.join(".claude-plugin/marketplace.json"), "{ nope").unwrap();
        let gone = f.claude_market(&f.claude(), "gone", "o/gone", json!([]));
        fs::remove_dir_all(&gone).unwrap();
        let c = f.build();
        assert_eq!(c.entries.len(), 1);
        assert!(
            c.warnings.iter().any(|w| w.contains("marketplace.json")),
            "{:?}",
            c.warnings
        );
        assert!(
            c.warnings
                .iter()
                .any(|w| w.contains("marketplace gone:") && w.contains("is missing")),
            "{:?}",
            c.warnings
        );
        // An unreadable Codex config is a warning too.
        fs::write(f.codex().join("config.toml"), "not = [toml").unwrap();
        assert!(f.build().warnings.iter().any(|w| w.starts_with("codex:")));
    }

    #[test]
    fn ac5_07_git_subdir_sources_resolve_in_origin_too() {
        let f = Fx::new();
        f.claude_market(
            &f.claude(),
            "official",
            "anthropics/official",
            json!([{"name": "api", "source": {"source": "git-subdir",
                     "url": "https://github.com/42Crunch-AI/claude-plugins.git",
                     "path": "plugins/api-security-testing", "sha": "faf5305"}}]),
        );
        write(
            &f.claude().join("plugins/installed_plugins.json"),
            &json!({"version": 2, "plugins": {"api@official": [{"scope": "user"}]}}),
        );
        let found = origin::claude(&f.claude(), "api@official", &|_| None).unwrap();
        assert_eq!(
            found.repo,
            "https://github.com/42Crunch-AI/claude-plugins.git"
        );
        assert_eq!(
            found.subpath.as_deref(),
            Some("plugins/api-security-testing")
        );
        assert_eq!(found.rev.as_deref(), Some("faf5305"));
        let c = f.build();
        assert_eq!(find(&c, "api@official").commit.as_deref(), Some("faf5305"));
    }

    #[test]
    fn ec5_04_nameless_and_repeated_entries() {
        let f = Fx::new();
        f.claude_market(
            &f.claude(),
            "m",
            "o/m",
            json!([{"description": "no name"}, {"name": "twice", "description": "first"},
                   {"name": "twice", "description": "second"}, {"name": ""}]),
        );
        let c = f.build();
        assert_eq!(c.entries.len(), 1);
        assert_eq!(find(&c, "twice@m").description, "first");
    }

    #[test]
    fn ec5_05_missing_harness_folders_are_skipped_and_excludes_do_not_hide() {
        let mut f = Fx::new();
        f.config
            .harnesses
            .push(harness("gone", Kind::Claude, f.root.join("nope")));
        f.config
            .harnesses
            .push(harness("agents", Kind::Agents, f.root.clone()));
        f.claude_market(
            &f.claude(),
            "m",
            "o/m",
            json!([{"name": "p", "source": "./p"}]),
        );
        f.config.harnesses[0].exclude.insert("plugins/p@m".into());
        let c = f.build();
        assert_eq!(c.entries.len(), 1);
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    }

    #[test]
    fn ac5_04_a_source_wins_over_its_own_local_marketplace() {
        let mut f = Fx::new();
        let clone = f.root.join("source/plugins/superpowers");
        write(
            &clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "superpowers-dev", "plugins": [{"name": "superpowers", "source": "./"}]}),
        );
        // Claude knows the clone as a local marketplace, as after colctl installed it.
        let known = f.claude().join("plugins/known_marketplaces.json");
        write(
            &known,
            &json!({"superpowers-dev": {"source": {"source": "directory", "path": clone.display().to_string()},
                                        "installLocation": clone.display().to_string()}}),
        );
        f.config.sources.push(config::Source {
            name: "superpowers".into(),
            repo: "https://github.com/obra/superpowers.git".into(),
            rev: "a".repeat(40),
            plugin: "superpowers".into(),
            replaces: vec![],
            path: None,
            convert: Vec::new(),
        });
        let c = f.build();
        let e = find(&c, "superpowers@superpowers-dev");
        assert_eq!(e.catalog, COLCTL);
        assert_eq!(
            e.repo.as_deref(),
            Some("https://github.com/obra/superpowers.git")
        );
        assert!(
            c.marketplaces
                .iter()
                .any(|m| m.name == "superpowers-dev" && m.managed)
        );
    }

    #[test]
    fn managed_marketplaces_are_marked() {
        let mut f = Fx::new();
        f.claude_market(
            &f.claude(),
            "m",
            "o/m",
            json!([{"name": "p", "source": "./p"}]),
        );
        f.config
            .marketplaces
            .insert("m".into(), "github:o/m".into());
        assert!(f.build().marketplaces[0].managed);
    }
}
