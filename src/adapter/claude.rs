//! Reading a Claude Code profile (the `claude` harness kind) into a [`Scan`].

use super::{Instructions, Plugins, Root, Scan, link_entries, owned_by, root_of, temps_in};
use crate::fsx;
use crate::source::{AGENTS, SKILLS, SYNCED};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub const CLAUDE_MD: &str = "CLAUDE.md";
const INSTALLED_PLUGINS: &str = "plugins/installed_plugins.json";
const KNOWN_MARKETPLACES: &str = "plugins/known_marketplaces.json";

/// Reads a harness. `source` is used to tell owned links from foreign ones.
pub fn scan(harness: &Path, source: &Path) -> Scan {
    if let Some(root) = root_of(harness) {
        return Scan::empty(root);
    }
    let owned = owned_by(source);
    let skills = link_entries(
        &harness.join(SKILLS),
        &source.join(SKILLS),
        "",
        &[SYNCED],
        &owned,
        |p| p.is_dir(),
    );
    let agents = link_entries(
        &harness.join(AGENTS),
        &source.join(AGENTS),
        ".md",
        &[],
        &owned,
        |p| p.is_file(),
    );

    let claude_md = harness.join(CLAUDE_MD);
    let instructions = if fsx::is_symlink(&claude_md) {
        Instructions::Link
    } else {
        match fs::read(&claude_md) {
            Ok(content) => Instructions::File {
                hash: fsx::hash_bytes(&content),
                content,
            },
            Err(_) => Instructions::Absent,
        }
    };

    Scan {
        root: Root::Ok,
        instructions,
        skills,
        agents,
        plugins: read_plugins(harness),
        temps: temps_in(harness, &[SKILLS, AGENTS]),
    }
}

fn read_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{} doesn't parse: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("reading {}: {e}", path.display())),
    }
}

/// Reads `installed_plugins.json` and `known_marketplaces.json`; missing files mean nothing is
/// installed (EC-12), unparsable ones are an error (EC-13).
pub fn read_plugins(harness: &Path) -> Result<Plugins, String> {
    let mut plugins = Plugins::default();
    if let Some(value) = read_json(&harness.join(INSTALLED_PLUGINS))? {
        let map = value
            .get("plugins")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{INSTALLED_PLUGINS} has no \"plugins\" object"))?;
        plugins.installed = map.keys().cloned().collect();
    }
    if let Some(value) = read_json(&harness.join(KNOWN_MARKETPLACES))? {
        let map = value
            .as_object()
            .ok_or_else(|| format!("{KNOWN_MARKETPLACES} isn't a JSON object"))?;
        plugins.marketplaces = map.clone().into_iter().collect();
    }
    Ok(plugins)
}

/// Turns a `known_marketplaces.json` entry back into a config source, for `adopt` (AC-27).
pub fn marketplace_source(entry: &Value) -> Option<String> {
    let source = entry.get("source")?;
    let field = |name: &str| source.get(name).and_then(Value::as_str);
    match field("source")? {
        "github" => Some(format!("github:{}", field("repo")?)),
        "url" | "git" => field("url").map(str::to_string),
        "directory" | "file" | "local" | "path" => field("path").map(str::to_string),
        _ => None,
    }
}

