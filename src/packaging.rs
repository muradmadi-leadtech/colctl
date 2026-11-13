//! Which tools a source plugin's clone has packaging for, and the marketplace each tool installs it
//! from (AC3-07…09). When the repo has only plugin manifests, colctl writes a one-plugin marketplace
//! manifest in `~/.colctl/plugins/.manifests/<name>/`, never inside the clone.

use crate::fsx;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Where one tool installs a source plugin from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The folder given to `<tool> plugin marketplace add`.
    pub marketplace_dir: PathBuf,
    /// The marketplace's name (from the repo's manifest, or `colctl-<source>`).
    pub marketplace: String,
    /// `<plugin>@<marketplace>`.
    pub id: String,
}

/// One tool's view of a clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPackaging {
    Supported(Target),
    /// The repo has nothing for this tool.
    Unsupported(String),
    /// It has something, but it can't be used.
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packaging {
    pub claude: ToolPackaging,
    pub codex: ToolPackaging,
}

impl Packaging {
    /// True when at least one tool can install it (EC3-04).
    pub fn any_supported(&self) -> bool {
        [&self.claude, &self.codex]
            .iter()
            .any(|t| matches!(t, ToolPackaging::Supported(_)))
    }
}

/// One tool's manifest locations and how colctl writes its marketplace manifest.
struct Tool {
    label: &'static str,
    marketplace: &'static str,
    plugin: &'static str,
    entry: fn(&str, &str) -> Value,
}

const CLAUDE: Tool = Tool {
    label: "Claude",
    marketplace: ".claude-plugin/marketplace.json",
    plugin: ".claude-plugin/plugin.json",
    entry: |name, link| json!({"name": name, "source": format!("./{link}")}),
};

const CODEX: Tool = Tool {
    label: "Codex",
    marketplace: ".agents/plugins/marketplace.json",
    plugin: ".codex-plugin/plugin.json",
    entry: |name, link| {
        json!({
            "name": name,
            "source": {"source": "url", "url": format!("./{link}")},
            "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
        })
    },
};

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} doesn't parse: {e}", path.display()))
}

/// What a repo offers one tool for a plugin, before anything is written.
enum Plan {
    /// The repo's own marketplace lists the plugin inside the repo: use it as is.
    Repo(Target),
    /// colctl writes a one-plugin marketplace pointing at this folder of the clone.
    Wrap {
        folder: PathBuf,
        plugin_name: String,
    },
    Done(ToolPackaging),
}

/// True when a marketplace entry's `source` is a folder inside the marketplace's own repo.
fn is_local(source: &Value) -> bool {
    match source {
        Value::String(s) => !s.contains("://"),
        Value::Object(o) => {
            let field = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or_default();
            match field("source") {
                "url" | "git" => field("url").starts_with('.'),
                "local" | "path" | "directory" => true,
                _ => false,
            }
        }
        _ => false,
    }
}

/// The folder holding `tool`'s plugin manifest, from the first candidate that has one.
fn plugin_folder(tool: &Tool, clone: &Path, candidates: &[Option<&str>]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|c| c.map_or(clone.to_path_buf(), |sub| clone.join(sub)))
        .find(|folder| folder.join(tool.plugin).is_file())
}

fn wrap_plan(tool: &Tool, folder: PathBuf, plugin: &str) -> Plan {
    match read_json(&folder.join(tool.plugin)) {
        Ok(v) => Plan::Wrap {
            plugin_name: v
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(plugin)
                .to_string(),
            folder,
        },
        Err(e) => Plan::Done(ToolPackaging::Invalid(e)),
    }
}

fn plan(tool: &Tool, clone: &Path, plugin: &str, subpath: Option<&str>) -> Plan {
    let manifest = clone.join(tool.marketplace);
    if manifest.is_file() {
        let value = match read_json(&manifest) {
            Ok(v) => v,
            Err(e) => return Plan::Done(ToolPackaging::Invalid(e)),
        };
        let Some(market) = value.get("name").and_then(Value::as_str) else {
            return Plan::Done(ToolPackaging::Invalid(format!(
                "{} has no \"name\"",
                manifest.display()
            )));
        };
        let entry = value
            .get("plugins")
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|p| p.get("name").and_then(Value::as_str) == Some(plugin))
            });
        let Some(entry) = entry else {
            return Plan::Done(ToolPackaging::Invalid(format!(
                "no plugin {plugin} in {}",
                manifest.display()
            )));
        };
        let source = entry.get("source").cloned().unwrap_or(Value::Null);
        if is_local(&source) {
            return Plan::Repo(Target {
                marketplace_dir: clone.to_path_buf(),
                marketplace: market.to_string(),
                id: format!("{plugin}@{market}"),
            });
        }
        // AC6-31: the repo's manifest installs the plugin from elsewhere (GitHub, a URL), which would
        // bypass the pinned clone. Install from the plugin's folder in the clone instead.
        let path = source.get("path").and_then(Value::as_str);
        return match plugin_folder(tool, clone, &[path, subpath, None]) {
            Some(folder) => wrap_plan(tool, folder, plugin),
            None => Plan::Done(ToolPackaging::Unsupported(format!(
                "the repo's {} manifest installs {plugin} from outside the repo",
                tool.label
            ))),
        };
    }
    match plugin_folder(tool, clone, &[subpath]) {
        Some(folder) => wrap_plan(tool, folder, plugin),
        None => Plan::Done(ToolPackaging::Unsupported(format!(
            "the repo has no {} packaging",
            tool.label
        ))),
    }
}

