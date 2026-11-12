//! What a held candidate changes (cycle 4, interfaces.md § Git calls): the commits, the changed
//! files with their content, the unified diff, and the risk flags. Everything comes from `git` and
//! the files on disk; nothing is fetched or written.

use crate::git::Git;
use crate::risk::{self, Changed, Content, Flag, Status};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    pub rev: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    pub status: Status,
    pub old_path: Option<String>,
    pub executable: bool,
    pub symlink: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Forward,
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub direction: Direction,
    pub commits: Vec<Commit>,
    pub files: Vec<FileChange>,
    pub flags: Vec<Flag>,
    pub diff: Option<String>,
}

fn git_text(git: &Git, dir: &Path, args: &[&str]) -> Result<String, String> {
    git.bytes(dir, args, &[0])
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .map_err(|e| format!("{e:#}"))
}

/// A source plugin's candidate: `from` (the pin) to `to` (the held commit), in its clone.
pub fn commits(
    git: &Git,
    clone: &Path,
    from: &str,
    to: &str,
    with_diff: bool,
) -> Result<Review, String> {
    let ahead = git_text(
        git,
        clone,
        &["rev-list", "--count", &format!("{from}..{to}")],
    )?;
    let direction = if ahead.trim() == "0" {
        Direction::Back
    } else {
        Direction::Forward
    };
    let range = match direction {
        Direction::Forward => format!("{from}..{to}"),
        Direction::Back => format!("{to}..{from}"),
    };
    let log = git_text(
        git,
        clone,
        &["log", "--format=%H%x00%s%x00%an%x00%as", &range],
    )?;
    let commits = log
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\0');
            Some(Commit {
                rev: parts.next()?.to_string(),
                subject: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
            })
        })
        .collect();
    let raw = git
        .bytes(
            clone,
            &["diff", "--raw", "-z", "-M", "--no-abbrev", from, to],
            &[0],
        )
        .map_err(|e| format!("{e:#}"))?;
    let changed = parse_raw(&raw, |sha, mode| blob(git, clone, sha, mode))?;
    let diff = if with_diff {
        Some(git_text(git, clone, &["diff", "-M", from, to])?)
    } else {
        None
    };
    Ok(finish(direction, commits, changed, diff))
}

fn finish(
    direction: Direction,
    commits: Vec<Commit>,
    changed: Vec<Changed>,
    diff: Option<String>,
) -> Review {
    let flags = risk::flags(&changed);
    let files = changed
        .iter()
        .map(|c| FileChange {
            path: c.path.clone(),
            status: c.status,
            old_path: c.old_path.clone(),
            executable: matches!(
                c.new,
                Some(Content::File {
                    executable: true,
                    ..
                })
            ),
            symlink: match &c.new {
                Some(Content::Link(t)) => Some(t.clone()),
                _ => None,
            },
        })
        .collect();
    Review {
        direction,
        commits,
        files,
        flags,
        diff,
    }
}

/// One side of a raw diff entry: the blob's content, read by its mode.
fn blob(git: &Git, clone: &Path, sha: &str, mode: &str) -> Result<Option<Content>, String> {
    if sha.bytes().all(|b| b == b'0') || mode == "000000" {
        return Ok(None);
    }
    if mode == "160000" {
        return Ok(Some(Content::Link(format!("submodule {sha}"))));
    }
    let bytes = git
        .bytes(clone, &["cat-file", "blob", sha], &[0])
        .map_err(|e| format!("{e:#}"))?;
    Ok(Some(if mode == "120000" {
        Content::Link(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        Content::File {
            bytes,
            executable: mode == "100755",
        }
    }))
}

/// Parses `git diff --raw -z` output: `:<old mode> <new mode> <old sha> <new sha> <status>\0<path>\0`
/// (and a second path for renames and copies).
fn parse_raw(
    raw: &[u8],
    mut read: impl FnMut(&str, &str) -> Result<Option<Content>, String>,
) -> Result<Vec<Changed>, String> {
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let head = &fields[i];
        let Some(meta) = head.strip_prefix(':') else {
            i += 1;
            continue;
        };
        let parts: Vec<&str> = meta.split(' ').collect();
        let [old_mode, new_mode, old_sha, new_sha, status] = parts[..] else {
            return Err(format!("unexpected git diff output: {head}"));
        };
        let two_paths = status.starts_with('R') || status.starts_with('C');
        let path_at = |n: usize| fields.get(i + n).cloned().unwrap_or_default();
        let (old_path, path) = if two_paths {
            (Some(path_at(1)), path_at(2))
        } else {
            (None, path_at(1))
        };
        i += if two_paths { 3 } else { 2 };
        let status = match status.chars().next() {
            Some('A') => Status::Added,
            Some('D') => Status::Deleted,
            Some('R' | 'C') => Status::Renamed,
            Some('T') => Status::Mode,
            _ if old_mode != new_mode && old_sha == new_sha => Status::Mode,
            _ => Status::Modified,
        };
        out.push(Changed {
            path,
            old_path,
            status,
            old: read(old_sha, old_mode)?,
            new: read(new_sha, new_mode)?,
        });
    }
    Ok(out)
}