/// The `CLAUDE.md` colctl generates: a comment and an import of the source's `AGENTS.md`.
pub fn generated_claude_md(source_display: &str) -> Vec<u8> {
    format!(
        "<!-- Managed by colctl. Edit {source_display}/AGENTS.md instead; this file is regenerated. -->\n@{source_display}/AGENTS.md\n"
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Entry;
    use serde_json::json;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct Fixture {
        _t: tempfile::TempDir,
        source: PathBuf,
        harness: PathBuf,
    }

    fn fixture() -> Fixture {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("src");
        let harness = t.path().join("h");
        fs::create_dir_all(source.join("skills/foo")).unwrap();
        fs::write(source.join("skills/foo/SKILL.md"), "x").unwrap();
        fs::create_dir_all(source.join("agents")).unwrap();
        fs::write(source.join("agents/rev.md"), "x").unwrap();
        fs::create_dir_all(harness.join("skills")).unwrap();
        fs::create_dir_all(harness.join("agents")).unwrap();
        Fixture {
            _t: t,
            source,
            harness,
        }
    }

    #[test]
    fn ec_06_ec_07_missing_and_invalid_roots() {
        let f = fixture();
        assert_eq!(scan(&f.harness.join("nope"), &f.source).root, Root::Missing);
        let file = f.harness.join("file");
        fs::write(&file, "x").unwrap();
        assert_eq!(scan(&file, &f.source).root, Root::Invalid);
        let link = f.harness.join("link");
        symlink(&f.harness, &link).unwrap();
        assert_eq!(scan(&link, &f.source).root, Root::Invalid);
    }

    #[test]
    fn owned_foreign_and_real_entries() {
        let f = fixture();
        symlink(f.source.join("skills/foo"), f.harness.join("skills/foo")).unwrap();
        symlink(f.source.join("skills/gone"), f.harness.join("skills/gone")).unwrap();
        symlink(
            f.source.join("skills/foo"),
            f.harness.join("skills/misnamed"),
        )
        .unwrap();
        symlink("/elsewhere/x", f.harness.join("skills/mine")).unwrap();
        fs::create_dir_all(f.harness.join("skills/real")).unwrap();
        fs::create_dir_all(f.harness.join("skills/synced/abc")).unwrap();
        fs::create_dir_all(f.harness.join("skills/.hidden")).unwrap();
        symlink(
            f.source.join("agents/rev.md"),
            f.harness.join("agents/rev.md"),
        )
        .unwrap();
        fs::write(f.harness.join("agents/notes.txt"), "x").unwrap();

        let s = scan(&f.harness, &f.source);
        assert_eq!(s.root, Root::Ok);
        assert!(matches!(
            s.skills["foo"],
            Entry::Owned { correct: true, .. }
        ));
        assert!(matches!(
            s.skills["gone"],
            Entry::Owned { correct: false, .. }
        ));
        assert!(matches!(
            s.skills["misnamed"],
            Entry::Owned { correct: false, .. }
        ));
        assert_eq!(
            s.skills["mine"],
            Entry::ForeignLink(PathBuf::from("/elsewhere/x"))
        );
        assert_eq!(s.skills["real"], Entry::Real);
        assert!(!s.skills.contains_key("synced"), "EC-16");
        assert!(!s.skills.contains_key(".hidden"), "EC-16");
        assert!(matches!(
            s.agents["rev"],
            Entry::Owned { correct: true, .. }
        ));
        assert_eq!(s.agents.len(), 1);
    }

    #[test]
    fn ec_16_synced_skills_are_ignored() {
        let f = fixture();
        fs::create_dir_all(f.harness.join("skills/synced/uuid/pdf")).unwrap();
        assert!(scan(&f.harness, &f.source).skills.is_empty());
    }

    #[test]
    fn instructions_states() {
        let f = fixture();
        assert_eq!(
            scan(&f.harness, &f.source).instructions,
            Instructions::Absent
        );
        fs::write(f.harness.join(CLAUDE_MD), "hi").unwrap();
        match scan(&f.harness, &f.source).instructions {
            Instructions::File { content, hash } => {
                assert_eq!(content, b"hi");
                assert_eq!(hash, fsx::hash_bytes(b"hi"));
            }
            other => panic!("{other:?}"),
        }
        fs::remove_file(f.harness.join(CLAUDE_MD)).unwrap();
        symlink("/x", f.harness.join(CLAUDE_MD)).unwrap();
        assert_eq!(scan(&f.harness, &f.source).instructions, Instructions::Link);
    }

    #[test]
    fn ec_21_temps_are_reported() {
        let f = fixture();
        fs::write(f.harness.join("skills/.colctl-tmp-1-foo"), "").unwrap();
        fs::write(f.harness.join(".colctl-tmp-2-CLAUDE.md"), "").unwrap();
        assert_eq!(scan(&f.harness, &f.source).temps.len(), 2);
    }

    #[test]
    fn ec_12_missing_plugin_files_mean_nothing_installed() {
        let f = fixture();
        assert_eq!(read_plugins(&f.harness).unwrap(), Plugins::default());
    }

    #[test]
    fn plugin_files_are_read() {
        let f = fixture();
        fs::create_dir_all(f.harness.join("plugins")).unwrap();
        fs::write(
            f.harness.join(INSTALLED_PLUGINS),
            json!({"version": 2, "plugins": {"a@m": [], "b@anthropic-plugin-directory": []}})
                .to_string(),
        )
        .unwrap();
        fs::write(
            f.harness.join(KNOWN_MARKETPLACES),
            json!({"m": {"source": {"source": "github", "repo": "o/r"}}}).to_string(),
        )
        .unwrap();
        let p = read_plugins(&f.harness).unwrap();
        assert_eq!(p.installed.len(), 2);
        assert_eq!(
            marketplace_source(&p.marketplaces["m"]).unwrap(),
            "github:o/r"
        );
    }

    #[test]
    fn ec_13_unparsable_plugin_files_are_errors() {
        let f = fixture();
        fs::create_dir_all(f.harness.join("plugins")).unwrap();
        fs::write(f.harness.join(INSTALLED_PLUGINS), "{ nope").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("doesn't parse")
        );
        fs::write(f.harness.join(INSTALLED_PLUGINS), "{}").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("no \"plugins\"")
        );
        fs::write(f.harness.join(INSTALLED_PLUGINS), r#"{"plugins":{}}"#).unwrap();
        fs::write(f.harness.join(KNOWN_MARKETPLACES), "[]").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("isn't a JSON object")
        );
        fs::remove_file(f.harness.join(KNOWN_MARKETPLACES)).unwrap();
        fs::create_dir(f.harness.join(KNOWN_MARKETPLACES)).unwrap();
        assert!(read_plugins(&f.harness).unwrap_err().contains("reading"));
        assert!(scan(&f.harness, &f.source).plugins.is_err());
    }

    #[test]
    fn marketplace_sources_map_back_to_config() {
        let src = |v: Value| marketplace_source(&json!({ "source": v }));
        assert_eq!(
            src(json!({"source": "url", "url": "https://x"})).unwrap(),
            "https://x"
        );
        assert_eq!(
            src(json!({"source": "git", "url": "https://g"})).unwrap(),
            "https://g"
        );
        assert_eq!(
            src(json!({"source": "directory", "path": "/p"})).unwrap(),
            "/p"
        );
        assert!(src(json!({"source": "npm", "package": "x"})).is_none());
        assert!(marketplace_source(&json!({})).is_none());
    }

    #[test]
    fn generated_content_is_exact() {
        assert_eq!(
            String::from_utf8(generated_claude_md("~/.colctl")).unwrap(),
            "<!-- Managed by colctl. Edit ~/.colctl/AGENTS.md instead; this file is regenerated. -->\n@~/.colctl/AGENTS.md\n"
        );
    }
}
