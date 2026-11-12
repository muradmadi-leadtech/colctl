//! The clones of source plugins in `~/.colctl/plugins/<name>`, kept at their pinned commit.
//! Only `update`, `adopt`, `add source` and a first clone reach the network (AC3-25).

use crate::fsx;
use crate::git::Git;
use std::ops::Not;
use std::path::{Path, PathBuf};

/// The state of a clone, read without touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloneState {
    Ok,
    /// Not cloned yet; sync will clone it.
    Missing,
    /// At another commit; sync will check out the pin.
    Moved {
        head: String,
    },
    /// Has modified or untracked files; never touched (AC3-06).
    Dirty,
    /// Not a usable clone of the declared repo (EC3-08).
    Invalid(String),
}

impl CloneState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CloneState::Ok => "ok",
            CloneState::Missing => "missing",
            CloneState::Moved { .. } => "moved",
            CloneState::Dirty => "dirty",
            CloneState::Invalid(_) => "invalid",
        }
    }
}

/// The folder holding every clone.
pub struct Vault<'a> {
    pub git: &'a Git,
    pub dir: PathBuf,
}

impl Vault<'_> {
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn git(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        self.git
            .run(dir, args)
            .map(|out| out.trim().to_string())
            .map_err(|e| format!("{e:#}"))
    }

    /// Reads a clone's state (offline).
    pub fn inspect(&self, name: &str, url: &str, rev: &str) -> CloneState {
        let path = self.path(name);
        if !fsx::exists_any(&path) {
            return CloneState::Missing;
        }
        if !is_clone(&path) {
            return CloneState::Invalid(format!("{} isn't a git clone", path.display()));
        }
        let origin = match self.git(&path, &["config", "--get", "remote.origin.url"]) {
            Ok(o) => o,
            Err(e) => return CloneState::Invalid(e),
        };
        if same_repo(&origin, url).not() {
            return CloneState::Invalid(format!(
                "{} is a clone, but its origin is {origin}, not {url}",
                path.display()
            ));
        }
        match self.git(&path, &["status", "--porcelain", "--untracked-files=all"]) {
            Ok(status) if !status.is_empty() => return CloneState::Dirty,
            Ok(_) => {}
            Err(e) => return CloneState::Invalid(e),
        }
        match self.git(&path, &["rev-parse", "HEAD"]) {
            Ok(head) if head == rev => CloneState::Ok,
            Ok(head) => CloneState::Moved { head },
            Err(e) => CloneState::Invalid(e),
        }
    }

    /// Clones if missing and checks out the pin, fetching if the commit isn't local yet.
    pub fn ensure(&self, name: &str, url: &str, rev: &str) -> Result<PathBuf, String> {
        let path = self.path(name);
        let fresh = match self.inspect(name, url, rev) {
            CloneState::Ok => return Ok(path),
            CloneState::Dirty => {
                return Err(format!("the clone at {} has local changes", path.display()));
            }
            CloneState::Invalid(reason) => return Err(reason),
            CloneState::Moved { .. } => false,
            CloneState::Missing => {
                std::fs::create_dir_all(&self.dir)
                    .map_err(|e| format!("creating {}: {e}", self.dir.display()))?;
                let dest = path.to_string_lossy().into_owned();
                if let Err(e) = self.git(&self.dir, &["clone", "-q", "--", url, &dest]) {
                    let _ = fsx::remove_any(&path);
                    return Err(format!("cloning {url}: {e}"));
                }
                true
            }
        };
        let result = self.checkout(&path, url, rev);
        if result.is_err() && fresh {
            let _ = fsx::remove_any(&path);
        }
        result.map(|()| path)
    }

    fn checkout(&self, path: &Path, url: &str, rev: &str) -> Result<(), String> {
        let has = |v: &Self| {
            v.git(path, &["cat-file", "-e", &format!("{rev}^{{commit}}")])
                .is_ok()
        };
        if !has(self) {
            self.git(path, &["fetch", "-q", "origin"])
                .map_err(|e| format!("fetching {url}: {e}"))?;
            if !has(self) {
                return Err(format!("rev {} not found in {url}", short(rev)));
            }
        }
        self.git(path, &["checkout", "-q", "--detach", rev])
            .map(|_| ())
            .map_err(|e| format!("checking out {}: {e}", short(rev)))
    }

    fn existing(&self, name: &str) -> Result<PathBuf, String> {
        let path = self.path(name);
        if is_clone(&path) {
            Ok(path)
        } else {
            Err(format!("{} isn't cloned", path.display()))
        }
    }

    /// Clones `url` if there's no clone yet, without checking anything out (for `add source`/`adopt`,
    /// before a pin is chosen).
    pub fn fetch_clone(&self, name: &str, url: &str) -> Result<PathBuf, String> {
        let path = self.path(name);
        if fsx::exists_any(&path) {
            return match self.inspect(name, url, "") {
                CloneState::Invalid(reason) => Err(reason),
                _ => Ok(path),
            };
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("creating {}: {e}", self.dir.display()))?;
        let dest = path.to_string_lossy().into_owned();
        if let Err(e) = self.git(&self.dir, &["clone", "-q", "--", url, &dest]) {
            let _ = fsx::remove_any(&path);
            return Err(format!("cloning {url}: {e}"));
        }
        Ok(path)
    }

    /// Fetches and returns the full commit of upstream's default-branch head.
    pub fn upstream_head(&self, name: &str) -> Result<String, String> {
        let path = self.existing(name)?;
        self.git(&path, &["fetch", "-q", "--tags", "origin"])?;
        if let Ok(head) = self.git(
            &path,
            &[
                "rev-parse",
                "--verify",
                "-q",
                "refs/remotes/origin/HEAD^{commit}",
            ],
        ) {
            return Ok(head);
        }
        self.git(&path, &["remote", "set-head", "origin", "-a"])?;
        self.git(
            &path,
            &[
                "rev-parse",
                "--verify",
                "-q",
                "refs/remotes/origin/HEAD^{commit}",
            ],
        )
    }

    /// Fetches and resolves a tag, branch or commit to a full commit.
    pub fn resolve(&self, name: &str, commitish: &str) -> Result<String, String> {
        let path = self.existing(name)?;
        self.git(&path, &["fetch", "-q", "--tags", "origin"])?;
        [
            format!("refs/remotes/origin/{commitish}"),
            commitish.to_string(),
        ]
        .iter()
        .find_map(|c| {
            self.git(
                &path,
                &["rev-parse", "--verify", "-q", &format!("{c}^{{commit}}")],
            )
            .ok()
            .filter(|s| !s.is_empty())
        })
        .ok_or_else(|| format!("{commitish} doesn't resolve to a commit in {name}"))
    }

    /// Commits from `old` to `new` (0 if unknown).
    pub fn count_between(&self, name: &str, old: &str, new: &str) -> usize {
        self.git(
            &self.path(name),
            &["rev-list", "--count", &format!("{old}..{new}")],
        )
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
    }

    /// Deletes a clone.
    pub fn remove(&self, name: &str) -> Result<(), String> {
        fsx::remove_any(&self.path(name)).map_err(|e| format!("{e:#}"))
    }
}

