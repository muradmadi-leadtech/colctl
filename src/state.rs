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
    /// Source plugins colctl installed here (cycle 3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, InstalledSource>,
}

/// A source plugin as installed in one harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledSource {
    pub id: String,
    pub marketplace: String,
    pub rev: String,
}

impl HarnessState {
    pub fn is_empty(&self) -> bool {
        self.generated.is_empty()
            && self.marketplaces.is_empty()
            && self.plugins.is_empty()
            && self.sources.is_empty()
    }
}

/// Where an adopted skill or agent came from, and the hash adopted or last refreshed (AC2-25).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub path: PathBuf,
    pub hash: String,
}

/// A held new version, waiting for `approve` or `reject` (cycle 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Candidate {
    /// A source plugin's commit, kept in its clone under [`crate::vault::CANDIDATE_REF`].
    Source(HeldCommit),
    /// A taken-over skill's snapshot in `.state/quarantine/skills/<name>`.
    Skill(HeldCopy),
    /// A taken-over agent's snapshot in `.state/quarantine/agents/<name>.md`.
    Agent(HeldCopy),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldCommit {
    /// The pin when it was held.
    pub from: String,
    /// The held commit.
    pub to: String,
    /// Unix seconds.
    pub held_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldCopy {
    pub origin: PathBuf,
    /// The source copy's hash when it was held.
    pub base: String,
    /// The snapshot's hash.
    pub hash: String,
    /// Unix seconds.
    pub held_at: u64,
}

impl Candidate {
    pub fn held_at(&self) -> u64 {
        match self {
            Candidate::Source(c) => c.held_at,
            Candidate::Skill(c) | Candidate::Agent(c) => c.held_at,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Candidate::Source(_) => "source",
            Candidate::Skill(_) => "skill",
            Candidate::Agent(_) => "agent",
        }
    }

    /// The version that `rejected` remembers: the commit, or the snapshot's hash.
    pub fn version(&self) -> &str {
        match self {
            Candidate::Source(c) => &c.to,
            Candidate::Skill(c) | Candidate::Agent(c) => &c.hash,
        }
    }

    fn is_valid(&self, key: &str) -> bool {
        let hex40 = |s: &str| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit());
        match self {
            Candidate::Source(c) => key.starts_with("sources/") && hex40(&c.from) && hex40(&c.to),
            Candidate::Skill(c) => key.starts_with("skills/") && c.origin.is_absolute(),
            Candidate::Agent(c) => key.starts_with("agents/") && c.origin.is_absolute(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u32,
    #[serde(default)]
    pub harnesses: BTreeMap<String, HarnessState>,
    /// Source item (`skills/<name>`, `agents/<name>.md`) → where it was taken over from.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub origins: BTreeMap<String, Origin>,
    /// Candidate key (`sources/<name>`, `skills/<name>`, `agents/<name>.md`) → the held version.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quarantine: BTreeMap<String, Candidate>,
    /// Candidate key → rejected commits or snapshot hashes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rejected: BTreeMap<String, BTreeSet<String>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            harnesses: BTreeMap::new(),
            origins: BTreeMap::new(),
            quarantine: BTreeMap::new(),
            rejected: BTreeMap::new(),
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
        self.rejected.retain(|_, r| !r.is_empty());
    }

    /// True when `version` of `key` was rejected.
    pub fn is_rejected(&self, key: &str, version: &str) -> bool {
        self.rejected.get(key).is_some_and(|r| r.contains(version))
    }

    /// Forgets everything held or rejected for `key` (when the item is removed).
    pub fn forget(&mut self, key: &str) {
        self.quarantine.remove(key);
        self.rejected.remove(key);
    }
}

pub fn state_dir(source: &Path) -> PathBuf {
    source.join(STATE_DIR)
}

/// Where a held skill or agent's snapshot lives: `.state/quarantine/<key>`, inside the git-ignored
/// state folder so nothing held is ever committed (AC4-02).
pub fn snapshot_path(source: &Path, key: &str) -> PathBuf {
    state_dir(source).join(QUARANTINE_DIR).join(key)
}

pub const QUARANTINE_DIR: &str = "quarantine";

/// Now, in Unix seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
        .filter(|s| s.quarantine.iter().all(|(k, c)| c.is_valid(k)))
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

    #[test]
    fn ec2_22_a_1_0_state_file_loads_unchanged() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        let v1 = r#"{
  "version": 1,
  "harnesses": {
    "kilo-claude": {
      "generated": { "CLAUDE.md": "sha256:abc" },
      "marketplaces": ["sadr"],
      "plugins": ["sadr@sadr"]
    }
  }
}
"#;
        fs::write(state_dir(t.path()).join(STATE_FILE), v1).unwrap();
        let mut s = load(t.path()).unwrap();
        assert_eq!(
            s.harness("kilo-claude").generated["CLAUDE.md"],
            "sha256:abc"
        );
        s.harness_mut("kilo-codex")
            .generated
            .insert("agents/rev.toml".into(), "sha256:def".into());
        save(t.path(), &s).unwrap();
        let again = load(t.path()).unwrap();
        assert_eq!(again.harness("kilo-claude").plugins.len(), 1);
        assert_eq!(
            again.harness("kilo-codex").generated["agents/rev.toml"],
            "sha256:def"
        );
    }

    #[test]
    fn ac4_01_older_state_files_load_and_new_fields_appear_only_when_used() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        let v12 = r#"{"version": 1, "harnesses": {"c": {"plugins": ["x@m"]}},
            "origins": {"skills/a": {"path": "/o", "hash": "sha256:1"}}}"#;
        fs::write(state_dir(t.path()).join(STATE_FILE), v12).unwrap();
        let mut s = load(t.path()).unwrap();
        assert!(s.quarantine.is_empty() && s.rejected.is_empty());
        save(t.path(), &s).unwrap();
        let text = fs::read_to_string(state_dir(t.path()).join(STATE_FILE)).unwrap();
        assert!(!text.contains("quarantine") && !text.contains("rejected"));

        let held = Candidate::Source(HeldCommit {
            from: "a".repeat(40),
            to: "b".repeat(40),
            held_at: 7,
        });
        let copy = Candidate::Skill(HeldCopy {
            origin: "/o/a".into(),
            base: "sha256:1".into(),
            hash: "sha256:2".into(),
            held_at: 8,
        });
        s.quarantine.insert("sources/sp".into(), held.clone());
        s.quarantine.insert("skills/a".into(), copy.clone());
        s.rejected
            .entry("sources/sp".into())
            .or_default()
            .insert("c".repeat(40));
        s.rejected.entry("skills/empty".into()).or_default();
        save(t.path(), &s).unwrap();
        let text = fs::read_to_string(state_dir(t.path()).join(STATE_FILE)).unwrap();
        assert!(text.contains(r#""kind": "source""#), "{text}");
        assert!(text.contains(r#""kind": "skill""#), "{text}");
        let again = load(t.path()).unwrap();
        assert_eq!(again.quarantine["sources/sp"], held);
        assert_eq!(again.quarantine["skills/a"], copy);
        assert!(again.is_rejected("sources/sp", &"c".repeat(40)));
        assert!(!again.is_rejected("sources/sp", &"b".repeat(40)));
        assert!(!again.rejected.contains_key("skills/empty"));
        assert_eq!(held.version(), "b".repeat(40));
        assert_eq!(copy.version(), "sha256:2");
        assert_eq!((held.kind(), held.held_at()), ("source", 7));
        assert_eq!((copy.kind(), copy.held_at()), ("skill", 8));

        let mut s = again;
        s.forget("sources/sp");
        assert!(!s.quarantine.contains_key("sources/sp") && !s.rejected.contains_key("sources/sp"));
        assert_eq!(
            snapshot_path(t.path(), "skills/a"),
            t.path().join(".state/quarantine/skills/a")
        );
        assert!(now() > 1_700_000_000);
    }

    #[test]
    fn ec4_04_a_malformed_quarantine_entry_is_corrupt_state() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        let file = state_dir(t.path()).join(STATE_FILE);
        let good = "a".repeat(40);
        for bad in [
            r#"{"sources/x": {"kind": "source", "from": "short", "to": "short", "held_at": 1}}"#.to_string(),
            r#"{"sources/x": {"kind": "plugin", "from": "a", "to": "b", "held_at": 1}}"#.to_string(),
            format!(r#"{{"skills/x": {{"kind": "source", "from": "{good}", "to": "{good}", "held_at": 1}}}}"#),
            r#"{"skills/x": {"kind": "skill", "origin": "relative", "base": "b", "hash": "h", "held_at": 1}}"#.to_string(),
            r#"{"agents/x.md": {"kind": "agent", "origin": "/o", "base": "b", "hash": "h", "held_at": 1, "extra": 1}}"#.to_string(),
        ] {
            fs::write(&file, format!(r#"{{"version": 1, "quarantine": {bad}}}"#)).unwrap();
            let e = load(t.path()).unwrap_err();
            assert_eq!(e.code, 2, "{bad}");
            assert!(e.message.contains("state is corrupt"), "{bad}");
        }
        fs::write(
            &file,
            r#"{"version": 1, "quarantine": {"agents/x.md": {"kind": "agent", "origin": "/o", "base": "b", "hash": "h", "held_at": 1}}}"#,
        )
        .unwrap();
        assert!(load(t.path()).is_ok());
    }

    #[test]
    fn ec3_17_a_1_1_state_file_loads_and_gains_sources() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(state_dir(t.path())).unwrap();
        let v11 = r#"{"version": 1, "harnesses": {"codex": {"plugins": ["x@openai-curated"]}},
            "origins": {"skills/a": {"path": "/o", "hash": "sha256:1"}}}"#;
        fs::write(state_dir(t.path()).join(STATE_FILE), v11).unwrap();
        let mut s = load(t.path()).unwrap();
        assert!(s.harness("codex").sources.is_empty());
        s.harness_mut("codex").sources.insert(
            "superpowers".into(),
            InstalledSource {
                id: "superpowers@superpowers-dev".into(),
                marketplace: "superpowers-dev".into(),
                rev: "a".repeat(40),
            },
        );
        save(t.path(), &s).unwrap();
        let again = load(t.path()).unwrap();
        assert_eq!(
            again.harness("codex").sources["superpowers"].marketplace,
            "superpowers-dev"
        );
        assert_eq!(again.origins.len(), 1);
    }
}