/// Writes (or, without `write`, only describes) colctl's one-plugin marketplace for `tool`, whose plugin
/// entry points at `./<link>`, a symlink to `folder`.
fn wrap(
    write: bool,
    tool: &Tool,
    manifests: &Path,
    name: &str,
    folder: &Path,
    plugin_name: &str,
    link: &str,
) -> ToolPackaging {
    let market = format!("colctl-{name}");
    let mut body = json!({"name": market, "plugins": [(tool.entry)(plugin_name, link)]});
    if tool.label == "Claude" {
        body["owner"] = json!({"name": "colctl"});
    }
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&body).unwrap_or_default()
    );
    let path = manifests.join(tool.marketplace);
    let written = if !write || fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        Ok(())
    } else {
        fsx::atomic_write(&path, text.as_bytes())
    };
    let linked = match fs::read_link(manifests.join(link)) {
        _ if !write => Ok(()),
        Ok(target) if target == folder => Ok(()),
        _ => fsx::atomic_symlink(folder, &manifests.join(link)),
    };
    if let Err(e) = written.and(linked) {
        return ToolPackaging::Invalid(format!("writing {}: {e:#}", manifests.display()));
    }
    ToolPackaging::Supported(Target {
        marketplace_dir: manifests.to_path_buf(),
        marketplace: market,
        id: format!("{plugin_name}@colctl-{name}"),
    })
}

fn finish(
    write: bool,
    tool: &Tool,
    manifests: &Path,
    name: &str,
    plan: Plan,
    link: &str,
) -> ToolPackaging {
    match plan {
        Plan::Repo(target) => ToolPackaging::Supported(target),
        Plan::Wrap {
            folder,
            plugin_name,
        } => wrap(write, tool, manifests, name, &folder, &plugin_name, link),
        Plan::Done(p) => p,
    }
}

/// Inspects `clone` for source `name`'s plugin `plugin`, writing manifests into `manifests` if needed.
/// `subpath` is the plugin's folder inside the repo, when it isn't the root (EC3-11).
pub fn inspect(
    clone: &Path,
    manifests: &Path,
    name: &str,
    plugin: &str,
    subpath: Option<&str>,
) -> Packaging {
    inspect_with(true, clone, manifests, name, plugin, subpath)
}

