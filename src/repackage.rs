//! Cycle 7: a Codex package built from a plugin folder that only has Claude packaging
//! (docs/convert/interfaces.md § The package). [`build`] reads the folder and never writes to it;
//! [`write`] puts the package on disk, the same bytes for the same input, replacing any old one whole.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

const CLAUDE_MANIFEST: &str = ".claude-plugin/plugin.json";
const CODEX_MANIFEST: &str = ".codex-plugin/plugin.json";
const HOOKS: &str = "hooks/hooks.json";
/// Hook events Codex runs (https://learn.chatgpt.com/docs/hooks.md).
const CODEX_EVENTS: &[&str] = &[
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "SessionStart",
    "SessionEnd",
    "PreModelRequest",
    "PostModelRequest",
    "Interrupt",
];
/// Events whose matcher is a tool name.
const TOOL_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "PermissionRequest"];
/// Claude tools Codex has no equivalent of, so a matcher naming them never fires.
const NO_SUCH_TOOL: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "LS",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "NotebookRead",
    "TodoWrite",
    "MultiEdit",
    "Task",
    "ExitPlanMode",
];
/// Fields copied from Claude's manifest when present (AC7-01).
const COPIED: &[&str] = &[
    "name",
    "version",
    "description",
    "author",
    "homepage",
    "repository",
    "license",
    "keywords",
];

/// Something in the plugin Codex won't get (AC7-06, EC7-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCarried {
    pub kind: String,
    pub name: String,
}

/// A hook kept in the package, as written in the plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptHook {
    pub event: String,
    pub matcher: Option<String>,
    pub kind: String,
    pub command: String,
}

/// What the conversion kept, left out and couldn't carry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Hooks and matcher parts Codex never runs (AC7-05).
    pub never_runs: Vec<String>,
    pub not_carried: Vec<NotCarried>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Content {
    File(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    /// Relative, `/`-separated.
    path: String,
    content: Content,
    executable: bool,
}

/// A Codex package, ready to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub manifest: Value,
    /// Skill folder names, sorted.
    pub skills: Vec<String>,
    pub hooks: Vec<KeptHook>,
    pub report: Report,
    entries: Vec<Entry>,
}

impl Package {
    /// SHA-256 over every file's path, mode and content hash, in path order.
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        for e in &self.entries {
            let bytes = match &e.content {
                Content::Bytes(b) => b.clone(),
                Content::File(p) => fs::read(p).unwrap_or_default(),
            };
            h.update(e.path.as_bytes());
            h.update([0]);
            h.update(mode(e.executable).to_string().as_bytes());
            h.update([0]);
            h.update(crate::fsx::hash_bytes(&bytes).as_bytes());
            h.update(b"\n");
        }
        crate::fsx::hex(&h.finalize())
    }

    /// Paths in the package, sorted.
    pub fn files(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.path.as_str()).collect()
    }
}

fn mode(executable: bool) -> u32 {
    if executable { 0o755 } else { 0o644 }
}

/// `rel` as a path inside the plugin, or an error when it leaves it (EC7-08).
fn inside(rel: &str, what: &str) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for c in Path::new(rel).components() {
        match c {
            Component::CurDir => {}
            Component::Normal(p) => out.push(p),
            Component::ParentDir if out.pop() => {}
            _ => return Err(format!("the {what} path {rel:?} is outside the plugin")),
        }
    }
    Ok(out)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} doesn't parse: {e}", path.display()))
}

/// Relative path → the real file it reads from.
type Files = Vec<(String, PathBuf)>;

/// Files under `folder`, following links that stay inside it; links that leave it are reported.
fn walk(folder: &Path) -> Result<(Files, Vec<String>), String> {
    let root = fs::canonicalize(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut files = Vec::new();
    let mut leaving = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![(String::new(), root.clone())];
    while let Some((rel, dir)) = stack.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let mut names: Vec<_> = fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        names.sort();
        for name in names {
            let name = name.to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if path == ".git" || path == ".claude-plugin" {
                continue;
            }
            let full = dir.join(&name);
            let real = if crate::fsx::is_symlink(&full) {
                match fs::canonicalize(&full) {
                    Ok(t) if t.starts_with(&root) => t,
                    _ => {
                        leaving.push(path);
                        continue;
                    }
                }
            } else {
                full
            };
            if real.is_dir() {
                stack.push((path, real));
            } else if real.is_file() {
                files.push((path, real));
            }
        }
    }
    files.sort();
    leaving.sort();
    Ok((files, leaving))
}

