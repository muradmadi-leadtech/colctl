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
fn ac2_22_docs_cover_codex_and_the_agents_folder() {
    let config = read("docs/reference/config.md");
    for needle in [
        "kind = \"codex\"",
        "kind = \"agents\"",
        "[codex.marketplaces]",
        "[codex.plugins]",
        "openai-curated",
    ] {
        assert!(
            config.contains(needle),
            "reference/config.md lacks {needle}"
        );
    }
    assert!(read("docs/reference/add.md").contains("--codex"));
    assert!(read("docs/reference/rm.md").contains("--codex"));
    assert!(read("docs/reference/status.md").contains("linked-elsewhere"));
    assert!(read("docs/reference/status.md").contains("origin_changed"));
    assert!(read("docs/reference/adopt.md").contains("refresh"));
    assert!(read("docs/reference/init.md").contains("~/.agents"));
    assert!(read("docs/reference/commands.md").contains("COLCTL_TOOL_TIMEOUT_SECS"));
    assert!(read("docs/reference/commands.md").contains("refresh"));
    assert!(read("README.md").contains("Codex"));
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
        // Cycle 2 (AC2-22)
        "docs/reference/refresh.md",
        "docs/how-to/add-codex.md",
        "docs/how-to/take-over-skills-a-tool-installed.md",
        "docs/explanation/codex-and-the-shared-skills-folder.md",
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

/// IDs defined in a cycle's acceptance criteria and edge cases: `AC-07`/`EC-14` for cycle 1,
/// `AC2-07`/`EC2-14` for cycle 2.
fn defined_ids() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (dir, ac, ec) in [
        ("claude-profile-sync", "AC", "EC"),
        ("codex-cli-support", "AC2", "EC2"),
    ] {
        for (file, prefix, id) in [
            ("acceptance-criteria.md", format!("**{ac}-"), ac),
            ("edge-cases.md", format!("| {ec}-"), ec),
        ] {
            for line in read(&format!("docs/{dir}/{file}")).lines() {
                if let Some(rest) = line.strip_prefix(&prefix) {
                    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                    ids.insert(format!("{id}-{digits}"));
                }
            }
        }
    }
    ids
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// IDs named by test functions (`fn ac_09_ac_10_…`, `fn ec_14_…`) in src/ and tests/.
fn tested_ids() -> BTreeSet<String> {
    let mut files = Vec::new();
    for dir in ["src", "tests"] {
        collect_rs(&repo().join(dir), &mut files);
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
                if ["ac", "ec", "ac2", "ec2"].contains(&pair[0])
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
fn ac_35_ac2_23_every_criterion_and_edge_case_has_a_named_test() {
    let defined = defined_ids();
    for (prefix, count) in [("AC-", 35), ("EC-", 31), ("AC2-", 28), ("EC2-", 28)] {
        assert_eq!(
            defined.iter().filter(|i| i.starts_with(prefix)).count(),
            count,
            "{prefix}: {defined:?}"
        );
    }
    let tested = tested_ids();
    let untested: Vec<&String> = defined.difference(&tested).collect();
    assert!(untested.is_empty(), "no test named after: {untested:?}");
}
