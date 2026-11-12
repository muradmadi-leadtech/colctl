//! A throwaway machine for integration tests: a temp `HOME`, a `PATH` holding only the fake
//! `claude` (examples/fake_claude.rs) and the real `git`, and helpers to build harness folders.
//! Tests never see the real home directory or the real `claude`.

#![allow(dead_code)]

use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::TempDir;

pub const KILO: &str = "Library/Application Support/kilo-proxy-headless/profiles/claude";

pub struct Env {
    _tmp: TempDir,
    pub home: PathBuf,
    pub source: PathBuf,
    pub bin: PathBuf,
    pub log: PathBuf,
    pub path: OsString,
    pub vars: Vec<(String, String)>,
}

/// Builds examples/fake_claude.rs into this test run's target folder, once per run. Plain
/// `cargo test` builds examples anyway; coverage runs (`cargo llvm-cov`) don't.
fn fake_claude() -> PathBuf {
    static FAKE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    FAKE.get_or_init(|| {
        let exe = std::env::current_exe().unwrap();
        let profile_dir = exe.parent().unwrap().parent().unwrap();
        let target_dir = profile_dir.parent().unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = std::process::Command::new(cargo)
            .args([
                "build",
                "--quiet",
                "--locked",
                "--example",
                "fake_claude",
                "--target-dir",
            ])
            .arg(target_dir)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("running cargo to build the fake claude");
        assert!(status.success(), "building examples/fake_claude.rs failed");
        let fake = profile_dir.join("examples/fake_claude");
        assert!(fake.is_file(), "{}", fake.display());
        fake
    })
    .clone()
}

fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|d| d.join("git"))
        .find(|p| p.is_file())
        .expect("git on PATH")
}

impl Env {
    pub fn new() -> Self {
        Self::build(true, true)
    }

    pub fn without_claude() -> Self {
        Self::build(false, true)
    }

    pub fn without_git() -> Self {
        Self::build(true, false)
    }

    fn build(claude: bool, git: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let home = root.join("home");
        let bin = root.join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        if claude {
            symlink(fake_claude(), bin.join("claude")).unwrap();
        }
        if git {
            symlink(real_git(), bin.join("git")).unwrap();
        }
        Self {
            source: home.join(".colctl"),
            log: root.join("claude.log"),
            path: OsString::from(&bin),
            bin,
            home,
            vars: Vec::new(),
            _tmp: tmp,
        }
    }

    /// Creates and returns the native profile folder.
    pub fn native(&self) -> PathBuf {
        let p = self.home.join(".claude");
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Creates and returns the kilo profile folder.
    pub fn kilo(&self) -> PathBuf {
        let p = self.home.join(KILO);
        fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn var(mut self, key: &str, value: &str) -> Self {
        self.vars.push((key.into(), value.into()));
        self
    }

    pub fn cmd(&self) -> assert_cmd::Command {
        let mut c = assert_cmd::Command::new(env!("CARGO_BIN_EXE_colctl"));
        c.env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.path)
            .env("FAKE_CLAUDE_LOG", &self.log)
            .current_dir(&self.home);
        // Coverage runs write their profiles where this says.
        for key in [
            "LLVM_PROFILE_FILE",
            "CARGO_LLVM_COV",
            "CARGO_LLVM_COV_TARGET_DIR",
        ] {
            if let Some(v) = std::env::var_os(key) {
                c.env(key, v);
            }
        }
        for (k, v) in &self.vars {
            c.env(k, v);
        }
        c
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    /// Runs and returns (code, stdout, stderr).
    pub fn code(&self, args: &[&str]) -> (i32, String, String) {
        let out = self.run(args);
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// Runs, asserts the exit code, returns stdout.
    pub fn expect(&self, code: i32, args: &[&str]) -> String {
        let (got, out, err) = self.code(args);
        assert_eq!(got, code, "colctl {args:?}\nstdout:\n{out}\nstderr:\n{err}");
        out
    }

    pub fn ok(&self, args: &[&str]) -> String {
        self.expect(0, args)
    }

    pub fn status_json(&self) -> Value {
        let (_, out, err) = self.code(&["status", "--json"]);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}\n{err}"))
    }

    /// `(harness, kind, name) -> state` from `status --json`.
    pub fn item_state(&self, harness: &str, kind: &str, name: &str) -> Option<String> {
        let v = self.status_json();
        v["harnesses"]
            .as_array()?
            .iter()
            .find(|h| h["name"] == harness)?["items"]
            .as_array()?
            .iter()
            .find(|i| i["kind"] == kind && i["name"] == name)
            .map(|i| i["state"].as_str().unwrap().to_string())
    }

    /// Creates `<dir>/<name>/SKILL.md` with `body`.
    pub fn skill_dir(&self, dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("SKILL.md"), body).unwrap();
        p
    }

    /// Puts a skill straight into the source (as a user would edit it).
    pub fn source_skill(&self, name: &str) -> PathBuf {
        self.skill_dir(
            &self.source.join("skills"),
            name,
            &format!("---\nname: {name}\n---\n"),
        )
    }

    pub fn installed(&self, harness: &Path) -> Vec<String> {
        let path = harness.join("plugins/installed_plugins.json");
        let Ok(text) = fs::read_to_string(path) else {
            return Vec::new();
        };
        let v: Value = serde_json::from_str(&text).unwrap();
        v["plugins"].as_object().unwrap().keys().cloned().collect()
    }

    pub fn log(&self) -> Vec<Value> {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    pub fn log_args(&self) -> Vec<String> {
        self.log()
            .iter()
            .map(|l| {
                l["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    pub fn git(&self, args: &[&str]) -> String {
        let out = std::process::Command::new(real_git())
            .arg("-C")
            .arg(&self.source)
            .args(args)
            .env("HOME", &self.home)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn commits(&self) -> Vec<String> {
        self.git(&["log", "--format=%s"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// A fingerprint of everything under the temp home (AC-19).
    pub fn snapshot(&self) -> String {
        colctl::fsx::hash_tree(&self.home).unwrap()
    }

    pub fn config(&self) -> String {
        fs::read_to_string(self.source.join("config.toml")).unwrap()
    }

    pub fn write_config(&self, text: &str) {
        fs::write(self.source.join("config.toml"), text).unwrap();
    }
}