/// Stems of the `.md` files directly in `dir` (a relative path in `files`).
fn md_names(files: &Files, dir: &str) -> Vec<String> {
    let prefix = format!("{}/", dir.trim_end_matches('/'));
    let mut out: Vec<String> = files
        .iter()
        .filter_map(|(p, _)| p.strip_prefix(&prefix))
        .filter(|rest| !rest.contains('/'))
        .filter_map(|rest| rest.strip_suffix(".md"))
        .map(str::to_string)
        .collect();
    out.sort();
    out
}

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// Hooks Codex runs, unchanged, from a Claude hooks document (AC7-04, AC7-05); the rest is reported.
/// Returns the document to write (none when nothing is left) and the kept hooks.
fn map_hooks(
    doc: &Value,
    file: &str,
    report: &mut Report,
) -> Result<(Option<Value>, Vec<KeptHook>), String> {
    let shape = |what: &str| format!("{file} has an unknown shape: {what}");
    let events = doc
        .get("hooks")
        .and_then(Value::as_object)
        .ok_or_else(|| shape("\"hooks\" isn't an object"))?;
    let mut out_events = Map::new();
    let mut kept = Vec::new();
    for (event, groups) in events {
        let groups = groups
            .as_array()
            .ok_or_else(|| shape(&format!("{event} isn't a list")))?;
        let mut out_groups = Vec::new();
        for group in groups {
            let matcher = match group.get("matcher") {
                None | Some(Value::Null) => None,
                Some(Value::String(m)) => Some(m.clone()),
                Some(_) => return Err(shape(&format!("a {event} matcher isn't text"))),
            };
            let handlers = group
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| shape(&format!("a {event} group has no hooks list")))?;
            if !CODEX_EVENTS.contains(&event.as_str()) {
                continue;
            }
            // Tool names Codex never fires come out of a plain `A|B` matcher.
            let mut matcher_out = matcher.clone();
            if let Some(m) = &matcher
                && TOOL_EVENTS.contains(&event.as_str())
                && m.split('|').all(|p| {
                    !p.is_empty()
                        && p.chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '*')
                })
            {
                let (gone, left): (Vec<&str>, Vec<&str>) =
                    m.split('|').partition(|p| NO_SUCH_TOOL.contains(p));
                if left.is_empty() {
                    report
                        .never_runs
                        .push(format!("{event} on {m} (Codex has no such tools)"));
                    continue;
                }
                if !gone.is_empty() {
                    report
                        .never_runs
                        .push(format!("{} in {event}'s matcher", gone.join(", ")));
                    matcher_out = Some(left.join("|"));
                }
            }
            let mut out_handlers = Vec::new();
            for h in handlers {
                let kind = h
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| shape(&format!("a {event} hook has no type")))?;
                if kind != "command" {
                    report.never_runs.push(format!(
                        "{kind} hook in {event} (Codex runs command hooks only)"
                    ));
                    continue;
                }
                kept.push(KeptHook {
                    event: event.clone(),
                    matcher: matcher_out.clone(),
                    kind: kind.to_string(),
                    command: h
                        .get("command")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                });
                out_handlers.push(h.clone());
            }
            if out_handlers.is_empty() {
                continue;
            }
            let mut g = group.clone();
            g["hooks"] = Value::Array(out_handlers);
            if let Some(m) = &matcher_out {
                g["matcher"] = json!(m);
            }
            out_groups.push(g);
        }
        if !CODEX_EVENTS.contains(&event.as_str()) {
            report
                .never_runs
                .push(format!("{event} hook (Codex has no such event)"));
            continue;
        }
        if !out_groups.is_empty() {
            out_events.insert(event.clone(), Value::Array(out_groups));
        }
    }
    if out_events.is_empty() {
        return Ok((None, kept));
    }
    let mut out = doc.clone();
    out["hooks"] = Value::Object(out_events);
    Ok((Some(out), kept))
}

