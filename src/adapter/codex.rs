//! Reading a Codex CLI profile (the `codex` harness kind, a `CODEX_HOME`) into a [`Scan`].

use super::{Plugins, Root, Scan};
use serde_json::Value;
use std::path::Path;

pub const AGENTS_MD: &str = "AGENTS.md";

/// Reads a Codex profile. `source` tells owned links from foreign ones.
pub fn scan(_harness: &Path, _source: &Path) -> Scan {
    Scan::empty(Root::Missing)
}

/// Installed plugins and known marketplaces from `$CODEX_HOME/config.toml`.
pub fn read_plugins(_harness: &Path) -> Result<Plugins, String> {
    Err("not implemented".into())
}

/// Turns a `[marketplaces.<name>]` entry back into a config source, for adopt (AC2-18).
pub fn marketplace_source(_entry: &Value) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{Entry, Generated, Instructions};
    use serde_json::json;
    use std::fs;
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
        let harness = t.path().join("codex");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("AGENTS.md"), "rules").unwrap();
        fs::create_dir_all(harness.join("agents")).unwrap();
        Fixture {
            _t: t,
            source,
            harness,
        }
    }

    const CONFIG: &str = r#"model = "gpt-6.1-sol"

[marketplaces.superpowers-dev]
source_type = "git"
source = "https://github.com/obra/superpowers.git"

[marketplaces.mine]
source_type = "local"
source = "/opt/market"

[plugins."superpowers@superpowers-dev"]
enabled = true

[plugins."quiet@mine"]
enabled = false
"#;

    #[test]
    fn ac2_16_plugin_state_comes_from_config_toml() {
        let f = fixture();
        fs::write(f.harness.join("config.toml"), CONFIG).unwrap();
        let p = read_plugins(&f.harness).unwrap();
        assert_eq!(
            p.installed.iter().map(String::as_str).collect::<Vec<_>>(),
            vec!["quiet@mine", "superpowers@superpowers-dev"]
        );
        assert_eq!(
            p.marketplaces
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["mine", "superpowers-dev"]
        );
        assert_eq!(scan(&f.harness, &f.source).plugins.unwrap(), p);
    }

    #[test]
    fn ec2_16_disabled_plugins_count_as_installed() {
        let f = fixture();
        fs::write(f.harness.join("config.toml"), CONFIG).unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap()
                .installed
                .contains("quiet@mine")
        );
    }

    #[test]
    fn ec2_04_missing_config_means_nothing_installed() {
        let f = fixture();
        assert_eq!(read_plugins(&f.harness).unwrap(), Plugins::default());
    }

    #[test]
    fn ec2_07_unparsable_config_is_an_error() {
        let f = fixture();
        fs::write(f.harness.join("config.toml"), "[plugins\nbroken").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("config.toml doesn't parse")
        );
        fs::write(f.harness.join("config.toml"), "plugins = 3\n").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("[plugins] must be a table")
        );
        fs::write(f.harness.join("config.toml"), "marketplaces = 3\n").unwrap();
        assert!(
            read_plugins(&f.harness)
                .unwrap_err()
                .contains("[marketplaces] must be a table")
        );
    }

    #[test]
    fn ec2_21_formatting_and_order_do_not_matter() {
        let f = fixture();
        fs::write(f.harness.join("config.toml"), CONFIG).unwrap();
        let a = read_plugins(&f.harness).unwrap();
        let rewritten = "plugins = { \"quiet@mine\" = { enabled = false }, \"superpowers@superpowers-dev\" = { enabled = true } }\nmarketplaces = { mine = { source_type = \"local\", source = \"/opt/market\" }, superpowers-dev = { source = \"https://github.com/obra/superpowers.git\", source_type = \"git\" } }\nmodel = \"x\"\n";
        fs::write(f.harness.join("config.toml"), rewritten).unwrap();
        assert_eq!(read_plugins(&f.harness).unwrap(), a);
    }

    #[test]
    fn marketplace_sources_map_back_to_config() {
        assert_eq!(
            marketplace_source(
                &json!({"source_type": "git", "source": "https://github.com/o/r.git"})
            )
            .as_deref(),
            Some("https://github.com/o/r.git")
        );
        assert_eq!(
            marketplace_source(&json!({"source_type": "local", "source": "/m"})).as_deref(),
            Some("/m")
        );
        assert!(marketplace_source(&json!({"source_type": "npm", "source": "x"})).is_none());
        assert!(marketplace_source(&json!({})).is_none());
    }

    #[test]
    fn agents_md_link_states() {
        let f = fixture();
        assert_eq!(
            scan(&f.harness, &f.source).instructions,
            Instructions::Absent
        );
        symlink(f.source.join("AGENTS.md"), f.harness.join("AGENTS.md")).unwrap();
        assert!(matches!(
            scan(&f.harness, &f.source).instructions,
            Instructions::Linked(Entry::Owned { correct: true, .. })
        ));
        fs::remove_file(f.harness.join("AGENTS.md")).unwrap();
        fs::write(f.harness.join("AGENTS.md"), "mine").unwrap();
        assert!(matches!(
            scan(&f.harness, &f.source).instructions,
            Instructions::File { .. }
        ));
    }

    #[test]
    fn ec2_12_agents_md_linked_elsewhere_is_foreign() {
        let f = fixture();
        symlink("/elsewhere/AGENTS.md", f.harness.join("AGENTS.md")).unwrap();
        assert_eq!(
            scan(&f.harness, &f.source).instructions,
            Instructions::Linked(Entry::ForeignLink("/elsewhere/AGENTS.md".into()))
        );
    }

    #[test]
    fn generated_agent_files_are_read() {
        let f = fixture();
        fs::write(f.harness.join("agents/rev.toml"), "name = \"rev\"").unwrap();
        symlink("/x.toml", f.harness.join("agents/linked.toml")).unwrap();
        fs::write(f.harness.join("agents/notes.md"), "ignored").unwrap();
        fs::write(f.harness.join("agents/.colctl-tmp-1-x.toml"), "").unwrap();
        let s = scan(&f.harness, &f.source);
        assert_eq!(s.root, Root::Ok);
        assert!(
            matches!(&s.generated["rev"], Generated::File { content, .. } if content == b"name = \"rev\"")
        );
        assert_eq!(s.generated["linked"], Generated::Link("/x.toml".into()));
        assert_eq!(s.generated.len(), 2);
        assert!(s.skills.is_empty() && s.agents.is_empty());
        assert_eq!(s.temps.len(), 1);
    }

    #[test]
    fn missing_and_invalid_roots() {
        let f = fixture();
        assert_eq!(scan(&f.harness.join("nope"), &f.source).root, Root::Missing);
        fs::write(f.harness.join("file"), "").unwrap();
        assert_eq!(scan(&f.harness.join("file"), &f.source).root, Root::Invalid);
    }
}
