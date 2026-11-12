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
pub const KILO_CODEX: &str = "Library/Application Support/kilo-proxy-headless/profiles/codex";

pub struct Env {
    _tmp: TempDir,
    pub home: PathBuf,
    pub source: PathBuf,
    pub bin: PathBuf,
    pub log: PathBuf,
    pub codex_log: PathBuf,
    pub path: OsString,
    pub vars: Vec<(String, String)>,
}

/// Builds `examples/<name>.rs` into this test run's target folder, once per run. Plain
/// `cargo test` builds examples anyway; coverage runs (`cargo llvm-cov`) don't.
fn fake(name: &'static str) -> PathBuf {
    static BUILT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let exe = std::env::current_exe().unwrap();
    let profile_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
    BUILT.get_or_init(|| {
        let target_dir = profile_dir.parent().unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = std::process::Command::new(cargo)
            .args(["build", "--quiet", "--locked", "--examples", "--target-dir"])
            .arg(target_dir)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("running cargo to build the fakes");
        assert!(status.success(), "building examples/ failed");
    });
    let fake = profile_dir.join("examples").join(name);
    assert!(fake.is_file(), "{}", fake.display());
    fake
}

fn upstream_git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new(real_git())
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

fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|d| d.join("git"))
        .find(|p| p.is_file())
        .expect("git on PATH")
}

impl Env {
    pub fn new() -> Self {
        Self::build(true, true, true)
    }

    pub fn without_claude() -> Self {
        Self::build(false, true, true)
    }

    pub fn without_codex() -> Self {
        Self::build(true, false, true)
    }

    pub fn without_git() -> Self {
        Self::build(true, true, false)
    }

    fn build(claude: bool, codex: bool, git: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let home = root.join("home");
        let bin = root.join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        if claude {
            symlink(fake("fake_claude"), bin.join("claude")).unwrap();
        }
        if codex {
            symlink(fake("fake_codex"), bin.join("codex")).unwrap();
        }
        if git {
            symlink(real_git(), bin.join("git")).unwrap();
        }
        Self {
            source: home.join(".colctl"),
            log: root.join("claude.log"),
            codex_log: root.join("codex.log"),
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

    /// Creates and returns the native Codex profile (`~/.codex`).
    pub fn codex(&self) -> PathBuf {
        let p = self.home.join(".codex");
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Creates and returns kilo's Codex profile.
    pub fn kilo_codex(&self) -> PathBuf {
        let p = self.home.join(KILO_CODEX);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Creates and returns the shared `~/.agents` folder.
    pub fn agents_dir(&self) -> PathBuf {
        let p = self.home.join(".agents");
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Plugin ids in a Codex profile's `config.toml`.
    pub fn codex_installed(&self, profile: &Path) -> Vec<String> {
        let Ok(text) = fs::read_to_string(profile.join("config.toml")) else {
            return Vec::new();
        };
        let t: toml::Table = toml::from_str(&text).unwrap();
        t.get("plugins")
            .and_then(|p| p.as_table())
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn codex_log(&self) -> Vec<Value> {
        fs::read_to_string(&self.codex_log)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    pub fn codex_log_args(&self) -> Vec<String> {
        self.codex_log()
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

    /// A local "upstream" plugin repo (reached by file://) with Claude and/or Codex packaging.
    /// With `marketplace`, the repo ships marketplace manifests under that name; without, plugin
    /// manifests only. Returns (path, url, head commit).
    pub fn upstream(
        &self,
        name: &str,
        claude: bool,
        codex: bool,
        marketplace: Option<&str>,
    ) -> (PathBuf, String, String) {
        let repo = self.home.parent().unwrap().join("upstreams").join(name);
        fs::create_dir_all(&repo).unwrap();
        let write = |rel: &str, value: serde_json::Value| {
            let path = repo.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value.to_string()).unwrap();
        };
        if claude {
            write(
                ".claude-plugin/plugin.json",
                serde_json::json!({"name": name}),
            );
            if let Some(m) = marketplace {
                write(
                    ".claude-plugin/marketplace.json",
                    serde_json::json!({"name": m, "plugins": [{"name": name, "source": "./"}]}),
                );
            }
        }
        if codex {
            write(
                ".codex-plugin/plugin.json",
                serde_json::json!({"name": name}),
            );
            if let Some(m) = marketplace {
                write(
                    ".agents/plugins/marketplace.json",
                    serde_json::json!({"name": m, "plugins": [{"name": name, "source": {"source": "url", "url": "./"}}]}),
                );
            }
        }
        fs::write(repo.join("README.md"), "v1").unwrap();
        upstream_git(&repo, &["init", "-q"]);
        let head = self.upstream_commit(&repo, "v1");
        let url = format!("file://{}", repo.display());
        (repo, url, head)
    }

    /// Commits a change in an upstream repo; returns the new head.
    pub fn upstream_commit(&self, repo: &Path, text: &str) -> String {
        fs::write(repo.join("README.md"), text).unwrap();
        upstream_git(repo, &["add", "-A"]);
        upstream_git(repo, &["commit", "-qm", text]);
        upstream_git(repo, &["rev-parse", "HEAD"])
    }

    /// Appends a [[source]] entry to config.toml.
    pub fn add_source_entry(&self, name: &str, url: &str, rev: &str, extra: &str) {
        let config = self.config();
        self.write_config(&format!(
            "{config}\n[[source]]\nname = \"{name}\"\nrepo = \"{url}\"\nrev = \"{rev}\"\n{extra}"
        ));
    }

    /// Replaces the harness list in config.toml with `(name, kind, path-as-written)` blocks,
    /// keeping the rest of the file.
    pub fn set_harnesses(&self, harnesses: &[(&str, &str, &str)]) {
        let config = self.config();
        let rest = config
            .split("\n[marketplaces]")
            .nth(1)
            .map(|r| format!("\n[marketplaces]{r}"))
            .unwrap_or_default();
        let mut text = String::from("version = 1\n");
        for (name, kind, path) in harnesses {
            text.push_str(&format!(
                "\n[[harness]]\nname = \"{name}\"\nkind = \"{kind}\"\npath = \"{path}\"\n"
            ));
        }
        text.push_str(&rest);
        self.write_config(&text);
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
            .env("FAKE_CODEX_LOG", &self.codex_log)
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
