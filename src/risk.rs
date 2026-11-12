//! Risk flags for a held candidate (interfaces.md § Risk rules): changes that let code run or widen
//! what an agent may do. Pure: it reads the changed files it's given and nothing else. A flag points
//! the owner at a file; it never says a version is safe.

use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use yaml_rust2::{Yaml, YamlLoader};

/// How a file changed between the installed version and the candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Added,
    Modified,
    Deleted,
    Renamed,
    Mode,
}

impl Status {
    /// The one-letter code `review` prints.
    pub fn letter(self) -> char {
        match self {
            Status::Added => 'A',
            Status::Modified => 'M',
            Status::Deleted => 'D',
            Status::Renamed => 'R',
            Status::Mode => 'T',
        }
    }
}

/// One side of a changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    File { bytes: Vec<u8>, executable: bool },
    Link(String),
}

/// A changed file. `old` is `None` when added, `new` is `None` when deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub path: String,
    pub old_path: Option<String>,
    pub status: Status,
    pub old: Option<Content>,
    pub new: Option<Content>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlagKind {
    Hook,
    Mcp,
    Lsp,
    Executable,
    Script,
    Binary,
    Symlink,
    AllowedTools,
    InlineShell,
    Manifest,
}

impl FlagKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FlagKind::Hook => "hook",
            FlagKind::Mcp => "mcp",
            FlagKind::Lsp => "lsp",
            FlagKind::Executable => "executable",
            FlagKind::Script => "script",
            FlagKind::Binary => "binary",
            FlagKind::Symlink => "symlink",
            FlagKind::AllowedTools => "allowed-tools",
            FlagKind::InlineShell => "inline-shell",
            FlagKind::Manifest => "manifest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flag {
    pub kind: FlagKind,
    pub path: String,
    pub reason: String,
}

const SCRIPT_EXTENSIONS: [&str; 12] = [
    "sh", "bash", "zsh", "fish", "py", "js", "mjs", "cjs", "ts", "rb", "pl", "ps1",
];

/// The flags for a set of changed files, ordered by path, then kind.
pub fn flags(changes: &[Changed]) -> Vec<Flag> {
    let _ = changes; // RED: not implemented yet
    Vec::new()
}

#[allow(dead_code)]
fn flags_impl(changes: &[Changed]) -> Vec<Flag> {
    let mut out = Vec::new();
    for c in changes {
        let Some(new) = &c.new else { continue };
        let mut flag = |kind: FlagKind, reason: String| {
            out.push(Flag {
                kind,
                path: c.path.clone(),
                reason,
            });
        };
        let added = c.old.is_none();
        let verb = if added { "adds" } else { "changes" };
        let new_or_changed = if added { "new" } else { "changed" };
        let name = c.path.rsplit('/').next().unwrap_or(&c.path);

        if let Content::Link(target) = new {
            if c.old.as_ref() != Some(new) {
                let mut reason = format!("links to {target}");
                if points_outside(&c.path, target) {
                    reason.push_str("; points outside");
                }
                flag(FlagKind::Symlink, reason);
            }
            continue;
        }
        let Content::File { bytes, executable } = new else {
            continue;
        };
        let (old_bytes, old_exec) = match &c.old {
            Some(Content::File { bytes, executable }) => (Some(bytes.as_slice()), *executable),
            _ => (None, false),
        };
        let content_changed = old_bytes != Some(bytes.as_slice());

        if name == "hooks.json" && content_changed {
            flag(FlagKind::Hook, format!("{verb} hooks"));
        }
        if name == ".mcp.json" && content_changed {
            flag(FlagKind::Mcp, format!("{verb} MCP servers"));
        }
        if name == ".lsp.json" && content_changed {
            flag(FlagKind::Lsp, format!("{verb} LSP servers"));
        }
        if name == "plugin.json" && content_changed {
            for (key, kind, what) in [
                ("hooks", FlagKind::Hook, "hooks"),
                ("mcpServers", FlagKind::Mcp, "MCP servers"),
                ("lspServers", FlagKind::Lsp, "LSP servers"),
            ] {
                let now = json_key(bytes, key);
                if now.is_some() && now != old_bytes.and_then(|b| json_key(b, key)) {
                    flag(kind, format!("{verb} {what} in plugin.json"));
                }
            }
        }
        if *executable {
            if !old_exec {
                let what = if added { "new" } else { "newly" };
                flag(FlagKind::Executable, format!("{what} executable file"));
            } else if content_changed {
                flag(FlagKind::Executable, "changed executable file".into());
            }
        }
        if content_changed {
            let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
            if SCRIPT_EXTENSIONS.contains(&ext) || bytes.starts_with(b"#!") {
                flag(FlagKind::Script, format!("{new_or_changed} script"));
            }
            if std::str::from_utf8(bytes).is_err() {
                flag(FlagKind::Binary, format!("{new_or_changed} binary file"));
            }
        }
        if content_changed && name.ends_with(".md") {
            let text = String::from_utf8_lossy(bytes);
            let old_text = old_bytes.map(String::from_utf8_lossy).unwrap_or_default();
            let keys: &[&str] = if is_agent(&c.path) {
                &["allowed-tools", "tools"]
            } else {
                &["allowed-tools"]
            };
            for key in keys {
                let now = frontmatter_key(&text, key);
                if let Some(value) = &now
                    && now != frontmatter_key(&old_text, key)
                {
                    let bash = if value.contains("Bash") { "Bash: " } else { "" };
                    flag(FlagKind::AllowedTools, format!("grants {bash}{value}"));
                }
            }
            let before: std::collections::HashSet<&str> = old_text.lines().collect();
            for line in text.lines() {
                if line.contains("!`") && !before.contains(line) {
                    let shown: String = line.trim().chars().take(80).collect();
                    flag(
                        FlagKind::InlineShell,
                        format!("runs a shell command when loaded: {shown}"),
                    );
                }
            }
        }
        if is_manifest(&c.path) && (content_changed || c.status == Status::Renamed) {
            let what = if added { "added" } else { "changed" };
            flag(FlagKind::Manifest, format!("plugin manifest {what}"));
        }
    }
    out.sort_by(|a, b| (&a.path, a.kind).cmp(&(&b.path, b.kind)));
    out
}

