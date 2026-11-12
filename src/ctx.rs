//! The environment a command runs in: home, source location, `PATH`, and the variables to scrub
//! before calling `claude`. Built once from the process so the rest of the code never reads
//! global state, which keeps it testable.

use crate::error::CliError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default time `claude` gets per call before it is killed (EC-25).
pub const DEFAULT_CLAUDE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct Ctx {
    /// The user's home directory.
    pub home: PathBuf,
    /// The source of truth: `$COLCTL_HOME` or `~/.colctl`.
    pub source: PathBuf,
    /// True when the source is the default `~/.colctl` (it is then written as `~/.colctl` in imports).
    pub default_source: bool,
    /// The `PATH` used to find `git` and `claude`.
    pub path_var: OsString,
    /// Names of every environment variable set for this process.
    pub env_keys: Vec<String>,
    /// How long one `claude` call may run.
    pub claude_timeout: Duration,
}

impl Ctx {
    /// Builds the context from explicit values; `vars` is the process environment.
    pub fn from_vars(vars: &[(String, OsString)]) -> Result<Self, CliError> {
        let get = |name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        let home = get("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| CliError::usage("HOME is not set"))?;
        let (source, default_source) = match get("COLCTL_HOME").filter(|v| !v.is_empty()) {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                if !dir.is_absolute() {
                    return Err(CliError::usage("COLCTL_HOME must be an absolute path"));
                }
                (dir, false)
            }
            None => (home.join(".colctl"), true),
        };
        let claude_timeout = get("COLCTL_CLAUDE_TIMEOUT_SECS")
            .and_then(|v| v.to_str().and_then(|s| s.parse::<u64>().ok()))
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_CLAUDE_TIMEOUT);
        Ok(Self {
            home,
            source,
            default_source,
            path_var: get("PATH").unwrap_or_default(),
            env_keys: vars.iter().map(|(k, _)| k.clone()).collect(),
            claude_timeout,
        })
    }

    /// Builds the context from the running process.
    pub fn from_process() -> Result<Self, CliError> {
        let vars: Vec<(String, OsString)> = std::env::vars_os()
            .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
            .collect();
        Self::from_vars(&vars)
    }

    /// The source as written in a generated `CLAUDE.md` import.
    pub fn source_display(&self) -> String {
        if self.default_source {
            "~/.colctl".to_string()
        } else {
            self.source.display().to_string()
        }
    }

    /// Writes `path` with the home directory shortened to `~`.
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect()
    }

    #[test]
    fn default_source_is_dot_colctl() {
        let ctx = Ctx::from_vars(&vars(&[("HOME", "/h"), ("PATH", "/bin")])).unwrap();
        assert_eq!(ctx.source, PathBuf::from("/h/.colctl"));
        assert!(ctx.default_source);
        assert_eq!(ctx.source_display(), "~/.colctl");
        assert_eq!(ctx.claude_timeout, DEFAULT_CLAUDE_TIMEOUT);
        assert_eq!(ctx.path_var, OsString::from("/bin"));
    }

    #[test]
    fn ac_03_colctl_home_overrides_source() {
        let ctx = Ctx::from_vars(&vars(&[("HOME", "/h"), ("COLCTL_HOME", "/some/dir")])).unwrap();
        assert_eq!(ctx.source, PathBuf::from("/some/dir"));
        assert!(!ctx.default_source);
        assert_eq!(ctx.source_display(), "/some/dir");
    }

    #[test]
    fn relative_colctl_home_and_missing_home_are_errors() {
        let err = Ctx::from_vars(&vars(&[("HOME", "/h"), ("COLCTL_HOME", "rel")])).unwrap_err();
        assert_eq!(err.code, 2);
        assert_eq!(Ctx::from_vars(&vars(&[])).unwrap_err().code, 2);
    }

    #[test]
    fn timeout_override_and_tilde() {
        let ctx = Ctx::from_vars(&vars(&[
            ("HOME", "/h"),
            ("COLCTL_CLAUDE_TIMEOUT_SECS", "2"),
        ]))
        .unwrap();
        assert_eq!(ctx.claude_timeout, Duration::from_secs(2));
        assert_eq!(ctx.tilde(Path::new("/h/.claude")), "~/.claude");
        assert_eq!(ctx.tilde(Path::new("/h")), "~");
        assert_eq!(ctx.tilde(Path::new("/other")), "/other");
        assert!(
            ctx.env_keys
                .contains(&"COLCTL_CLAUDE_TIMEOUT_SECS".to_string())
        );
    }

    #[test]
    fn from_process_reads_the_real_environment() {
        // HOME is set wherever the test suite runs.
        assert!(Ctx::from_process().is_ok());
    }
}
