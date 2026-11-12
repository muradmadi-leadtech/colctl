//! `.state/`: what colctl generated or installed in each harness, and the run lock
//! (interfaces.md § .state/state.json). Machine-specific, so it is git-ignored.

use crate::error::CliError;
use crate::fsx;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const STATE_DIR: &str = ".state";
const STATE_FILE: &str = "state.json";
const LOCK_FILE: &str = "lock";

/// Everything colctl recorded about one harness.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessState {
    /// Generated file (relative to the harness) → `sha256:` of what colctl wrote.
    #[serde(default)]
    pub generated: BTreeMap<String, String>,
    /// Marketplaces colctl added.
    #[serde(default)]
    pub marketplaces: BTreeSet<String>,
    /// Plugins colctl installed.
    #[serde(default)]
    pub plugins: BTreeSet<String>,
}

impl HarnessState {
    pub fn is_empty(&self) -> bool {
        self.generated.is_empty() && self.marketplaces.is_empty() && self.plugins.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u32,
    #[serde(default)]
    pub harnesses: BTreeMap<String, HarnessState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            harnesses: BTreeMap::new(),
        }
    }
}

impl State {
    pub fn harness(&self, name: &str) -> HarnessState {
        self.harnesses.get(name).cloned().unwrap_or_default()
    }

    pub fn harness_mut(&mut self, name: &str) -> &mut HarnessState {
        self.harnesses.entry(name.to_string()).or_default()
    }

    /// Drops harnesses with nothing recorded so the file stays small.
    pub fn prune(&mut self) {
        self.harnesses.retain(|_, h| !h.is_empty());
    }
}

pub fn state_dir(source: &Path) -> PathBuf {
    source.join(STATE_DIR)
}

/// Loads the state; missing is empty (EC-24), unreadable is an error (EC-23).
pub fn load(source: &Path) -> Result<State, CliError> {
    let path = state_dir(source).join(STATE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(err) => {
            return Err(CliError::usage(format!(
                "reading {}: {err}",
                path.display()
            )));
        }
    };
    serde_json::from_str::<State>(&text)
        .ok()
        .filter(|s| s.version == 1)
        .ok_or_else(|| {
            CliError::usage(format!(
                "state is corrupt at {}; move it aside and run colctl status (generated files will then show as foreign)",
                path.display()
            ))
        })
}

pub fn save(source: &Path, state: &State) -> anyhow::Result<()> {
    let mut state = state.clone();
    state.prune();
    let mut text = serde_json::to_string_pretty(&state)?;
    text.push('\n');
    fsx::atomic_write(&state_dir(source).join(STATE_FILE), text.as_bytes())
}

/// Held while a command that changes something runs; released on drop (AC-29).
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// Takes the lock, reclaiming it from a pid that is no longer running (EC-27).
    pub fn acquire(source: &Path) -> Result<Lock, CliError> {
        let dir = state_dir(source);
        fs::create_dir_all(&dir)
            .map_err(|e| CliError::usage(format!("creating {}: {e}", dir.display())))?;
        let path = dir.join(LOCK_FILE);
        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    write!(file, "{}", std::process::id())
                        .map_err(|e| CliError::usage(format!("writing {}: {e}", path.display())))?;
                    return Ok(Lock { path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = fs::read_to_string(&path).unwrap_or_default();
                    let pid = holder.trim().parse::<u32>().ok();
                    if let Some(pid) = pid.filter(|p| pid_alive(*p)) {
                        return Err(CliError::locked(format!(
                            "another colctl is running (pid {pid}); lock at {}",
                            path.display()
                        )));
                    }
                    eprintln!("colctl: reclaimed a stale lock from pid {}", holder.trim());
                    let _ = fs::remove_file(&path);
                }
                Err(err) => {
                    return Err(CliError::usage(format!(
                        "creating {}: {err}",
                        path.display()
                    )));
                }
            }
        }
        Err(CliError::locked(format!(
            "could not take the lock at {}",
            path.display()
        )))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// True when `pid` is a running process (checked with `kill -0`).
pub fn pid_alive(pid: u32) -> bool {
    ["/bin/kill", "/usr/bin/kill"]
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|kill| {
            Command::new(kill)
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec_24_missing_state_is_empty() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(load(t.path()).unwrap(), State::default());
    }

    #[test]
    fn state_round_trips_and_prunes() {
        let t = tempfile::tempdir().unwrap();
        let mut s = State::default();
        s.harness_mut("a").plugins.insert("p@m".into());
        s.harness_mut("empty");
        save(t.path(), &s).unwrap();
        let loaded = load(t.path()).unwrap();
        assert!(loaded.harnesses.contains_key("a"));
        assert!(!loaded.harnesses.contains_key("empty"));
        assert!(loaded.harness("a").plugins.contains("p@m"));
        assert!(loaded.harness("zzz").is_empty());
    }

    #[test]
    fn ec_23_corrupt_state_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        fs::write(state_dir(t.path()).join(STATE_FILE), "{ nope").unwrap();
        let e = load(t.path()).unwrap_err();
        assert_eq!(e.code, 2);
        assert!(e.message.contains("state is corrupt"));
        fs::write(state_dir(t.path()).join(STATE_FILE), r#"{"version": 9}"#).unwrap();
        assert!(load(t.path()).is_err());
        fs::remove_file(state_dir(t.path()).join(STATE_FILE)).unwrap();
        fs::create_dir(state_dir(t.path()).join(STATE_FILE)).unwrap();
        assert!(load(t.path()).unwrap_err().message.contains("reading"));
    }

    #[test]
    fn ac_29_second_lock_is_refused_while_held() {
        let t = tempfile::tempdir().unwrap();
        let lock = Lock::acquire(t.path()).unwrap();
        let e = Lock::acquire(t.path()).unwrap_err();
        assert_eq!(e.code, 3);
        assert!(e.message.contains(&std::process::id().to_string()));
        drop(lock);
        assert!(Lock::acquire(t.path()).is_ok());
    }

    #[test]
    fn ec_27_stale_lock_is_reclaimed() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        // A pid far above any real pid range.
        fs::write(state_dir(t.path()).join(LOCK_FILE), "999999999").unwrap();
        let lock = Lock::acquire(t.path()).unwrap();
        drop(lock);
        fs::write(state_dir(t.path()).join(LOCK_FILE), "garbage").unwrap();
        assert!(Lock::acquire(t.path()).is_ok());
    }

    #[test]
    fn lock_dir_errors_are_usage_errors() {
        let t = tempfile::tempdir().unwrap();
        fs::write(
            t.path().join(STATE_DIR),
            "a file where the folder should be",
        )
        .unwrap();
        assert_eq!(Lock::acquire(t.path()).unwrap_err().code, 2);
    }

    #[test]
    fn pid_alive_knows_this_process() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(999_999_999));
    }
}
