//! `config.toml`: the harness registry, marketplaces and plugins (interfaces.md § config.toml).
//!
//! Parsing is strict: unknown keys and invalid values are errors that name the line. Edits go
//! through `toml_edit` so the user's formatting and comments survive.

use crate::error::CliError;
use crate::fsx;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml::Spanned;
use toml_edit::{DocumentMut, Item, value};

/// The config file's name inside the source.
pub const CONFIG_FILE: &str = "config.toml";
/// Anthropic's plugin directory: a plugin source that needs no `[marketplaces]` entry.
pub const PLUGIN_DIRECTORY: &str = "anthropic-plugin-directory";

/// A harness kind; only Claude Code profiles exist in cycle 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Claude,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Claude => "claude",
        }
    }
}

/// One registered harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Harness {
    pub name: String,
    pub kind: Kind,
    /// Absolute path, with `~` expanded.
    pub path: PathBuf,
    /// Items colctl must stay out of: `CLAUDE.md`, `skills/<name>`, `agents/<name>.md`, `plugins/<id>`.
    pub exclude: BTreeSet<String>,
}

impl Harness {
    pub fn excludes(&self, item: &str) -> bool {
        self.exclude.contains(item)
    }
}

/// A validated config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub harnesses: Vec<Harness>,
    /// Marketplace name → source (`github:owner/repo`, a URL, or an absolute path).
    pub marketplaces: BTreeMap<String, String>,
    /// Enabled plugin ids, `name@marketplace`, in file order.
    pub plugins: Vec<String>,
}

