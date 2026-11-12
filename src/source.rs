//! Reads the source store: `AGENTS.md`, `skills/<name>/` and `agents/<name>.md`.

use crate::config::valid_item_name;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const AGENTS_MD: &str = "AGENTS.md";
pub const SKILLS: &str = "skills";
pub const AGENTS: &str = "agents";
/// claude.ai account skills live here in `~/.claude/skills`; never managed (EC-16).
pub const SYNCED: &str = "synced";

/// A source entry that can't be synced, with the reason (EC-05, EC-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    pub kind: &'static str,
    pub name: String,
    pub reason: String,
}

/// What the source holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub agents_md: bool,
    pub skills: BTreeSet<String>,
    pub agents: BTreeSet<String>,
    pub invalid: Vec<Invalid>,
}

/// Lists entries of `dir` that aren't dot-entries, sorted. A missing folder is empty.
pub fn visible_entries(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

pub fn scan(source: &Path) -> Source {
    let mut out = Source {
        agents_md: source.join(AGENTS_MD).is_file(),
        ..Source::default()
    };
    for name in visible_entries(&source.join(SKILLS)) {
        let path = source.join(SKILLS).join(&name);
        let meta = fs::symlink_metadata(&path);
        let reason = if !valid_item_name(&name) {
            Some("name must match [A-Za-z0-9][A-Za-z0-9._-]*".to_string())
        } else if !meta.as_ref().is_ok_and(|m| m.is_dir()) {
            Some("a skill must be a real folder".to_string())
        } else if !path.join("SKILL.md").is_file() {
            Some("no SKILL.md".to_string())
        } else {
            None
        };
        match reason {
            Some(reason) => out.invalid.push(Invalid {
                kind: "skill",
                name,
                reason,
            }),
            None => {
                out.skills.insert(name);
            }
        }
    }
    for file in visible_entries(&source.join(AGENTS)) {
        let path = source.join(AGENTS).join(&file);
        let stem = file.strip_suffix(".md");
        let reason = match stem {
            None => Some("an agent must be a .md file".to_string()),
            Some(stem) if !valid_item_name(stem) => {
                Some("name must match [A-Za-z0-9][A-Za-z0-9._-]*".to_string())
            }
            Some(_) if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) => {
                Some("an agent must be a regular file".to_string())
            }
            Some(_) => None,
        };
        match (reason, stem) {
            (None, Some(stem)) => {
                out.agents.insert(stem.to_string());
            }
            (reason, _) => out.invalid.push(Invalid {
                kind: "agent",
                name: file.clone(),
                reason: reason.unwrap_or_default(),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec_03_empty_source() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(scan(t.path()), Source::default());
    }

    #[test]
    fn valid_items_are_listed() {
        let t = tempfile::tempdir().unwrap();
        let s = t.path();
        fs::write(s.join(AGENTS_MD), "").unwrap();
        fs::create_dir_all(s.join("skills/foo")).unwrap();
        fs::write(s.join("skills/foo/SKILL.md"), "x").unwrap();
        fs::create_dir_all(s.join("skills/.colctl-tmp-x")).unwrap();
        fs::create_dir_all(s.join("agents")).unwrap();
        fs::write(s.join("agents/reviewer.md"), "x").unwrap();
        let src = scan(s);
        assert!(src.agents_md);
        assert_eq!(src.skills, BTreeSet::from(["foo".to_string()]));
        assert_eq!(src.agents, BTreeSet::from(["reviewer".to_string()]));
        assert!(src.invalid.is_empty());
    }

    #[test]
    fn ec_05_skill_without_skill_md_is_invalid() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("skills/bare")).unwrap();
        let src = scan(t.path());
        assert!(src.skills.is_empty());
        assert_eq!(src.invalid[0].name, "bare");
        assert_eq!(src.invalid[0].reason, "no SKILL.md");
    }

    #[test]
    fn ec_08_bad_names_and_shapes_are_invalid() {
        let t = tempfile::tempdir().unwrap();
        let s = t.path();
        fs::create_dir_all(s.join("skills/bad name")).unwrap();
        fs::write(s.join("skills/file-not-folder"), "x").unwrap();
        fs::create_dir_all(s.join("agents/folder.md")).unwrap();
        fs::write(s.join("agents/notes.txt"), "x").unwrap();
        fs::write(s.join("agents/bad name.md"), "x").unwrap();
        let src = scan(s);
        assert!(src.skills.is_empty() && src.agents.is_empty());
        let reasons: Vec<String> = src
            .invalid
            .iter()
            .map(|i| format!("{}:{}", i.name, i.reason))
            .collect();
        assert_eq!(src.invalid.len(), 5, "{reasons:?}");
        assert!(reasons.iter().any(|r| r.starts_with("bad name:name must")));
        assert!(reasons.iter().any(|r| r.contains("real folder")));
        assert!(reasons.iter().any(|r| r.contains("regular file")));
        assert!(reasons.iter().any(|r| r.contains(".md file")));
        assert!(
            reasons
                .iter()
                .any(|r| r.starts_with("bad name.md:name must"))
        );
    }
}