/// Every entry under `root` (or `root` itself, for a single file) by relative path, without following
/// symlinks.
fn entries(root: &Path, is_dir: bool) -> Result<BTreeMap<String, Content>, String> {
    let mut out = BTreeMap::new();
    if !is_dir {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.insert(name, read_entry(root)?);
        return Ok(out);
    }
    walk(root, "", &mut out)?;
    Ok(out)
}

fn walk(root: &Path, rel: &str, out: &mut BTreeMap<String, Content>) -> Result<(), String> {
    let dir = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let list = fs::read_dir(&dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    for entry in list.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let path = root.join(&rel);
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.is_dir() {
            walk(root, &rel, out)?;
        } else {
            out.insert(rel, read_entry(&path)?);
        }
    }
    Ok(())
}

fn read_entry(path: &Path) -> Result<Content, String> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(path).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(Content::Link(target.to_string_lossy().into_owned()));
    }
    Ok(Content::File {
        bytes: fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
        executable: meta.permissions().mode() & 0o111 != 0,
    })
}

/// A taken-over skill or agent's candidate: the source copy (`installed`) against the snapshot
/// (`held`). Paths are relative to the skill folder (or the agent's file name). The diff runs in
/// `source` so its headers show paths inside `~/.colctl`.
pub fn copies(
    git: &Git,
    source: &Path,
    installed: &Path,
    held: &Path,
    is_dir: bool,
    with_diff: bool,
) -> Result<Review, String> {
    let old = entries(installed, is_dir)?;
    let new = entries(held, is_dir)?;
    let mut changed = Vec::new();
    for path in old
        .keys()
        .chain(new.keys().filter(|k| !old.contains_key(*k)))
    {
        let (o, n) = (old.get(path), new.get(path));
        if o == n {
            continue;
        }
        let status = match (o, n) {
            (None, _) => Status::Added,
            (_, None) => Status::Deleted,
            (Some(Content::File { bytes: a, .. }), Some(Content::File { bytes: b, .. }))
                if a == b =>
            {
                Status::Mode
            }
            _ => Status::Modified,
        };
        changed.push(Changed {
            path: path.clone(),
            old_path: None,
            status,
            old: o.cloned(),
            new: n.cloned(),
        });
    }
    changed.sort_by(|a, b| a.path.cmp(&b.path));
    let diff = if with_diff {
        let rel = |p: &Path| {
            p.strip_prefix(source)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        };
        let (a, b) = (rel(installed), rel(held));
        let bytes = git
            .bytes(source, &["diff", "--no-index", "--", &a, &b], &[0, 1])
            .map_err(|e| format!("{e:#}"))?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        None
    };
    Ok(finish(Direction::Forward, Vec::new(), changed, diff))
}

/// Text from a candidate (file names, commit subjects, authors) made safe to print: control
/// characters and escapes are shown as `\xNN`/`\n`/`\t`, so they can't rewrite the terminal (EC4-22).
pub fn escape(text: &str) -> String {
    escape_keeping(text, &[])
}

/// Like [`escape`], but keeps newlines and tabs (for diff bodies).
pub fn escape_body(text: &str) -> String {
    escape_keeping(text, &['\n', '\t'])
}

fn escape_keeping(text: &str, keep: &[char]) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if keep.contains(&c) || !c.is_control() {
            out.push(c);
        } else if c == '\n' {
            out.push_str("\\n");
        } else if c == '\t' {
            out.push_str("\\t");
        } else {
            out.push_str(&format!("\\x{:02x}", c as u32));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec4_22_control_characters_are_escaped() {
        assert_eq!(escape("a\x1b[31mred\nx\ty"), "a\\x1b[31mred\\nx\\ty");
        assert_eq!(escape_body("+ok\n\tindent\x07\n"), "+ok\n\tindent\\x07\n");
        assert_eq!(escape("plain é"), "plain é");
        assert_eq!(escape("\u{9b}"), "\\x9b");
    }

    #[test]
    fn raw_output_parses_every_status() {
        let z = "0".repeat(40);
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let raw = format!(
            ":000000 100644 {z} {a} A\0new.txt\0:100644 100755 {a} {a} M\0mode\0:100644 000000 {a} {z} D\0gone\0:100644 100644 {a} {b} R090\0old\0renamed\0:100644 120000 {a} {b} T\0typed\0:160000 160000 {a} {b} M\0sub\0"
        );
        let got = parse_raw(raw.as_bytes(), |sha, mode| {
            Ok(if sha == z {
                None
            } else if mode == "160000" {
                Some(Content::Link("submodule".into()))
            } else {
                Some(Content::File {
                    bytes: sha.as_bytes()[..1].to_vec(),
                    executable: mode == "100755",
                })
            })
        })
        .unwrap();
        let st: Vec<(String, Status)> = got.iter().map(|c| (c.path.clone(), c.status)).collect();
        assert_eq!(
            st,
            vec![
                ("new.txt".into(), Status::Added),
                ("mode".into(), Status::Mode),
                ("gone".into(), Status::Deleted),
                ("renamed".into(), Status::Renamed),
                ("typed".into(), Status::Mode),
                ("sub".into(), Status::Modified),
            ]
        );
        assert_eq!(got[3].old_path.as_deref(), Some("old"));
        assert!(got[0].old.is_none() && got[2].new.is_none());
        assert!(parse_raw(b":bad\0x\0", |_, _| Ok(None)).is_err());
    }
}