impl Config {
    pub fn harness(&self, name: &str) -> Option<&Harness> {
        self.harnesses.iter().find(|h| h.name == name)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    version: Spanned<i64>,
    #[serde(default)]
    harness: Vec<RawHarness>,
    #[serde(default)]
    marketplaces: BTreeMap<String, Spanned<String>>,
    #[serde(default)]
    plugins: Option<RawPlugins>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHarness {
    name: Spanned<String>,
    kind: Spanned<String>,
    path: Spanned<String>,
    #[serde(default)]
    exclude: Vec<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlugins {
    #[serde(default)]
    enabled: Vec<Spanned<String>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// True for a valid harness name: `[a-z0-9-]+`.
pub fn valid_harness_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// True for a valid skill, agent or marketplace name: `^[A-Za-z0-9][A-Za-z0-9._-]*$`.
pub fn valid_item_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Splits `name@marketplace`; both halves must be valid item names.
pub fn split_plugin_id(id: &str) -> Option<(&str, &str)> {
    let (name, market) = id.rsplit_once('@')?;
    (valid_item_name(name) && valid_item_name(market)).then_some((name, market))
}

/// Checks a marketplace source: `github:owner/repo`, an `http(s)://` URL, or an absolute path.
pub fn valid_marketplace_source(source: &str) -> bool {
    if let Some(repo) = source.strip_prefix("github:") {
        let parts: Vec<&str> = repo.split('/').collect();
        return parts.len() == 2 && parts.iter().all(|p| valid_item_name(p));
    }
    source.starts_with("https://") || source.starts_with("http://") || source.starts_with('/')
}

fn valid_exclude(item: &str) -> bool {
    if item == "CLAUDE.md" {
        return true;
    }
    if let Some(name) = item.strip_prefix("skills/") {
        return valid_item_name(name);
    }
    if let Some(file) = item.strip_prefix("agents/") {
        return file.strip_suffix(".md").is_some_and(valid_item_name);
    }
    item.strip_prefix("plugins/")
        .is_some_and(|id| split_plugin_id(id).is_some())
}

/// Expands a config path: `~` or `~/…` against `home`; other paths must be absolute (EC-09).
pub fn expand_path(raw: &str, home: &Path) -> Option<PathBuf> {
    if raw == "~" {
        return Some(home.to_path_buf());
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return Some(home.join(rest));
    }
    let path = PathBuf::from(raw);
    path.is_absolute().then_some(path)
}

/// Parses and validates config text. Errors are `config.toml: line N: …` (AC-02).
pub fn parse(text: &str, home: &Path) -> Result<Config, String> {
    let raw: Raw = toml::from_str(text).map_err(|err| {
        let line = err
            .span()
            .map(|s| format!("line {}: ", line_of(text, s.start)))
            .unwrap_or_default();
        format!("{CONFIG_FILE}: {line}{}", err.message())
    })?;
    let at = |span: std::ops::Range<usize>, msg: String| {
        format!("{CONFIG_FILE}: line {}: {msg}", line_of(text, span.start))
    };

    if *raw.version.get_ref() != 1 {
        return Err(at(raw.version.span(), "version must be 1".into()));
    }

    let mut config = Config::default();
    let mut canonical: Vec<(PathBuf, String)> = Vec::new();
    for h in &raw.harness {
        let name = h.name.get_ref();
        if !valid_harness_name(name) {
            return Err(at(
                h.name.span(),
                format!("harness name \"{name}\" must match [a-z0-9-]+"),
            ));
        }
        if config.harness(name).is_some() {
            return Err(at(
                h.name.span(),
                format!("duplicate harness name \"{name}\""),
            ));
        }
        let kind = match h.kind.get_ref().as_str() {
            "claude" => Kind::Claude,
            other => {
                return Err(at(
                    h.kind.span(),
                    format!("unknown harness kind \"{other}\" (supported: \"claude\")"),
                ));
            }
        };
        let path = expand_path(h.path.get_ref(), home).ok_or_else(|| {
            at(
                h.path.span(),
                format!(
                    "harness path \"{}\" must be absolute or start with ~/",
                    h.path.get_ref()
                ),
            )
        })?;
        let canon = fsx::canonical_or_normal(&path);
        if let Some((_, other)) = canonical.iter().find(|(c, _)| *c == canon) {
            return Err(at(
                h.path.span(),
                format!("harnesses \"{other}\" and \"{name}\" point at the same folder"),
            ));
        }
        canonical.push((canon, name.clone()));
        let mut exclude = BTreeSet::new();
        for item in &h.exclude {
            if !valid_exclude(item.get_ref()) {
                return Err(at(
                    item.span(),
                    format!(
                        "exclude entry \"{}\" must be CLAUDE.md, skills/<name>, agents/<name>.md or plugins/<id>",
                        item.get_ref()
                    ),
                ));
            }
            exclude.insert(item.get_ref().clone());
        }
        config.harnesses.push(Harness {
            name: name.clone(),
            kind,
            path,
            exclude,
        });
    }

    for (name, source) in &raw.marketplaces {
        if !valid_item_name(name) || name == PLUGIN_DIRECTORY {
            return Err(at(
                source.span(),
                format!("invalid marketplace name \"{name}\""),
            ));
        }
        if !valid_marketplace_source(source.get_ref()) {
            return Err(at(
                source.span(),
                format!(
                    "marketplace source \"{}\" must be github:owner/repo, an http(s) URL or an absolute path",
                    source.get_ref()
                ),
            ));
        }
        config
            .marketplaces
            .insert(name.clone(), source.get_ref().clone());
    }

    for id in raw.plugins.iter().flat_map(|p| p.enabled.iter()) {
        let Some((_, market)) = split_plugin_id(id.get_ref()) else {
            return Err(at(
                id.span(),
                format!("plugin id \"{}\" must be name@marketplace", id.get_ref()),
            ));
        };
        if market != PLUGIN_DIRECTORY && !config.marketplaces.contains_key(market) {
            return Err(at(
                id.span(),
                format!(
                    "plugin \"{}\" uses marketplace \"{market}\", which isn't in [marketplaces]",
                    id.get_ref()
                ),
            ));
        }
        if config.plugins.contains(id.get_ref()) {
            return Err(at(
                id.span(),
                format!("duplicate plugin \"{}\"", id.get_ref()),
            ));
        }
        config.plugins.push(id.get_ref().clone());
    }
    Ok(config)
}

/// Reads and validates `<source>/config.toml`. A missing file is EC-01.
pub fn load(source: &Path, home: &Path) -> Result<(Config, String), CliError> {
    let path = source.join(CONFIG_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::usage(format!(
                "no colctl source at {}; run colctl init",
                source.display()
            )));
        }
        Err(err) => {
            return Err(CliError::usage(format!(
                "reading {}: {err}",
                path.display()
            )));
        }
    };
    let config = parse(&text, home).map_err(CliError::usage)?;
    Ok((config, text))
}

/// A fresh config listing `harnesses` as (name, path-as-written).
pub fn initial_text(harnesses: &[(String, String)]) -> String {
    let mut text = String::from(
        "# colctl configuration. Reference: https://github.com/muradmadi-leadtech/colctl/blob/main/docs/reference/config.md\nversion = 1\n",
    );
    for (name, path) in harnesses {
        text.push_str(&harness_block(name, path));
    }
    text.push_str("\n[marketplaces]\n# name = \"github:owner/repo\"\n\n[plugins]\nenabled = []\n");
    text
}

fn harness_block(name: &str, path: &str) -> String {
    format!(
        "\n[[harness]]\nname = {}\nkind = \"claude\"\npath = {}\n",
        toml_edit::Value::from(name),
        toml_edit::Value::from(path)
    )
}

fn document(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>()
        .map_err(|e| format!("{CONFIG_FILE}: {e}"))
}

/// Appends a `[[harness]]` table, leaving every existing line unchanged (AC-06).
pub fn append_harness(text: &str, name: &str, path: &str) -> String {
    let mut out = text.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&harness_block(name, path));
    out
}

/// Adds a plugin (and its marketplace, if given and absent) to the config text (AC-22, AC-27).
pub fn add_plugin(
    text: &str,
    id: &str,
    marketplace: Option<(&str, &str)>,
) -> Result<String, String> {
    let mut doc = document(text)?;
    if let Some((name, source)) = marketplace {
        if doc.get("marketplaces").is_none() {
            doc["marketplaces"] = toml_edit::table();
        }
        let table = doc["marketplaces"]
            .as_table_like_mut()
            .ok_or_else(|| format!("{CONFIG_FILE}: [marketplaces] must be a table"))?;
        if table.get(name).is_none() {
            table.insert(name, value(source));
        }
    }
    if doc.get("plugins").is_none() {
        doc["plugins"] = toml_edit::table();
    }
    let plugins = doc["plugins"]
        .as_table_like_mut()
        .ok_or_else(|| format!("{CONFIG_FILE}: [plugins] must be a table"))?;
    if plugins.get("enabled").is_none() {
        plugins.insert("enabled", value(toml_edit::Array::new()));
    }
    let enabled = plugins
        .get_mut("enabled")
        .and_then(Item::as_array_mut)
        .ok_or_else(|| format!("{CONFIG_FILE}: plugins.enabled must be an array"))?;
    enabled.push(id);
    Ok(doc.to_string())
}

/// Removes a plugin from `[plugins].enabled` (AC-23). Its marketplace stays.
pub fn remove_plugin(text: &str, id: &str) -> Result<String, String> {
    let mut doc = document(text)?;
    if let Some(enabled) = doc
        .get_mut("plugins")
        .and_then(|p| p.as_table_like_mut())
        .and_then(|p| p.get_mut("enabled"))
        .and_then(Item::as_array_mut)
    {
        enabled.retain(|v| v.as_str() != Some(id));
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/me";

    fn ok(text: &str) -> Config {
        parse(text, Path::new(HOME)).unwrap()
    }

    fn err(text: &str) -> String {
        parse(text, Path::new(HOME)).unwrap_err()
    }

    const FULL: &str = r#"version = 1

[[harness]]
name = "claude"
kind = "claude"
path = "~/.claude"

[[harness]]
name = "kilo-claude"
kind = "claude"
path = "/opt/kilo/profiles/claude"
exclude = ["skills/foo", "CLAUDE.md", "agents/a.md", "plugins/x@sadr"]

[marketplaces]
sadr = "github:muradmadi/sadr"

[plugins]
enabled = ["sadr@sadr", "superpowers@anthropic-plugin-directory"]
"#;

    #[test]
    fn ac_01_valid_config_loads_with_tilde_expanded() {
        let c = ok(FULL);
        assert_eq!(c.harnesses.len(), 2);
        assert_eq!(c.harnesses[0].path, PathBuf::from("/home/me/.claude"));
        assert_eq!(c.harnesses[0].kind, Kind::Claude);
        assert_eq!(
            c.harnesses[1].path,
            PathBuf::from("/opt/kilo/profiles/claude")
        );
        assert!(c.harnesses[1].excludes("skills/foo"));
        assert!(!c.harnesses[0].excludes("skills/foo"));
        assert_eq!(c.marketplaces["sadr"], "github:muradmadi/sadr");
        assert_eq!(
            c.plugins,
            vec!["sadr@sadr", "superpowers@anthropic-plugin-directory"]
        );
        assert_eq!(c.harness("claude").unwrap().name, "claude");
        assert!(c.harness("nope").is_none());
        assert_eq!(Kind::Claude.as_str(), "claude");
    }

    #[test]
    fn ac_02_unknown_key_names_the_line() {
        let e = err("version = 1\nbogus = true\n");
        assert!(e.starts_with("config.toml: line 2:"), "{e}");
        assert!(e.contains("bogus"), "{e}");
    }

    #[test]
    fn ac_02_unknown_kind_duplicate_name_and_bad_plugin_id() {
        let e = err("version = 1\n[[harness]]\nname = \"a\"\nkind = \"codex\"\npath = \"/a\"\n");
        assert!(
            e.contains("line 4") && e.contains("unknown harness kind \"codex\""),
            "{e}"
        );

        let dup = "version = 1\n[[harness]]\nname = \"a\"\nkind = \"claude\"\npath = \"/a\"\n[[harness]]\nname = \"a\"\nkind = \"claude\"\npath = \"/b\"\n";
        let e = err(dup);
        assert!(
            e.contains("line 7") && e.contains("duplicate harness name"),
            "{e}"
        );

        let e = err("version = 1\n[plugins]\nenabled = [\"nomarket\"]\n");
        assert!(
            e.contains("line 3") && e.contains("name@marketplace"),
            "{e}"
        );

        let e = err("version = 1\n[plugins]\nenabled = [\"p@unknown\"]\n");
        assert!(e.contains("isn't in [marketplaces]"), "{e}");

        let e = err(
            "version = 1\n[plugins]\nenabled = [\"p@anthropic-plugin-directory\", \"p@anthropic-plugin-directory\"]\n",
        );
        assert!(e.contains("duplicate plugin"), "{e}");
    }

    #[test]
    fn ac_02_other_invalid_values() {
        assert!(err("version = 2\n").contains("version must be 1"));
        assert!(err("").contains("version"));
        assert!(
            err("version = 1\n[[harness]]\nname = \"Bad\"\nkind = \"claude\"\npath = \"/a\"\n")
                .contains("[a-z0-9-]+")
        );
        assert!(err("version = 1\n[[harness]]\nname = \"a\"\nkind = \"claude\"\npath = \"/a\"\nexclude = [\"bin/x\"]\n").contains("exclude entry"));
        assert!(
            err("version = 1\n[marketplaces]\nm = \"relative/path\"\n")
                .contains("marketplace source")
        );
        assert!(
            err("version = 1\n[marketplaces]\n\"-bad\" = \"/x\"\n")
                .contains("invalid marketplace name")
        );
        assert!(
            err("version = 1\n[marketplaces]\nanthropic-plugin-directory = \"/x\"\n")
                .contains("invalid marketplace name")
        );
        assert!(err("version = [").contains("config.toml"));
    }

    #[test]
    fn ec_09_relative_harness_path_is_rejected() {
        let e =
            err("version = 1\n[[harness]]\nname = \"a\"\nkind = \"claude\"\npath = \"rel/dir\"\n");
        assert!(
            e.contains("line 5") && e.contains("must be absolute"),
            "{e}"
        );
    }

    #[test]
    fn ec_10_two_harnesses_with_the_same_folder() {
        let text = "version = 1\n[[harness]]\nname = \"a\"\nkind = \"claude\"\npath = \"~/.claude\"\n[[harness]]\nname = \"b\"\nkind = \"claude\"\npath = \"/home/me/x/../.claude\"\n";
        let e = err(text);
        assert!(
            e.contains("\"a\" and \"b\" point at the same folder"),
            "{e}"
        );
    }

    #[test]
    fn names_ids_and_sources() {
        assert!(valid_item_name("foo-bar_1.2"));
        assert!(!valid_item_name(".hidden"));
        assert!(!valid_item_name(".."));
        assert!(!valid_item_name("a/b"));
        assert!(!valid_item_name(""));
        assert_eq!(split_plugin_id("a@b"), Some(("a", "b")));
        assert_eq!(split_plugin_id("a@"), None);
        assert!(valid_marketplace_source("github:o/r"));
        assert!(!valid_marketplace_source("github:o"));
        assert!(valid_marketplace_source("https://x"));
        assert!(valid_marketplace_source("/abs"));
        assert!(!valid_marketplace_source("rel"));
        assert_eq!(expand_path("~", Path::new(HOME)), Some(PathBuf::from(HOME)));
        assert!(!valid_harness_name(""));
    }

    #[test]
    fn ec_01_missing_config_says_run_init() {
        let t = tempfile::tempdir().unwrap();
        let e = load(t.path(), Path::new(HOME)).unwrap_err();
        assert_eq!(e.code, 2);
        assert!(e.message.contains("run colctl init"));
        std::fs::create_dir(t.path().join(CONFIG_FILE)).unwrap();
        assert!(
            load(t.path(), Path::new(HOME))
                .unwrap_err()
                .message
                .contains("reading")
        );
    }

    #[test]
    fn ec_02_no_harnesses_is_valid() {
        let c = ok(&initial_text(&[]));
        assert!(c.harnesses.is_empty() && c.plugins.is_empty());
    }

    #[test]
    fn initial_text_round_trips() {
        let text = initial_text(&[("claude".into(), "~/.claude".into())]);
        let c = ok(&text);
        assert_eq!(c.harnesses[0].path, PathBuf::from("/home/me/.claude"));
    }

    #[test]
    fn ac_06_append_harness_keeps_existing_lines() {
        let original = "# my comment\nversion = 1 # trailing\n\n[plugins]\nenabled = []";
        let out = append_harness(original, "kilo-claude", "~/kilo");
        assert!(
            out.starts_with("# my comment\nversion = 1 # trailing\n\n[plugins]\nenabled = []\n")
        );
        let c = ok(&out);
        assert_eq!(c.harnesses[0].name, "kilo-claude");
    }

    #[test]
    fn ac_22_add_and_remove_plugin_keep_comments() {
        let original = "# keep me\nversion = 1\n\n[marketplaces]\n# a note\n\n[plugins]\nenabled = [] # trailing\n";
        let out = add_plugin(
            original,
            "sadr@sadr",
            Some(("sadr", "github:muradmadi/sadr")),
        )
        .unwrap();
        assert!(
            out.contains("# keep me") && out.contains("# a note") && out.contains("# trailing"),
            "{out}"
        );
        let c = ok(&out);
        assert_eq!(c.plugins, vec!["sadr@sadr"]);
        assert_eq!(c.marketplaces["sadr"], "github:muradmadi/sadr");

        // An existing marketplace is left as it is.
        let again = add_plugin(&out, "other@sadr", Some(("sadr", "github:x/y"))).unwrap();
        assert_eq!(ok(&again).marketplaces["sadr"], "github:muradmadi/sadr");

        let removed = remove_plugin(&again, "sadr@sadr").unwrap();
        assert_eq!(ok(&removed).plugins, vec!["other@sadr"]);
        assert!(removed.contains("# keep me"));
    }

    #[test]
    fn add_plugin_creates_missing_tables() {
        let out = add_plugin("version = 1\n", "p@m", Some(("m", "/abs"))).unwrap();
        let c = ok(&out);
        assert_eq!(c.plugins, vec!["p@m"]);
        let out = add_plugin(
            "version = 1\n[plugins]\n",
            "p@anthropic-plugin-directory",
            None,
        )
        .unwrap();
        assert_eq!(ok(&out).plugins, vec!["p@anthropic-plugin-directory"]);
        assert!(add_plugin("version = [", "p@m", None).is_err());
        assert!(add_plugin("version = 1\nplugins = 3\n", "p@m", None).is_err());
        assert!(add_plugin("version = 1\nmarketplaces = 3\n", "p@m", Some(("m", "/x"))).is_err());
        assert!(add_plugin("version = 1\n[plugins]\nenabled = 3\n", "p@m", None).is_err());
        assert_eq!(
            remove_plugin("version = 1\n", "p@m").unwrap(),
            "version = 1\n"
        );
    }
}
