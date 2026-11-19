//! Filesystem primitives that are safe by construction: links and files are created under a
//! temporary name and renamed into place, and nothing here ever replaces a real file or folder
//! with a link.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Prefix of every temporary entry colctl creates; leftovers are removed on the next run (EC-21).
pub const TMP_PREFIX: &str = ".colctl-tmp-";

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `sha256:<hex>` of `bytes`.
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

/// `sha256:<hex>` of a file's content.
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(hash_bytes(&bytes))
}

/// A hash of a whole folder: every entry's relative path and kind, file contents, executable
/// bits and symlink targets. Two trees hash equal exactly when a copy would be indistinguishable.
pub fn hash_tree(root: &Path) -> Result<String> {
    let mut lines = Vec::new();
    walk(root, Path::new(""), &mut lines)?;
    lines.sort();
    Ok(hash_bytes(lines.join("\n").as_bytes()))
}

fn walk(root: &Path, rel: &Path, out: &mut Vec<String>) -> Result<()> {
    let dir = root.join(rel);
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let rel = rel.join(entry.file_name());
        let path = root.join(&rel);
        let meta = fs::symlink_metadata(&path)?;
        let shown = rel.display();
        if meta.file_type().is_symlink() {
            out.push(format!("l {shown} -> {}", fs::read_link(&path)?.display()));
        } else if meta.is_dir() {
            out.push(format!("d {shown}"));
            walk(root, &rel, out)?;
        } else {
            out.push(format!(
                "f {shown} {} {}",
                is_executable(&meta),
                hash_file(&path)?
            ));
        }
    }
    Ok(())
}

fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

/// Copies a folder recursively, keeping symlinks as symlinks. With `skip_git`, `.git` entries are
/// left out (used when adding a skill from a cloned repo).
pub fn copy_tree(from: &Path, to: &Path, skip_git: bool) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        if skip_git && entry.file_name() == ".git" {
            continue;
        }
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let meta = fs::symlink_metadata(&src)?;
        if meta.file_type().is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&src)?, &dst)?;
        } else if meta.is_dir() {
            copy_tree(&src, &dst, skip_git)?;
        } else {
            fs::copy(&src, &dst).with_context(|| format!("copying {}", src.display()))?;
        }
    }
    Ok(())
}

/// Makes `to` a real copy of `from` with every link resolved: a link to something inside `bound` is
/// copied as what it points to, and one that leads outside `bound`, dangles or loops back into the
/// tree is left out, as are `.git` entries. Claude refuses a plugin path that resolves outside the
/// plugin or its marketplace folder, so a plugin folder handed to it can't hold links. `to` (a link
/// or a folder from an earlier run) is replaced only when the copy differs, and atomically: a failed
/// copy leaves it as it was.
pub fn mirror_tree(from: &Path, to: &Path, bound: &Path) -> Result<()> {
    let bound = fs::canonicalize(bound).with_context(|| format!("reading {}", bound.display()))?;
    let root = fs::canonicalize(from).with_context(|| format!("reading {}", from.display()))?;
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let tmp = temp_sibling(to, "mirror-")?;
    if exists_any(&tmp) {
        remove_any(&tmp)?;
    }
    if let Err(e) = copy_resolved(&root, &tmp, &bound, &mut vec![root.clone()]) {
        let _ = remove_any(&tmp);
        return Err(e);
    }
    if !is_symlink(to) && to.is_dir() && hash_tree(to).ok() == hash_tree(&tmp).ok() {
        return remove_any(&tmp);
    }
    if !exists_any(to) {
        return fs::rename(&tmp, to)
            .with_context(|| format!("moving a copy into {}", to.display()));
    }
    let aside = temp_sibling(to, "old-")?;
    fs::rename(to, &aside).with_context(|| format!("moving {} aside", to.display()))?;
    if let Err(e) = fs::rename(&tmp, to) {
        fs::rename(&aside, to).ok();
        let _ = remove_any(&tmp);
        return Err(e).with_context(|| format!("moving a copy into {}", to.display()));
    }
    remove_any(&aside)
}