/// The first 7 characters of a commit.
pub fn short(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

fn same_repo(a: &str, b: &str) -> bool {
    let norm = |u: &str| u.trim_end_matches('/').trim_end_matches(".git").to_string();
    norm(a) == norm(b)
}

/// True when `path` looks like a git checkout.
pub fn is_clone(path: &Path) -> bool {
    path.join(".git").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// A local "upstream" repo with commits, plus a vault folder.
    struct Fixture {
        _t: tempfile::TempDir,
        upstream: PathBuf,
        url: String,
        git: Git,
        vault_dir: PathBuf,
    }

    fn sh_git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    impl Fixture {
        fn new() -> Self {
            let t = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(t.path()).unwrap();
            let upstream = root.join("upstream");
            fs::create_dir_all(&upstream).unwrap();
            sh_git(&upstream, &["init", "-q"]);
            let url = format!("file://{}", upstream.display());
            let git = Git::find(&std::env::var_os("PATH").unwrap()).unwrap();
            let f = Fixture {
                _t: t,
                upstream,
                url,
                git,
                vault_dir: root.join("plugins"),
            };
            f.commit("v1");
            f
        }

        fn commit(&self, text: &str) -> String {
            fs::write(self.upstream.join("README.md"), text).unwrap();
            sh_git(&self.upstream, &["add", "-A"]);
            sh_git(&self.upstream, &["commit", "-qm", text]);
            sh_git(&self.upstream, &["rev-parse", "HEAD"])
        }

        fn vault(&self) -> Vault<'_> {
            Vault {
                git: &self.git,
                dir: self.vault_dir.clone(),
            }
        }

        fn head(&self) -> String {
            sh_git(&self.vault().path("p"), &["rev-parse", "HEAD"])
        }
    }

    #[test]
    fn ac3_04_a_missing_clone_is_made_at_the_pin() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        f.commit("v2");
        assert_eq!(f.vault().inspect("p", &f.url, &v1), CloneState::Missing);
        let path = f.vault().ensure("p", &f.url, &v1).unwrap();
        assert_eq!(path, f.vault_dir.join("p"));
        assert_eq!(f.head(), v1);
        assert_eq!(fs::read_to_string(path.join("README.md")).unwrap(), "v1");
        assert_eq!(f.vault().inspect("p", &f.url, &v1), CloneState::Ok);
        assert!(is_clone(&path));
    }

    #[test]
    fn ac3_05_a_moved_clone_goes_back_to_the_pin_and_fetches_new_commits() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        f.vault().ensure("p", &f.url, &v1).unwrap();
        let v2 = f.commit("v2");
        // The pin moves to a commit the clone doesn't have yet.
        f.vault().ensure("p", &f.url, &v2).unwrap();
        assert_eq!(f.head(), v2);
        // Someone checks out another commit by hand.
        sh_git(&f.vault().path("p"), &["checkout", "-q", "--detach", &v1]);
        assert_eq!(
            f.vault().inspect("p", &f.url, &v2),
            CloneState::Moved { head: v1.clone() }
        );
        assert_eq!(CloneState::Moved { head: v1 }.as_str(), "moved");
        f.vault().ensure("p", &f.url, &v2).unwrap();
        assert_eq!(f.head(), v2);
    }

    #[test]
    fn ac3_06_a_dirty_clone_is_never_touched() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        let v2 = f.commit("v2");
        let path = f.vault().ensure("p", &f.url, &v1).unwrap();
        fs::write(path.join("README.md"), "my edit").unwrap();
        assert_eq!(f.vault().inspect("p", &f.url, &v2), CloneState::Dirty);
        let err = f.vault().ensure("p", &f.url, &v2).unwrap_err();
        assert!(err.contains("has local changes"), "{err}");
        assert_eq!(
            fs::read_to_string(path.join("README.md")).unwrap(),
            "my edit"
        );
        assert_eq!(f.head(), v1);
        // Untracked files count too.
        sh_git(&path, &["checkout", "-q", "--", "README.md"]);
        fs::write(path.join("new.txt"), "x").unwrap();
        assert_eq!(f.vault().inspect("p", &f.url, &v1), CloneState::Dirty);
    }

    #[test]
    fn ec3_02_a_rev_that_does_not_exist() {
        let f = Fixture::new();
        let missing = "0123456789abcdef0123456789abcdef01234567";
        let err = f.vault().ensure("p", &f.url, missing).unwrap_err();
        assert!(err.contains("rev 0123456 not found in"), "{err}");
    }

    #[test]
    fn ec3_03_an_unreachable_repo_leaves_no_half_clone() {
        let f = Fixture::new();
        let err = f
            .vault()
            .ensure("p", "file:///no/such/repo", &"a".repeat(40))
            .unwrap_err();
        assert!(err.contains("cloning file:///no/such/repo"), "{err}");
        assert!(!f.vault().path("p").exists());
    }

    #[test]
    fn ec3_08_a_folder_that_is_not_our_clone_is_left_alone() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        let path = f.vault().path("p");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("keep.txt"), "x").unwrap();
        assert!(
            matches!(f.vault().inspect("p", &f.url, &v1), CloneState::Invalid(r) if r.contains("isn't a git clone"))
        );
        assert!(f.vault().ensure("p", &f.url, &v1).is_err());
        assert!(path.join("keep.txt").is_file());

        fs::remove_dir_all(&path).unwrap();
        f.vault().ensure("p", &f.url, &v1).unwrap();
        let other = "file:///somewhere/else";
        assert!(
            matches!(f.vault().inspect("p", other, &v1), CloneState::Invalid(r) if r.contains("its origin is"))
        );
        assert!(f.vault().ensure("p", other, &v1).is_err());
    }

    #[test]
    fn ec3_14_a_failed_checkout_leaves_the_clone_where_it_was() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        let v2 = f.commit("v2");
        let path = f.vault().ensure("p", &f.url, &v2).unwrap();
        fs::write(path.join(".git/index.lock"), "").unwrap();
        let err = f.vault().ensure("p", &f.url, &v1).unwrap_err();
        assert!(err.contains("checking out"), "{err}");
        fs::remove_file(path.join(".git/index.lock")).unwrap();
        assert_eq!(f.head(), v2);
    }

    #[test]
    fn ec3_16_ac3_25_a_cloned_pin_needs_no_upstream() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        let v2 = f.commit("v2");
        f.vault().ensure("p", &f.url, &v2).unwrap();
        sh_git(&f.vault().path("p"), &["checkout", "-q", "--detach", &v1]);
        // Upstream disappears (or force-pushes the pin away); the pin is still local.
        fs::remove_dir_all(&f.upstream).unwrap();
        f.vault().ensure("p", &f.url, &v2).unwrap();
        assert_eq!(f.head(), v2);
        assert_eq!(f.vault().inspect("p", &f.url, &v2), CloneState::Ok);
    }

    #[test]
    fn upstream_head_resolve_and_count() {
        let f = Fixture::new();
        let v1 = sh_git(&f.upstream, &["rev-parse", "HEAD"]);
        f.vault().ensure("p", &f.url, &v1).unwrap();
        sh_git(&f.upstream, &["tag", "v1.0"]);
        f.commit("v2");
        let v3 = f.commit("v3");
        assert_eq!(f.vault().upstream_head("p").unwrap(), v3);
        assert_eq!(f.vault().count_between("p", &v1, &v3), 2);
        assert_eq!(f.vault().resolve("p", "v1.0").unwrap(), v1);
        assert_eq!(f.vault().resolve("p", "main").unwrap(), v3);
        assert_eq!(f.vault().resolve("p", &v3[..10]).unwrap(), v3);
        assert!(
            f.vault()
                .resolve("p", "no-such-tag")
                .unwrap_err()
                .contains("doesn't resolve")
        );
        assert_eq!(f.head(), v1, "looking upstream never moves the checkout");
        f.vault().remove("p").unwrap();
        assert!(!f.vault().path("p").exists());
        assert!(f.vault().upstream_head("p").is_err());
    }
}