fn is_manifest(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    parts
        .iter()
        .any(|p| *p == ".claude-plugin" || *p == ".codex-plugin")
        || parts.windows(2).any(|w| w == [".agents", "plugins"])
}

fn is_agent(path: &str) -> bool {
    path.split('/').rev().nth(1) == Some("agents")
}

/// True when a symlink at `path` (relative to the repo or skill) with `target` resolves outside it.
/// Resolved by reading the path only; nothing is followed.
fn points_outside(path: &str, target: &str) -> bool {
    if Path::new(target).is_absolute() {
        return true;
    }
    let mut at: PathBuf = Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    for part in Path::new(target).components() {
        match part {
            Component::ParentDir => {
                if !at.pop() {
                    return true;
                }
            }
            Component::Normal(p) => at.push(p),
            _ => {}
        }
    }
    false
}

/// A top-level key of a JSON object, as canonical text.
fn json_key(bytes: &[u8], key: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value.get(key).map(serde_json::Value::to_string)
}

/// A key of a Markdown file's YAML frontmatter, as text (lists joined with ", ").
fn frontmatter_key(text: &str, key: &str) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let docs = YamlLoader::load_from_str(&rest[..end]).ok()?;
    let value = &docs.first()?[key];
    match value {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Array(items) => Some(
            items
                .iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Yaml::BadValue | Yaml::Null => None,
        other => Some(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> Option<Content> {
        Some(Content::File {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        })
    }

    fn exe(text: &str) -> Option<Content> {
        Some(Content::File {
            bytes: text.as_bytes().to_vec(),
            executable: true,
        })
    }

    fn link(target: &str) -> Option<Content> {
        Some(Content::Link(target.into()))
    }

    fn change(path: &str, old: Option<Content>, new: Option<Content>) -> Changed {
        let status = match (&old, &new) {
            (None, _) => Status::Added,
            (_, None) => Status::Deleted,
            _ => Status::Modified,
        };
        Changed {
            path: path.into(),
            old_path: None,
            status,
            old,
            new,
        }
    }

    fn kinds(changes: &[Changed]) -> Vec<(FlagKind, String, String)> {
        flags(changes)
            .into_iter()
            .map(|f| (f.kind, f.path, f.reason))
            .collect()
    }

    #[test]
    fn ac4_04_hooks_are_flagged() {
        let got = kinds(&[
            change("hooks/hooks.json", None, file("{}")),
            change("sub/hooks.json", file("{}"), file(r#"{"a":1}"#)),
            change(
                ".claude-plugin/plugin.json",
                file(r#"{"name":"p"}"#),
                file(r#"{"name":"p","hooks":"./h.json"}"#),
            ),
        ]);
        assert!(got.contains(&(
            FlagKind::Hook,
            "hooks/hooks.json".into(),
            "adds hooks".into()
        )));
        assert!(got.contains(&(
            FlagKind::Hook,
            "sub/hooks.json".into(),
            "changes hooks".into()
        )));
        assert!(got.contains(&(
            FlagKind::Hook,
            ".claude-plugin/plugin.json".into(),
            "changes hooks in plugin.json".into()
        )));
        // The same hooks key, unchanged, isn't flagged as a hook.
        let same = kinds(&[change(
            "plugin.json",
            file(r#"{"hooks":"x","v":1}"#),
            file(r#"{"hooks":"x","v":2}"#),
        )]);
        assert!(same.iter().all(|f| f.0 != FlagKind::Hook), "{same:?}");
    }

    #[test]
    fn ac4_05_mcp_and_lsp_servers_are_flagged() {
        let got = kinds(&[
            change(".mcp.json", None, file("{}")),
            change(".lsp.json", file("{}"), file("{\"x\":1}")),
            change(
                "plugin.json",
                None,
                file(r#"{"mcpServers":{"a":{}},"lspServers":{"b":{}}}"#),
            ),
        ]);
        assert!(got.contains(&(FlagKind::Mcp, ".mcp.json".into(), "adds MCP servers".into())));
        assert!(got.contains(&(
            FlagKind::Lsp,
            ".lsp.json".into(),
            "changes LSP servers".into()
        )));
        assert!(got.contains(&(
            FlagKind::Mcp,
            "plugin.json".into(),
            "adds MCP servers in plugin.json".into()
        )));
        assert!(got.contains(&(
            FlagKind::Lsp,
            "plugin.json".into(),
            "adds LSP servers in plugin.json".into()
        )));
    }

    #[test]
    fn ac4_06_code_that_can_run_is_flagged() {
        let mut mode = change("bin/tool", file("x"), exe("x"));
        mode.status = Status::Mode;
        let got = kinds(&[
            mode,
            change("bin/new", None, exe("echo")),
            change("bin/old", exe("a"), exe("b")),
            change("scripts/run.sh", None, file("echo hi")),
            change("hook", file("#!/bin/sh\na"), file("#!/bin/sh\nb")),
            change("lib/x.py", file("1"), file("2")),
            change(
                "blob.bin",
                None,
                Some(Content::File {
                    bytes: vec![0xff, 0xfe, 0x00],
                    executable: false,
                }),
            ),
        ]);
        let has = |k: FlagKind, p: &str, r: &str| got.contains(&(k, p.into(), r.into()));
        assert!(
            has(FlagKind::Executable, "bin/tool", "newly executable file"),
            "{got:?}"
        );
        assert!(has(FlagKind::Executable, "bin/new", "new executable file"));
        assert!(has(
            FlagKind::Executable,
            "bin/old",
            "changed executable file"
        ));
        assert!(has(FlagKind::Script, "scripts/run.sh", "new script"));
        assert!(has(FlagKind::Script, "hook", "changed script"));
        assert!(has(FlagKind::Script, "lib/x.py", "changed script"));
        assert!(has(FlagKind::Binary, "blob.bin", "new binary file"));
        // A mode change alone isn't a script or binary change.
        assert!(
            !got.iter()
                .any(|f| f.1 == "bin/tool" && f.0 != FlagKind::Executable)
        );
    }

    #[test]
    fn ac4_07_symlinks_are_flagged_with_their_target() {
        let got = kinds(&[
            change("skills/a", None, link("../shared/a")),
            change("deep/x", link("y"), link("../../outside")),
            change("abs", None, link("/etc/passwd")),
            change("same", link("t"), link("t")),
        ]);
        assert!(got.contains(&(
            FlagKind::Symlink,
            "skills/a".into(),
            "links to ../shared/a".into()
        )));
        assert!(got.contains(&(
            FlagKind::Symlink,
            "deep/x".into(),
            "links to ../../outside; points outside".into()
        )));
        assert!(got.contains(&(
            FlagKind::Symlink,
            "abs".into(),
            "links to /etc/passwd; points outside".into()
        )));
        assert!(!got.iter().any(|f| f.1 == "same"));
    }

    #[test]
    fn ac4_08_permissions_and_inline_shell_are_flagged() {
        let skill_old = "---\nname: s\nallowed-tools: Read\n---\nbody\n";
        let skill_new =
            "---\nname: s\nallowed-tools: Read, Bash(git:*)\n---\nbody\nRun !`git status` first\n";
        let agent = "---\nname: a\ntools:\n  - Read\n  - Bash\n---\nx\n";
        let got = kinds(&[
            change("skills/s/SKILL.md", file(skill_old), file(skill_new)),
            change("agents/a.md", None, file(agent)),
            change(
                "commands/c.md",
                None,
                file("---\nallowed-tools: Edit\n---\n"),
            ),
        ]);
        assert!(
            got.contains(&(
                FlagKind::AllowedTools,
                "skills/s/SKILL.md".into(),
                "grants Bash: Read, Bash(git:*)".into()
            )),
            "{got:?}"
        );
        assert!(got.contains(&(
            FlagKind::InlineShell,
            "skills/s/SKILL.md".into(),
            "runs a shell command when loaded: Run !`git status` first".into()
        )));
        assert!(got.contains(&(
            FlagKind::AllowedTools,
            "agents/a.md".into(),
            "grants Bash: Read, Bash".into()
        )));
        assert!(got.contains(&(
            FlagKind::AllowedTools,
            "commands/c.md".into(),
            "grants Edit".into()
        )));
        // `tools` outside an agents/ folder isn't a permission key.
        let not_agent = kinds(&[change("docs/x.md", None, file("---\ntools: Bash\n---\n"))]);
        assert!(not_agent.is_empty(), "{not_agent:?}");
        // An inline-shell line that was already there isn't new.
        let same_line = kinds(&[change("s.md", file("a !`ls`\n"), file("a !`ls`\nmore\n"))]);
        assert!(same_line.is_empty(), "{same_line:?}");
    }

    #[test]
    fn ac4_09_manifests_are_flagged() {
        let got = kinds(&[
            change(
                ".claude-plugin/marketplace.json",
                file("{}"),
                file("{\"a\":1}"),
            ),
            change("p/.codex-plugin/plugin.json", None, file("{}")),
            change(
                ".agents/plugins/marketplace.json",
                file("{}"),
                file("{\"b\":1}"),
            ),
        ]);
        let manifests: Vec<_> = got.iter().filter(|f| f.0 == FlagKind::Manifest).collect();
        assert_eq!(manifests.len(), 3, "{got:?}");
        assert!(got.contains(&(
            FlagKind::Manifest,
            "p/.codex-plugin/plugin.json".into(),
            "plugin manifest added".into()
        )));
    }

    #[test]
    fn ac4_10_nothing_risky_has_no_flags() {
        let got = flags(&[
            change("README.md", file("v1"), file("v2")),
            change(
                "skills/s/SKILL.md",
                file("---\nname: s\n---\nold"),
                file("---\nname: s\n---\nnew"),
            ),
            change("scripts/old.sh", exe("echo"), None),
            change("hooks/hooks.json", file("{}"), None),
        ]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn ec4_21_a_symlink_leaving_a_skill_is_flagged_outside() {
        let got = kinds(&[change("SKILL.md.link", None, link("../../../.ssh/id_rsa"))]);
        assert_eq!(
            got,
            vec![(
                FlagKind::Symlink,
                "SKILL.md.link".into(),
                "links to ../../../.ssh/id_rsa; points outside".into()
            )]
        );
    }

    #[test]
    fn ec4_23_dot_dot_and_absolute_targets_are_only_reported() {
        assert!(points_outside("a/b", "../../x"));
        assert!(!points_outside("a/b", "../x"));
        assert!(!points_outside("a/b", "./c/../d"));
        assert!(points_outside("x", "/abs"));
        assert!(points_outside("x", ".."));
    }

    #[test]
    fn names_and_letters() {
        let all = [
            FlagKind::Hook,
            FlagKind::Mcp,
            FlagKind::Lsp,
            FlagKind::Executable,
            FlagKind::Script,
            FlagKind::Binary,
            FlagKind::Symlink,
            FlagKind::AllowedTools,
            FlagKind::InlineShell,
            FlagKind::Manifest,
        ];
        for k in all {
            assert_eq!(
                serde_json::to_value(k).unwrap(),
                serde_json::Value::String(k.as_str().into())
            );
        }
        let letters: String = [
            Status::Added,
            Status::Modified,
            Status::Deleted,
            Status::Renamed,
            Status::Mode,
        ]
        .iter()
        .map(|s| s.letter())
        .collect();
        assert_eq!(letters, "AMDRT");
        assert_eq!(frontmatter_key("no frontmatter", "x"), None);
        assert_eq!(
            frontmatter_key("---\nx: 3\n---\n", "x"),
            Some("Integer(3)".into())
        );
        assert_eq!(json_key(b"not json", "x"), None);
    }
}