/// Copies the real folder `dir` into `to`, following links as [`mirror_tree`] describes. `stack`
/// holds the real folders being copied, so a link back into one of them is left out.
fn copy_resolved(dir: &Path, to: &Path, bound: &Path, stack: &mut Vec<PathBuf>) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let real = if is_symlink(&src) {
            match fs::canonicalize(&src) {
                Ok(real) if real.starts_with(bound) => real,
                _ => continue,
            }
        } else {
            src
        };
        if real.is_dir() {
            if stack.iter().any(|d| d.starts_with(&real)) {
                continue;
            }
            stack.push(real.clone());
            let copied = copy_resolved(&real, &dst, bound, stack);
            stack.pop();
            copied?;
        } else {
            fs::copy(&real, &dst).with_context(|| format!("copying {}", real.display()))?;
        }
    }
    Ok(())
}

/// True when `path` itself (not what it points to) is a symlink.
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// True when `path` exists in any form, including as a dangling symlink.
pub fn exists_any(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn temp_sibling(path: &Path, tag: &str) -> Result<PathBuf> {
    let parent = path.parent().context("path has no parent")?;
    let name = path.file_name().context("path has no file name")?;
    Ok(parent.join(format!(
        "{TMP_PREFIX}{tag}{}-{}",
        std::process::id(),
        name.to_string_lossy()
    )))
}

/// Points `link` at `target`, creating the parent folder if needed. An existing symlink at
/// `link` is replaced atomically; an existing real file or folder is never touched.
pub fn atomic_symlink(target: &Path, link: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    if exists_any(link) && !is_symlink(link) {
        bail!(
            "refusing to replace {}: it is not a symlink",
            link.display()
        );
    }
    let tmp = temp_sibling(link, "")?;
    let _ = fs::remove_file(&tmp);
    std::os::unix::fs::symlink(target, &tmp)
        .with_context(|| format!("creating a link at {}", tmp.display()))?;
    fs::rename(&tmp, link).with_context(|| format!("moving a link into {}", link.display()))?;
    Ok(())
}

/// Writes `bytes` to `path` through a temporary file and a rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    if is_symlink(path) || path.is_dir() {
        bail!(
            "refusing to replace {}: it is not a regular file",
            path.display()
        );
    }
    let tmp = temp_sibling(path, "")?;
    fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Removes `link`, which must be a symlink.
pub fn remove_link(link: &Path) -> Result<()> {
    if !is_symlink(link) {
        bail!("refusing to remove {}: it is not a symlink", link.display());
    }
    fs::remove_file(link).with_context(|| format!("removing {}", link.display()))
}

/// Replaces the real file or folder at `item` with a link to `target`. The original is moved
/// aside first and restored if the link can't be created.
pub fn replace_with_link(item: &Path, target: &Path) -> Result<()> {
    let aside = temp_sibling(item, "adopt-")?;
    fs::rename(item, &aside).with_context(|| format!("moving {} aside", item.display()))?;
    if let Err(err) = std::os::unix::fs::symlink(target, item) {
        fs::rename(&aside, item).ok();
        return Err(err).with_context(|| format!("linking {}", item.display()));
    }
    remove_any(&aside)
}

/// Removes a file, link or folder.
pub fn remove_any(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("removing {}", path.display()))
}

/// Temporary entries left directly inside `dir` by an interrupted run.
pub fn find_temps(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(TMP_PREFIX))
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

/// Resolves `.` and `..` without touching the filesystem.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The canonical form of `path` if it exists, otherwise its normalized form.
pub fn canonical_or_normal(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| normalize(path))
}

