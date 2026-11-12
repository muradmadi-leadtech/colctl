//! Runs external programs (`claude`, `git`) with an explicit environment, a timeout, and full
//! capture of stdout and stderr (AC-30, EC-25).

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A program call.
#[derive(Debug, Clone, Default)]
pub struct Cmd {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env_set: Vec<(String, String)>,
    pub env_remove: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

/// What a call produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Exit code; `None` if killed by a signal or the timeout.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

impl Cmd {
    pub fn new(program: impl Into<PathBuf>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            ..Self::default()
        }
    }

    /// The command as a person would type it, with the program's file name only.
    pub fn display(&self) -> String {
        let name = self
            .program
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::iter::once(name)
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Runs the command to completion or until the timeout, then kills it.
    pub fn run(&self) -> std::io::Result<Output> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in &self.env_remove {
            command.env_remove(name);
        }
        for (name, value) in &self.env_set {
            command.env(name, value);
        }
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        // Its own process group, so a timeout kills everything it started (EC-25).
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command.spawn()?;
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if self.timeout.is_some_and(|t| started.elapsed() >= t) {
                kill_group(child.id());
                let _ = child.kill();
                timed_out = true;
                break child.wait()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        Ok(Output {
            code: if timed_out { None } else { status.code() },
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
            timed_out,
        })
    }
}

fn kill_group(pgid: u32) {
    if let Some(kill) = ["/bin/kill", "/usr/bin/kill"]
        .iter()
        .find(|p| Path::new(p).exists())
    {
        let _ = Command::new(kill)
            .args(["-KILL", "--", &format!("-{pgid}")])
            .stderr(Stdio::null())
            .status();
    }
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        text
    })
}

/// Finds an executable called `name` on `path_var`.
pub fn find_program(name: &str, path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Cmd {
        Cmd::new("/bin/sh", &["-c", script])
    }

    #[test]
    fn captures_output_and_exit_code() {
        let out = sh("echo out; echo err >&2; exit 3").run().unwrap();
        assert_eq!(out.code, Some(3));
        assert_eq!(out.stdout, "out\n");
        assert_eq!(out.stderr, "err\n");
        assert!(!out.success());
        assert!(sh("true").run().unwrap().success());
    }

    #[test]
    fn env_is_set_and_removed() {
        let mut cmd = sh("printf '%s|%s' \"$KEEP\" \"${GONE-unset}\"");
        cmd.env_set.push(("KEEP".into(), "yes".into()));
        cmd.env_remove.push("GONE".into());
        cmd.cwd = Some(std::env::temp_dir());
        assert_eq!(cmd.run().unwrap().stdout, "yes|unset");
    }

    #[test]
    fn ec_25_timeout_kills_the_child() {
        let mut cmd = sh("sleep 5 & sleep 5; wait");
        cmd.timeout = Some(Duration::from_millis(100));
        let started = Instant::now();
        let out = cmd.run().unwrap();
        assert!(out.timed_out);
        assert_eq!(out.code, None);
        assert!(!out.success());
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn display_and_lookup() {
        assert_eq!(
            Cmd::new("/usr/bin/claude", &["plugin", "install", "x"]).display(),
            "claude plugin install x"
        );
        assert!(find_program("sh", OsStr::new("/nope:/bin")).is_some());
        assert!(find_program("definitely-not-here", OsStr::new("/bin")).is_none());
        assert!(Cmd::new("/no/such/program", &[]).run().is_err());
    }
}