/// [`inspect`], optionally without writing manifests (for `status`, which never writes).
pub fn inspect_with(
    write: bool,
    clone: &Path,
    manifests: &Path,
    name: &str,
    plugin: &str,
    subpath: Option<&str>,
) -> Packaging {
    let claude = plan(&CLAUDE, clone, plugin, subpath);
    let codex = plan(&CODEX, clone, plugin, subpath);
    // Both tools share one wrapper folder; Codex gets its own link when its plugin lives elsewhere
    // (EC6-21: e.g. plugins/x/claude and plugins/x/codex).
    let codex_link = match (&claude, &codex) {
        (Plan::Wrap { folder: a, .. }, Plan::Wrap { folder: b, .. }) if a != b => "plugin-codex",
        _ => "plugin",
    };
    Packaging {
        claude: finish(write, &CLAUDE, manifests, name, claude, "plugin"),
        codex: finish(write, &CODEX, manifests, name, codex, codex_link),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _t: tempfile::TempDir,
        clone: PathBuf,
        manifests: PathBuf,
    }

    fn fixture() -> Fixture {
        let t = tempfile::tempdir().unwrap();
        let clone = t.path().join("plugins/sp");
        let manifests = t.path().join("plugins/.manifests/sp");
        fs::create_dir_all(&clone).unwrap();
        Fixture {
            _t: t,
            clone,
            manifests,
        }
    }

    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value.to_string()).unwrap();
    }

    fn supported(p: &ToolPackaging) -> &Target {
        match p {
            ToolPackaging::Supported(t) => t,
            other => panic!("{other:?}"),
        }
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    /// A databricks-style repo: per-tool plugin folders, and marketplace manifests whose entries send
    /// both tools to GitHub.
    fn remote_manifests(f: &Fixture) {
        write(
            &f.clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "dbx-skills", "plugins": [{"name": "dbx", "source": {"source": "git-subdir",
                "url": "databricks/dbx", "path": "plugins/dbx/claude", "ref": "main"}}]}),
        );
        write(
            &f.clone.join(".agents/plugins/marketplace.json"),
            &json!({"name": "dbx-skills", "plugins": [{"name": "dbx", "source": {"source": "git-subdir",
                "url": "https://github.com/databricks/dbx.git", "path": "plugins/dbx/codex", "ref": "main"}}]}),
        );
        write(
            &f.clone
                .join("plugins/dbx/claude/.claude-plugin/plugin.json"),
            &json!({"name": "dbx"}),
        );
        write(
            &f.clone.join("plugins/dbx/codex/.codex-plugin/plugin.json"),
            &json!({"name": "dbx"}),
        );
    }

    #[test]
    fn ac6_31_a_manifest_pointing_outside_the_repo_is_wrapped() {
        let f = fixture();
        remote_manifests(&f);
        let p = inspect(
            &f.clone,
            &f.manifests,
            "dbx",
            "dbx",
            Some("plugins/dbx/claude"),
        );
        for tool in [&p.claude, &p.codex] {
            let t = supported(tool);
            assert_eq!(t.marketplace_dir, f.manifests);
            assert_eq!(t.id, "dbx@colctl-dbx");
        }
        // Each tool's wrapper entry points at its own folder of the clone.
        assert_eq!(
            fs::read_link(f.manifests.join("plugin")).unwrap(),
            f.clone.join("plugins/dbx/claude")
        );
        assert_eq!(
            fs::read_link(f.manifests.join("plugin-codex")).unwrap(),
            f.clone.join("plugins/dbx/codex")
        );
        assert_eq!(
            read(&f.manifests.join(".agents/plugins/marketplace.json"))["plugins"][0]["source"]["url"],
            "./plugin-codex"
        );
        assert_eq!(
            read(&f.manifests.join(".claude-plugin/marketplace.json"))["plugins"][0]["source"],
            "./plugin"
        );
        // A remote entry with no matching folder in the clone can't be installed from it.
        let g = fixture();
        write(
            &g.clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "m", "plugins": [{"name": "x", "source": {"source": "github", "repo": "o/elsewhere"}}]}),
        );
        assert!(matches!(
            inspect(&g.clone, &g.manifests, "x", "x", None).claude,
            ToolPackaging::Unsupported(reason) if reason.contains("from outside the repo")
        ));
    }

    #[test]
    fn ec6_21_decided_per_tool() {
        let f = fixture();
        remote_manifests(&f);
        // Claude's manifest points inside the repo; Codex's still points at GitHub.
        write(
            &f.clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "dbx-skills", "plugins": [{"name": "dbx", "source": "./plugins/dbx/claude"}]}),
        );
        let p = inspect(&f.clone, &f.manifests, "dbx", "dbx", None);
        assert_eq!(supported(&p.claude).id, "dbx@dbx-skills");
        assert_eq!(supported(&p.claude).marketplace_dir, f.clone);
        assert_eq!(supported(&p.codex).id, "dbx@colctl-dbx");
        assert_eq!(
            fs::read_link(f.manifests.join("plugin")).unwrap(),
            f.clone.join("plugins/dbx/codex"),
            "one wrapped tool keeps the plain link name"
        );
        assert!(is_local(&json!({"source": "local", "path": "x"})));
        assert!(!is_local(&json!("https://x/y")));
        assert!(!is_local(&json!(3)));
    }

    #[test]
    fn ac3_07_the_repo_manifests_decide_the_ids() {
        let f = fixture();
        write(
            &f.clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "superpowers-dev", "plugins": [{"name": "superpowers", "source": "./"}]}),
        );
        write(
            &f.clone.join(".agents/plugins/marketplace.json"),
            &json!({"name": "superpowers-dev", "plugins": [{"name": "superpowers", "source": {"source": "url", "url": "./"}}]}),
        );
        let p = inspect(&f.clone, &f.manifests, "superpowers", "superpowers", None);
        for tool in [&p.claude, &p.codex] {
            let t = supported(tool);
            assert_eq!(t.marketplace_dir, f.clone);
            assert_eq!(t.marketplace, "superpowers-dev");
            assert_eq!(t.id, "superpowers@superpowers-dev");
        }
        assert!(p.any_supported());
        assert!(
            !f.manifests.exists(),
            "nothing written when the repo has its own manifests"
        );
    }

    #[test]
    fn ac3_08_plugin_only_repos_get_a_colctl_manifest() {
        let f = fixture();
        write(
            &f.clone.join(".claude-plugin/plugin.json"),
            &json!({"name": "tool"}),
        );
        write(
            &f.clone.join(".codex-plugin/plugin.json"),
            &json!({"name": "tool"}),
        );
        let before = crate::fsx::hash_tree(&f.clone).unwrap();
        let p = inspect(&f.clone, &f.manifests, "sp", "tool", None);
        for tool in [&p.claude, &p.codex] {
            let t = supported(tool);
            assert_eq!(t.marketplace_dir, f.manifests);
            assert_eq!(t.marketplace, "colctl-sp");
            assert_eq!(t.id, "tool@colctl-sp");
        }
        assert_eq!(
            crate::fsx::hash_tree(&f.clone).unwrap(),
            before,
            "the clone is never written"
        );
        assert_eq!(fs::read_link(f.manifests.join("plugin")).unwrap(), f.clone);
        let claude = read(&f.manifests.join(".claude-plugin/marketplace.json"));
        assert_eq!(claude["name"], "colctl-sp");
        assert_eq!(
            claude["plugins"][0],
            json!({"name": "tool", "source": "./plugin"})
        );
        let codex = read(&f.manifests.join(".agents/plugins/marketplace.json"));
        assert_eq!(
            codex["plugins"][0]["source"],
            json!({"source": "url", "url": "./plugin"})
        );
        // Running again changes nothing.
        let again = inspect(&f.clone, &f.manifests, "sp", "tool", None);
        assert_eq!(again, p);
    }

    #[test]
    fn ac3_09_a_one_tool_repo_is_unsupported_for_the_other() {
        let f = fixture();
        write(
            &f.clone.join(".claude-plugin/plugin.json"),
            &json!({"name": "tool"}),
        );
        let p = inspect(&f.clone, &f.manifests, "sp", "tool", None);
        assert!(matches!(p.claude, ToolPackaging::Supported(_)));
        assert_eq!(
            p.codex,
            ToolPackaging::Unsupported("the repo has no Codex packaging".into())
        );
        assert!(!f.manifests.join(".agents").exists());
    }

    #[test]
    fn ec3_04_no_packaging_at_all() {
        let f = fixture();
        let p = inspect(&f.clone, &f.manifests, "sp", "tool", None);
        assert_eq!(
            p.claude,
            ToolPackaging::Unsupported("the repo has no Claude packaging".into())
        );
        assert!(!p.any_supported());
    }

    #[test]
    fn ec3_05_the_plugin_is_picked_from_several() {
        let f = fixture();
        write(
            &f.clone.join(".claude-plugin/marketplace.json"),
            &json!({"name": "m", "plugins": [{"name": "a", "source": "./a"}, {"name": "b", "source": "./b"}]}),
        );
        assert_eq!(
            supported(&inspect(&f.clone, &f.manifests, "sp", "b", None).claude).id,
            "b@m"
        );
        match inspect(&f.clone, &f.manifests, "sp", "c", None).claude {
            ToolPackaging::Invalid(r) => assert!(
                r.starts_with("no plugin c in") && r.ends_with("marketplace.json"),
                "{r}"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ec3_06_a_broken_manifest_only_affects_its_tool() {
        let f = fixture();
        fs::create_dir_all(f.clone.join(".claude-plugin")).unwrap();
        fs::write(f.clone.join(".claude-plugin/marketplace.json"), "{ nope").unwrap();
        write(
            &f.clone.join(".codex-plugin/plugin.json"),
            &json!({"name": "tool"}),
        );
        let p = inspect(&f.clone, &f.manifests, "sp", "tool", None);
        assert!(
            matches!(&p.claude, ToolPackaging::Invalid(r) if r.contains("doesn't parse")),
            "{:?}",
            p.claude
        );
        assert!(matches!(p.codex, ToolPackaging::Supported(_)));
        write(
            &f.clone.join(".claude-plugin/marketplace.json"),
            &json!({"plugins": []}),
        );
        assert!(matches!(
            inspect(&f.clone, &f.manifests, "sp", "tool", None).claude,
            ToolPackaging::Invalid(r) if r.contains("no \"name\"")
        ));
    }

    #[test]
    fn ec3_11_a_plugin_in_a_subfolder() {
        let f = fixture();
        write(
            &f.clone.join("plugins/x/.claude-plugin/plugin.json"),
            &json!({"name": "x"}),
        );
        let p = inspect(&f.clone, &f.manifests, "sp", "x", Some("plugins/x"));
        assert_eq!(supported(&p.claude).id, "x@colctl-sp");
        assert_eq!(
            fs::read_link(f.manifests.join("plugin")).unwrap(),
            f.clone.join("plugins/x")
        );
    }
}
