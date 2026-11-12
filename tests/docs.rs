//! Checks on the repository itself: the release pipeline, the user docs, and traceability from
//! every acceptance criterion and edge case to a test named after it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn ac_33_publish_builds_attested_archives_for_both_targets() {
    let publish = read(".github/workflows/publish.yml");
    for needle in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "actions/attest",
        "colctl-$TARGET.tar.gz",
        "gh release upload",
    ] {
        assert!(publish.contains(needle), "publish.yml lacks {needle}");
    }
    let release = read(".github/workflows/release.yml");
    assert!(
        release.contains("gh workflow run publish.yml"),
        "release.yml must start publish.yml on a new tag"
    );
}

#[test]
fn ac_34_docs_exist_for_everything_shipped() {
    let pages = [
        "docs/tutorials/first-sync.md",
        "docs/how-to/add-a-claude-profile.md",
        "docs/how-to/adopt-what-you-have.md",
        "docs/how-to/add-a-plugin.md",
        "docs/reference/commands.md",
        "docs/reference/init.md",
        "docs/reference/status.md",
        "docs/reference/sync.md",
        "docs/reference/add.md",
        "docs/reference/rm.md",
        "docs/reference/adopt.md",
        "docs/reference/config.md",
        "docs/reference/exit-codes.md",
        "docs/reference/state.md",
        "docs/explanation/links-not-copies.md",
        "docs/explanation/ownership.md",
    ];
    for page in pages {
        assert!(read(page).starts_with("# "), "{page} needs a title");
    }
    // Every relative link in the README and docs points at a file that exists.
    let mut files = vec![repo().join("README.md")];
    collect_md(&repo().join("docs"), &mut files);
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for target in links(&text) {
            let resolved = file.parent().unwrap().join(&target);
            assert!(
                resolved.exists(),
                "{} links to missing {target}",
                file.display()
            );
        }
    }
}

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Relative Markdown link targets, without anchors.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = rest[..end]
            .split('#')
            .next()
            .unwrap_or_default()
            .to_string();
        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            out.push(target);
        }
        rest = &rest[end..];
    }
    out
}

/// IDs (`AC-07`, `EC-14`) defined in a cycle's acceptance criteria and edge cases.
fn defined_ids() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for rel in [
        "docs/claude-profile-sync/acceptance-criteria.md",
        "docs/claude-profile-sync/edge-cases.md",
    ] {
        for line in read(rel).lines() {
            for prefix in ["**AC-", "| EC-"] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                    ids.insert(format!(
                        "{}-{digits}",
                        &prefix[prefix.len() - 3..prefix.len() - 1]
                    ));
                }
            }
        }
    }
    ids
}

/// IDs named by test functions (`fn ac_09_ac_10_…`, `fn ec_14_…`) in src/ and tests/.
fn tested_ids() -> BTreeSet<String> {
    let mut files = Vec::new();
    for dir in ["src", "tests"] {
        for entry in fs::read_dir(repo().join(dir)).unwrap().flatten() {
            if entry.path().extension().is_some_and(|e| e == "rs") {
                files.push(entry.path());
            }
        }
    }
    let mut ids = BTreeSet::new();
    for file in files {
        for line in fs::read_to_string(file).unwrap().lines() {
            let Some(name) = line.trim().strip_prefix("fn ") else {
                continue;
            };
            let name: String = name
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let parts: Vec<&str> = name.split('_').collect();
            for pair in parts.windows(2) {
                if (pair[0] == "ac" || pair[0] == "ec")
                    && pair[1].len() == 2
                    && pair[1].chars().all(|c| c.is_ascii_digit())
                {
                    ids.insert(format!("{}-{}", pair[0].to_uppercase(), pair[1]));
                }
            }
        }
    }
    ids
}

#[test]
fn ac_35_every_criterion_and_edge_case_has_a_named_test() {
    let defined = defined_ids();
    assert_eq!(
        defined.iter().filter(|i| i.starts_with("AC-")).count(),
        35,
        "{defined:?}"
    );
    assert_eq!(
        defined.iter().filter(|i| i.starts_with("EC-")).count(),
        31,
        "{defined:?}"
    );
    let tested = tested_ids();
    let untested: Vec<&String> = defined.difference(&tested).collect();
    assert!(untested.is_empty(), "no test named after: {untested:?}");
}