/// Builds the Codex package for the plugin in `folder`. `fallback` names it when Claude's manifest
/// doesn't (EC7-07).
pub fn build(folder: &Path, fallback: &str) -> Result<Package, String> {
    let claude = folder.join(CLAUDE_MANIFEST);
    if !claude.is_file() {
        return Err(format!(
            "{} has no Claude packaging to convert",
            folder.display()
        ));
    }
    let source = read_json(&claude)?;
    let name = source
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string();
    let skills_rel = match source.get("skills").and_then(Value::as_str) {
        Some(s) => {
            inside(s, "skills")?;
            Some(s.to_string())
        }
        None => None,
    };
    let skills_dir = inside(skills_rel.as_deref().unwrap_or("./skills/"), "skills")?
        .display()
        .to_string();

    let (files, leaving) = walk(folder)?;

    let mut skills: Vec<String> = files
        .iter()
        .filter_map(|(p, _)| p.strip_prefix(&format!("{skills_dir}/")))
        .filter_map(|rest| rest.strip_suffix("/SKILL.md"))
        .filter(|name| !name.contains('/'))
        .map(str::to_string)
        .collect();
    skills.sort();

    let mut report = Report::default();
    let mut carry = |kind: &str, names: Vec<String>| {
        for n in names {
            report.not_carried.push(NotCarried {
                kind: kind.to_string(),
                name: n,
            });
        }
    };
    carry("command", md_names(&files, "commands"));
    carry("agent", md_names(&files, "agents"));
    let mcp = folder.join(".mcp.json");
    if mcp.is_file() {
        let v = read_json(&mcp)?;
        carry(
            "MCP server",
            keys(v.get("mcpServers").unwrap_or(&Value::Null)),
        );
    }
    let lsp = folder.join(".lsp.json");
    if lsp.is_file() {
        carry("LSP server", keys(&read_json(&lsp)?));
    }
    if let Some(styles) = source.get("outputStyles").and_then(Value::as_str) {
        let dir = inside(styles, "output styles")?.display().to_string();
        carry("output style", md_names(&files, &dir));
    }
    carry("link leaving the plugin", leaving);

    // Hooks: inline in the manifest, at the manifest's path, or hooks/hooks.json (EC7-18).
    let (hooks_doc, hooks_file) = match source.get("hooks") {
        Some(Value::Object(_)) => (source.get("hooks").cloned(), None),
        Some(Value::String(rel)) => {
            let rel = inside(rel, "hooks")?.display().to_string();
            (Some(read_json(&folder.join(&rel))?), Some(rel))
        }
        _ if folder.join(HOOKS).is_file() => (
            Some(read_json(&folder.join(HOOKS))?),
            Some(HOOKS.to_string()),
        ),
        _ => (None, None),
    };
    let (hooks_out, hooks) = match hooks_doc {
        Some(doc) => map_hooks(
            &doc,
            hooks_file.as_deref().unwrap_or(CLAUDE_MANIFEST),
            &mut report,
        )?,
        None => (None, Vec::new()),
    };

    let mut manifest = Map::new();
    for key in COPIED {
        if let Some(v) = source.get(*key) {
            manifest.insert((*key).to_string(), v.clone());
        }
    }
    manifest.insert("name".into(), json!(name));
    if !skills.is_empty() {
        manifest.insert(
            "skills".into(),
            json!(skills_rel.unwrap_or_else(|| "./skills/".to_string())),
        );
    }
    manifest.insert("interface".into(), json!({"displayName": name}));
    let manifest = Value::Object(manifest);

    let mut entries: Vec<Entry> = files
        .into_iter()
        .filter(|(path, _)| Some(path.as_str()) != hooks_file.as_deref())
        .map(|(path, real)| {
            let executable = fs::metadata(&real)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
            Entry {
                path,
                content: Content::File(real),
                executable,
            }
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&manifest).expect("a JSON value serialises");
    text.push('\n');
    entries.push(Entry {
        path: CODEX_MANIFEST.to_string(),
        content: Content::Bytes(text.into_bytes()),
        executable: false,
    });
    if let Some(doc) = hooks_out {
        let mut text = serde_json::to_string_pretty(&doc).expect("a JSON value serialises");
        text.push('\n');
        entries.retain(|e| e.path != HOOKS);
        entries.push(Entry {
            path: HOOKS.to_string(),
            content: Content::Bytes(text.into_bytes()),
            executable: false,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(Package {
        manifest,
        skills,
        hooks,
        report,
        entries,
    })
}

/// The digest of a package written to `dir` ([`Package::digest`] of what's there), or `None` when
/// it's missing or can't be read.
pub fn digest_dir(dir: &Path) -> Option<String> {
    let mut files = Vec::new();
    let mut stack = vec![(String::new(), dir.to_path_buf())];
    while let Some((rel, d)) = stack.pop() {
        for e in fs::read_dir(&d).ok()? {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let meta = fs::symlink_metadata(e.path()).ok()?;
            if meta.is_dir() {
                stack.push((path, e.path()));
            } else {
                files.push((path, e.path(), meta.permissions().mode() & 0o111 != 0));
            }
        }
    }
    files.sort();
    let pkg = Package {
        manifest: Value::Null,
        skills: Vec::new(),
        hooks: Vec::new(),
        report: Report::default(),
        entries: files
            .into_iter()
            .map(|(path, real, executable)| Entry {
                path,
                content: Content::File(real),
                executable,
            })
            .collect(),
    };
    Some(pkg.digest())
}

/// Writes `pkg` to `dest`, replacing what's there, and returns its digest. It's built next to `dest`
/// and renamed into place, so a reader never sees half a package (EC7-19).
pub fn write(pkg: &Package, dest: &Path) -> Result<String, String> {
    let err = |e: std::io::Error| format!("writing {}: {e}", dest.display());
    let parent = dest.parent().ok_or("a package needs a parent folder")?;
    fs::create_dir_all(parent).map_err(err)?;
    let base = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = parent.join(format!(".tmp-{base}-{}", std::process::id()));
    let old = parent.join(format!(".old-{base}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    let built = (|| -> std::io::Result<()> {
        for e in &pkg.entries {
            let to = tmp.join(&e.path);
            let dir = to.parent().expect("an entry has a folder");
            fs::create_dir_all(dir)?;
            match &e.content {
                Content::Bytes(b) => fs::write(&to, b)?,
                Content::File(p) => {
                    fs::copy(p, &to)?;
                }
            }
            fs::set_permissions(&to, fs::Permissions::from_mode(mode(e.executable)))?;
        }
        Ok(())
    })();
    if let Err(e) = built {
        let _ = fs::remove_dir_all(&tmp);
        return Err(err(e));
    }
    if fs::symlink_metadata(dest).is_ok() {
        fs::rename(dest, &old).map_err(err)?;
    }
    fs::rename(&tmp, dest).map_err(err)?;
    let _ = fs::remove_dir_all(&old);
    Ok(pkg.digest())
}
