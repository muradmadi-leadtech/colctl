//! `colctl-setup`: the plugin colctl builds from `~/.colctl/hooks` (cycle 10, T6; interfaces.md §
//! Hooks). Claude has no CLI for hooks outside plugins and colctl never writes `settings.json`, so
//! hooks travel as a plugin of colctl's own, installed through the same path as source plugins.
//! Codex gets the same package through cycle 7's conversion.

use crate::fsx;
use crate::packaging::{self, ToolPackaging};
use crate::plan::{SourceInput, SourceTarget};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The folder in `~/.colctl` that holds `hooks.json` and the scripts it runs.
pub const HOOKS: &str = "hooks";
/// The hooks file, in Claude's plugin format.
pub const HOOKS_FILE: &str = "hooks.json";
/// How status names it, and the key it's recorded under in each profile.
pub const SOURCE: &str = "colctl-setup";
/// The plugin's name: with its marketplace, `setup@colctl-setup`.
pub const PLUGIN: &str = "setup";
/// Where colctl builds the packages, inside the (git-ignored) plugins folder.
const BUILT: &str = ".setup";

/// Every file in `dir` (recursively), as (path relative to `dir`, content, executable).
fn files(dir: &Path) -> std::io::Result<Vec<(PathBuf, Vec<u8>, bool)>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let p = e?.path();
            let meta = fs::symlink_metadata(&p)?;
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_file() {
                let rel = p.strip_prefix(dir).unwrap_or(&p).to_path_buf();
                out.push((rel, fs::read(&p)?, meta.permissions().mode() & 0o111 != 0));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The Claude package's files: each file of `hooks/` at the plugin root (so
/// `${CLAUDE_PLUGIN_ROOT}/<script>` finds it), `hooks.json` in `hooks/`, and a manifest whose
/// version is the content's hash, so a change reinstalls it (AC10-52).
fn package(hooks: &[(PathBuf, Vec<u8>, bool)]) -> (Vec<(PathBuf, Vec<u8>, bool)>, String) {
    let mut digest_input = Vec::new();
    for (rel, content, exec) in hooks {
        digest_input.extend(rel.to_string_lossy().as_bytes());
        digest_input.push(0);
        digest_input.extend(content);
        digest_input.push(u8::from(*exec));
    }
    let hash = fsx::hash_bytes(&digest_input);
    let short: String = hash
        .trim_start_matches("sha256:")
        .chars()
        .take(12)
        .collect();
    let manifest = serde_json::json!({
        "name": PLUGIN,
        "version": format!("0.0.0-{short}"),
        "description": "Hooks from ~/.colctl/hooks, kept by colctl",
        "author": {"name": "colctl"},
    });
    let mut out: Vec<(PathBuf, Vec<u8>, bool)> = hooks
        .iter()
        .map(|(rel, c, x)| {
            let to = if rel.as_os_str() == HOOKS_FILE {
                Path::new(HOOKS).join(HOOKS_FILE)
            } else {
                rel.clone()
            };
            (to, c.clone(), *x)
        })
        .collect();
    out.push((
        PathBuf::from(".claude-plugin/plugin.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).unwrap_or_default()
        )
        .into_bytes(),
        false,
    ));
    (out, short)
}

/// Writes the package to `dest` unless it's already there, replacing the folder whole.
fn write_package(dest: &Path, files: &[(PathBuf, Vec<u8>, bool)]) -> Result<(), String> {
    let manifest = dest.join(".claude-plugin/plugin.json");
    let want = files
        .iter()
        .find(|(r, ..)| r == Path::new(".claude-plugin/plugin.json"))
        .map(|(_, c, _)| c.clone());
    if fs::read(&manifest).ok() == want {
        return Ok(());
    }
    let err = |e: std::io::Error| format!("writing {}: {e}", dest.display());
    let tmp = dest.with_extension("tmp");
    let _ = fs::remove_dir_all(&tmp);
    for (rel, content, exec) in files {
        let to = tmp.join(rel);
        fs::create_dir_all(to.parent().unwrap_or(&tmp)).map_err(err)?;
        fs::write(&to, content).map_err(err)?;
        let mode = if *exec { 0o755 } else { 0o644 };
        fs::set_permissions(&to, fs::Permissions::from_mode(mode)).map_err(err)?;
    }
    let _ = fs::remove_dir_all(dest);
    fs::rename(&tmp, dest).map_err(err)
}

fn target(p: ToolPackaging) -> SourceTarget {
    match p {
        ToolPackaging::Supported(t) => SourceTarget::Ready(t),
        ToolPackaging::Unsupported(why) => SourceTarget::Unsupported(why),
        ToolPackaging::Invalid(why) => SourceTarget::Invalid(why),
    }
}

/// The `colctl-setup` plugin as a source to plan, or `None` when there are no hooks (AC10-54).
/// `write` builds the packages and their marketplace (sync); without it nothing is written, and a
/// package that isn't built yet plans as outdated.
pub fn input(source: &Path, plugins: &Path, manifests: &Path, write: bool) -> Option<SourceInput> {
    let dir = source.join(HOOKS);
    let file = dir.join(HOOKS_FILE);
    if !file.is_file() {
        return None;
    }
    let invalid = |why: String| SourceInput {
        name: SOURCE.into(),
        repo: source.display().to_string(),
        rev: "invalid".into(),
        replaces: Vec::new(),
        claude: SourceTarget::Invalid(why.clone()),
        codex: SourceTarget::Invalid(why),
        codex_rev: None,
    };
    // EC10-20: a hooks file that isn't Claude's format leaves what's installed as it was.
    let parsed = fs::read_to_string(&file)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()));
    match parsed {
        Ok(v) if v.get("hooks").is_some_and(serde_json::Value::is_object) => {}
        Ok(_) => {
            return Some(invalid(format!(
                "{} has no \"hooks\" object (Claude's plugin hooks format)",
                file.display()
            )));
        }
        Err(e) => return Some(invalid(format!("{} doesn't parse: {e}", file.display()))),
    }
    let hooks = match files(&dir) {
        Ok(f) => f,
        Err(e) => return Some(invalid(format!("reading {}: {e}", dir.display()))),
    };
    let (pkg, short) = package(&hooks);
    let built = plugins.join(BUILT);
    let claude_dir = built.join("claude");
    let codex_dir = built.join("codex");
    let rev = format!("hooks-{short}");
    // Status builds nothing: it plans from the content's hash alone.
    let staged;
    let claude_src = if write {
        if let Err(e) = write_package(&claude_dir, &pkg) {
            return Some(invalid(e));
        }
        claude_dir.clone()
    } else {
        staged = tempfile::tempdir().ok()?;
        let p = staged.path().join("claude");
        write_package(&p, &pkg).ok()?;
        p
    };
    let claude = target(packaging::wrap_claude(
        write,
        manifests,
        PLUGIN,
        &claude_dir,
        PLUGIN,
    ));
    let codex = match crate::repackage::build(&claude_src, PLUGIN) {
        Ok(p) if p.hooks.is_empty() => SourceTarget::Unsupported(
            "Codex runs none of these hooks (see colctl convert's rules)".into(),
        ),
        Ok(p) => {
            if write
                && crate::repackage::digest_dir(&codex_dir).as_deref() != Some(p.digest().as_str())
                && let Err(e) = crate::repackage::write(&p, &codex_dir)
            {
                return Some(invalid(e));
            }
            target(packaging::wrap_converted(
                write, manifests, PLUGIN, &codex_dir, PLUGIN,
            ))
        }
        Err(e) => SourceTarget::Invalid(e),
    };
    Some(SourceInput {
        name: SOURCE.into(),
        repo: source.display().to_string(),
        rev,
        replaces: Vec::new(),
        claude,
        codex,
        codex_rev: None,
    })
}
