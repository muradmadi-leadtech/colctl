//! The source's local git history (interfaces.md § Calling git). colctl only ever commits; it never
//! pushes, and the repo has no remote unless the user adds one.

use crate::error::CliError;
use crate::runner::{self, Cmd};
use anyhow::{Result, bail};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Git {
    program: PathBuf,
}

impl Git {
    /// Finds `git` on `path_var` (AC-32).
    pub fn find(path_var: &OsStr) -> Result<Self, CliError> {
        runner::find_program("git", path_var)
            .map(|program| Self { program })
            .ok_or_else(|| CliError::usage("colctl needs git, and git isn't on PATH"))
    }

    fn cmd(&self, dir: &Path, args: &[&str]) -> Cmd {
        let mut all = vec!["-C", dir.to_str().unwrap_or_default()];
        all.extend_from_slice(args);
        Cmd::new(&self.program, &all)
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Result<String> {
        let cmd = self.cmd(dir, args);
        let out = cmd.run()?;
        if !out.success() {
            bail!("{} failed: {}", cmd.display(), out.stderr.trim());
        }
        Ok(out.stdout)
    }

    pub fn is_repo(dir: &Path) -> bool {
        dir.join(".git").exists()
    }

    pub fn init(&self, dir: &Path) -> Result<()> {
        self.run(dir, &["init", "-q"]).map(|_| ())
    }

    /// Paths with uncommitted changes (ignored files excluded).
    pub fn dirty_paths(&self, dir: &Path) -> Result<Vec<String>> {
        let out = self.run(dir, &["status", "--porcelain", "--untracked-files=all"])?;
        Ok(out
            .lines()
            .filter(|l| l.len() > 3)
            .map(|l| l[3..].trim_matches('"').to_string())
            .collect())
    }

    fn has_identity(&self, dir: &Path) -> bool {
        ["user.name", "user.email"].iter().all(|key| {
            self.run(dir, &["config", key])
                .is_ok_and(|v| !v.trim().is_empty())
        })
    }

    /// Commits everything in `dir`. Returns false when there was nothing to commit. Without a
    /// configured identity it commits as `colctl <colctl@localhost>` (EC-26).
    pub fn commit_all(&self, dir: &Path, message: &str) -> Result<bool> {
        self.run(dir, &["add", "-A"])?;
        if self.dirty_paths(dir)?.is_empty() {
            return Ok(false);
        }
        let mut args: Vec<&str> = Vec::new();
        if !self.has_identity(dir) {
            args.extend([
                "-c",
                "user.name=colctl",
                "-c",
                "user.email=colctl@localhost",
            ]);
        }
        args.extend(["commit", "-q", "--no-verify", "-m", message]);
        self.run(dir, &args)?;
        Ok(true)
    }

    /// `git clone --depth 1 -- <url> <dir>`. The `--` keeps a URL that starts with `-` from being
    /// read as an option.
    pub fn shallow_clone(&self, url: &str, into: &Path) -> Result<()> {
        let parent = into.parent().unwrap_or(Path::new("/"));
        self.run(
            parent,
            &[
                "clone",
                "-q",
                "--depth",
                "1",
                "--",
                url,
                into.to_str().unwrap_or_default(),
            ],
        )
        .map(|_| ())
    }

    /// Commit subjects, newest first.
    pub fn log_subjects(&self, dir: &Path) -> Result<Vec<String>> {
        Ok(self
            .run(dir, &["log", "--format=%s"])?
            .lines()
            .map(str::to_string)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn git() -> Git {
        Git::find(&std::env::var_os("PATH").unwrap_or_default()).expect("git on PATH for tests")
    }

    #[test]
    fn ac_32_git_missing_is_a_usage_error() {
        let e = Git::find(&OsString::from("/definitely/not/here")).unwrap_err();
        assert_eq!(e.code, 2);
        assert!(e.message.contains("colctl needs git"));
    }

    #[test]
    fn ec_26_commits_without_a_configured_identity() {
        let t = tempfile::tempdir().unwrap();
        let g = git();
        g.init(t.path()).unwrap();
        assert!(Git::is_repo(t.path()));
        // A repo-local empty identity forces the fallback regardless of the machine's config.
        g.run(t.path(), &["config", "user.name", ""]).unwrap();
        g.run(t.path(), &["config", "user.email", ""]).unwrap();
        assert!(!g.commit_all(t.path(), "colctl: nothing").unwrap());
        std::fs::write(t.path().join("a.txt"), "x").unwrap();
        assert_eq!(g.dirty_paths(t.path()).unwrap(), vec!["a.txt"]);
        assert!(g.commit_all(t.path(), "colctl: init").unwrap());
        assert!(g.dirty_paths(t.path()).unwrap().is_empty());
        assert_eq!(g.log_subjects(t.path()).unwrap(), vec!["colctl: init"]);
        let author = g.run(t.path(), &["log", "--format=%an <%ae>"]).unwrap();
        assert_eq!(author.trim(), "colctl <colctl@localhost>");
    }

    #[test]
    fn errors_carry_stderr() {
        let t = tempfile::tempdir().unwrap();
        let e = git().dirty_paths(t.path()).unwrap_err().to_string();
        assert!(e.contains("git -C"), "{e}");
    }

    #[test]
    fn shallow_clone_copies_a_repo() {
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("origin");
        std::fs::create_dir(&origin).unwrap();
        let g = git();
        g.init(&origin).unwrap();
        std::fs::write(origin.join("SKILL.md"), "x").unwrap();
        g.commit_all(&origin, "first").unwrap();
        let into = t.path().join("clone");
        g.shallow_clone(&format!("file://{}", origin.display()), &into)
            .unwrap();
        assert!(into.join("SKILL.md").is_file());
    }

    #[test]
    fn shallow_clone_never_reads_the_url_as_an_option() {
        let t = tempfile::tempdir().unwrap();
        let marker = t.path().join("pwned");
        let url = format!("--upload-pack=touch {}; x://y", marker.display());
        let err = git()
            .shallow_clone(&url, &t.path().join("clone"))
            .unwrap_err()
            .to_string();
        assert!(!marker.exists(), "the url ran as an option: {err}");
        // git must treat the whole string as the repository URL, not as --upload-pack.
        assert!(err.contains("protocol '--upload-pack="), "{err}");
    }
}