/// Where a symlink points, made absolute and normalized. `None` if `link` isn't a symlink.
pub fn link_target(link: &Path) -> Option<PathBuf> {
    let target = fs::read_link(link).ok()?;
    let absolute = if target.is_absolute() {
        target
    } else {
        link.parent()?.join(target)
    };
    Some(normalize(&absolute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn hashes_are_prefixed_hex() {
        assert_eq!(
            hash_bytes(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(hex(&[0, 255]), "00ff");
    }

    #[test]
    fn tree_hash_tracks_content_names_modes_and_links() {
        let t = tmp();
        let a = t.path().join("a");
        fs::create_dir_all(a.join("sub")).unwrap();
        fs::write(a.join("SKILL.md"), "x").unwrap();
        fs::write(a.join("sub/run.sh"), "echo").unwrap();
        symlink("SKILL.md", a.join("alias")).unwrap();
        let b = t.path().join("b");
        copy_tree(&a, &b, false).unwrap();
        assert_eq!(hash_tree(&a).unwrap(), hash_tree(&b).unwrap());
        assert!(is_symlink(&b.join("alias")));

        fs::set_permissions(b.join("sub/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_ne!(hash_tree(&a).unwrap(), hash_tree(&b).unwrap());
        fs::set_permissions(b.join("sub/run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(a.join("sub/run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(hash_tree(&a).unwrap(), hash_tree(&b).unwrap());
        fs::write(b.join("SKILL.md"), "y").unwrap();
        assert_ne!(hash_tree(&a).unwrap(), hash_tree(&b).unwrap());
        assert!(hash_tree(&t.path().join("missing")).is_err());
        assert!(hash_file(&t.path().join("missing")).is_err());
    }

    #[test]
    fn copy_tree_can_skip_git() {
        let t = tmp();
        let a = t.path().join("a");
        fs::create_dir_all(a.join(".git")).unwrap();
        fs::write(a.join("SKILL.md"), "x").unwrap();
        let b = t.path().join("b");
        copy_tree(&a, &b, true).unwrap();
        assert!(b.join("SKILL.md").is_file());
        assert!(!b.join(".git").exists());
        assert!(copy_tree(&t.path().join("missing"), &t.path().join("c"), false).is_err());
    }

    #[test]
    fn mirror_tree_resolves_links_inside_the_bound_and_drops_the_rest() {
        use std::os::unix::fs::MetadataExt;
        let t = tmp();
        let repo = t.path().join("repo");
        let plugin = repo.join("plugins/x/claude");
        fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        fs::write(plugin.join(".claude-plugin/plugin.json"), "{}").unwrap();
        fs::create_dir_all(repo.join("skills/a/scripts")).unwrap();
        fs::write(repo.join("skills/a/SKILL.md"), "a").unwrap();
        fs::write(repo.join("skills/a/scripts/run.sh"), "echo").unwrap();
        fs::set_permissions(
            repo.join("skills/a/scripts/run.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::write(repo.join("README.md"), "r").unwrap();
        fs::write(t.path().join("secret"), "s").unwrap();
        symlink("../../../skills", plugin.join("skills")).unwrap();
        symlink("../../../README.md", plugin.join("README.md")).unwrap();
        symlink(t.path().join("secret"), plugin.join("escapes")).unwrap();
        symlink("nowhere", plugin.join("dangling")).unwrap();
        symlink("..", plugin.join("loop")).unwrap();
        fs::create_dir_all(plugin.join(".git")).unwrap();

        let to = t.path().join("manifests/plugin");
        // An older colctl left a link here; it's replaced by a real folder.
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        symlink(&plugin, &to).unwrap();
        mirror_tree(&plugin, &to, &repo).unwrap();

        assert!(!is_symlink(&to) && to.is_dir());
        assert!(!is_symlink(&to.join("skills")));
        assert_eq!(fs::read(to.join("skills/a/SKILL.md")).unwrap(), b"a");
        let mode = fs::metadata(to.join("skills/a/scripts/run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "executable bits kept");
        assert!(!is_symlink(&to.join("README.md")) && to.join("README.md").is_file());
        for gone in ["escapes", "dangling", "loop", ".git"] {
            assert!(!exists_any(&to.join(gone)), "{gone} is left out");
        }
        assert!(find_temps(to.parent().unwrap()).is_empty());

        // Unchanged: the copy isn't rewritten. Changed: it's replaced.
        let ino = fs::metadata(&to).unwrap().ino();
        mirror_tree(&plugin, &to, &repo).unwrap();
        assert_eq!(fs::metadata(&to).unwrap().ino(), ino);
        fs::write(repo.join("skills/a/SKILL.md"), "b").unwrap();
        mirror_tree(&plugin, &to, &repo).unwrap();
        assert_eq!(fs::read(to.join("skills/a/SKILL.md")).unwrap(), b"b");
        assert!(find_temps(to.parent().unwrap()).is_empty());
        assert!(mirror_tree(&t.path().join("missing"), &to, &repo).is_err());
        assert!(
            to.join("skills/a/SKILL.md").is_file(),
            "a failed copy keeps the old one"
        );
    }

    #[test]
    fn atomic_symlink_creates_and_replaces_links_only() {
        let t = tmp();
        let link = t.path().join("skills/foo");
        atomic_symlink(Path::new("/target/one"), &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("/target/one"));
        atomic_symlink(Path::new("/target/two"), &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("/target/two"));
        assert!(find_temps(&t.path().join("skills")).is_empty());

        let real = t.path().join("skills/real");
        fs::create_dir_all(&real).unwrap();
        let err = atomic_symlink(Path::new("/x"), &real).unwrap_err();
        assert!(err.to_string().contains("not a symlink"));
        assert!(real.is_dir());
    }

    #[test]
    fn atomic_write_refuses_links_and_folders() {
        let t = tmp();
        let file = t.path().join("dir/CLAUDE.md");
        atomic_write(&file, b"one").unwrap();
        atomic_write(&file, b"two").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"two");
        let link = t.path().join("link.md");
        symlink(&file, &link).unwrap();
        assert!(atomic_write(&link, b"x").is_err());
        assert!(atomic_write(t.path(), b"x").is_err());
    }

    #[test]
    fn remove_link_refuses_real_files() {
        let t = tmp();
        let file = t.path().join("f");
        fs::write(&file, "x").unwrap();
        assert!(remove_link(&file).is_err());
        let link = t.path().join("l");
        symlink("/nowhere", &link).unwrap();
        assert!(exists_any(&link));
        remove_link(&link).unwrap();
        assert!(!exists_any(&link));
    }

    #[test]
    fn replace_with_link_moves_the_original_away() {
        let t = tmp();
        let item = t.path().join("skills/foo");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("SKILL.md"), "x").unwrap();
        replace_with_link(&item, Path::new("/src/skills/foo")).unwrap();
        assert_eq!(
            fs::read_link(&item).unwrap(),
            PathBuf::from("/src/skills/foo")
        );
        assert!(find_temps(&t.path().join("skills")).is_empty());
        assert!(replace_with_link(&t.path().join("missing"), Path::new("/x")).is_err());
    }

    #[test]
    fn ec_21_temps_are_found_and_removable() {
        let t = tmp();
        fs::create_dir_all(t.path().join(".colctl-tmp-adopt-1-foo")).unwrap();
        fs::write(t.path().join(".colctl-tmp-2-bar"), "x").unwrap();
        fs::write(t.path().join("keep"), "x").unwrap();
        let temps = find_temps(t.path());
        assert_eq!(temps.len(), 2);
        for path in &temps {
            remove_any(path).unwrap();
        }
        assert!(find_temps(t.path()).is_empty());
        assert!(t.path().join("keep").exists());
        assert!(find_temps(&t.path().join("missing")).is_empty());
        assert!(remove_any(&t.path().join("missing")).is_err());
    }

    #[test]
    fn paths_normalize_and_links_resolve() {
        assert_eq!(normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
        let t = tmp();
        let link = t.path().join("l");
        symlink("../x/y", &link).unwrap();
        assert_eq!(
            link_target(&link).unwrap(),
            normalize(&t.path().join("../x/y"))
        );
        assert!(link_target(&t.path().join("missing")).is_none());
        assert_eq!(
            canonical_or_normal(Path::new("/no/such/../path")),
            PathBuf::from("/no/path")
        );
        assert!(canonical_or_normal(t.path()).is_absolute());
    }
}
