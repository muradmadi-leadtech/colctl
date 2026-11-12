//! Reading the shared `~/.agents` folder (the `agents` harness kind): only its `skills/`.

use super::{Root, Scan};
use std::path::Path;

/// Reads `<harness>/skills`. `source` tells owned links from foreign ones.
pub fn scan(_harness: &Path, _source: &Path) -> Scan {
    Scan::empty(Root::Missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Entry;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn ac2_24_skills_linked_from_another_tool_are_reported_with_their_target() {
        let t = tempfile::tempdir().unwrap();
        let source = t.path().join("src");
        let agents = t.path().join(".agents");
        let twg = t.path().join(".config/twg/skills/twg-jira");
        fs::create_dir_all(&twg).unwrap();
        fs::create_dir_all(source.join("skills/foo")).unwrap();
        fs::write(source.join("skills/foo/SKILL.md"), "x").unwrap();
        fs::create_dir_all(agents.join("skills/real-one")).unwrap();
        symlink(&twg, agents.join("skills/twg-jira")).unwrap();
        symlink(source.join("skills/foo"), agents.join("skills/foo")).unwrap();
        fs::write(agents.join(".skill-lock.json"), "{}").unwrap();
        fs::write(agents.join("skills/.DS_Store"), "").unwrap();

        let s = scan(&agents, &source);
        assert_eq!(s.root, Root::Ok);
        assert_eq!(s.skills["twg-jira"], Entry::ForeignLink(twg.clone()));
        assert!(matches!(
            s.skills["foo"],
            Entry::Owned { correct: true, .. }
        ));
        assert_eq!(s.skills.len(), 3);
        assert!(s.agents.is_empty() && s.generated.is_empty());
        assert_eq!(s.plugins, Ok(Default::default()));
    }

    #[test]
    fn ec2_13_a_real_folder_with_a_source_name_is_real() {
        let t = tempfile::tempdir().unwrap();
        let agents = t.path().join(".agents");
        fs::create_dir_all(agents.join("skills/foo")).unwrap();
        assert_eq!(
            scan(&agents, &t.path().join("src")).skills["foo"],
            Entry::Real
        );
        assert_eq!(
            scan(&t.path().join("missing"), &t.path().join("src")).root,
            Root::Missing
        );
    }
}
