//! Cycle 9: solid. Saved checks of held versions, the TUI's saved status, the remaining commands
//! as core calls, and the carried follow-ups. Named after AC9-NN / EC9-NN.

mod support;

use serde_json::Value;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use support::Env;

fn machine() -> Env {
    let env = Env::new();
    env.native();
    env.kilo();
    env.codex();
    env.agents_dir();
    env.ok(&["init"]);
    env
}

/// Replaces the test PATH's git with a script that logs each call's arguments, then runs git.
/// Written to a temporary name and renamed, so it's never executed half written.
fn count_git(env: &Env) -> PathBuf {
    let log = env.home.parent().unwrap().join("git.log");
    let real = fs::canonicalize(env.bin.join("git")).unwrap();
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        real.display()
    );
    let tmp = env.bin.join(".git.tmp");
    fs::write(&tmp, script).unwrap();
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_file(env.bin.join("git")).unwrap();
    fs::rename(&tmp, env.bin.join("git")).unwrap();
    log
}

fn calls(log: &Path, needle: &str) -> usize {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(needle))
        .count()
}

fn reset(log: &Path) {
    let _ = fs::remove_file(log);
}

fn cache_dir(env: &Env) -> PathBuf {
    env.source.join(".state/cache/reviews")
}

fn cached(env: &Env, key: &str) -> PathBuf {
    cache_dir(env).join(format!("{}.json", key.replace('/', "__")))
}

/// A machine with superpowers added at v1 and a held update two commits ahead.
fn held_source() -> (Env, PathBuf, String) {
    let env = machine();
    let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.ok(&["add", "source", &url]);
    let skills = repo.join("skills/tidy");
    fs::create_dir_all(&skills).unwrap();
    fs::write(skills.join("SKILL.md"), "---\nname: tidy\n---\nTidy up.\n").unwrap();
    env.upstream_commit(&repo, "v2");
    let v3 = env.upstream_commit(&repo, "v3");
    (env, repo, v3)
}

/// A skill twg keeps in its own folder, taken over by colctl, with a change held by refresh.
fn held_skill(env: &Env) -> PathBuf {
    let origin = env.skill_dir(&env.home.join(".config/twg/skills"), "twg-jira", "v1");
    let skills = env.home.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink(&origin, skills.join("twg-jira")).unwrap();
    env.ok(&["adopt", "agents", "skills/twg-jira"]);
    fs::write(origin.join("SKILL.md"), "v2").unwrap();
    env.ok(&["refresh"]);
    origin
}

// ---------------------------------------------------------------- saved checks (T2)

#[test]
fn ac9_02_a_held_version_is_checked_once() {
    let (env, _, _) = held_source();
    let log = count_git(&env);
    env.ok(&["update"]);
    assert_eq!(calls(&log, "diff --raw"), 1, "update checks what it holds");
    assert!(cached(&env, "sources/superpowers").is_file());
    reset(&log);
    env.expect(1, &["status"]);
    env.run(&["review"]);
    env.run(&["review", "superpowers", "--json"]);
    env.expect(1, &["status", "--json"]);
    assert_eq!(
        calls(&log, "diff --raw"),
        0,
        "later commands read the saved check"
    );
}

#[test]
fn ac9_03_saved_and_fresh_checks_print_the_same() {
    let (env, _, _) = held_source();
    held_skill(&env);
    env.ok(&["update"]);
    let run = |args: &[&str]| env.code(args);
    let commands: [&[&str]; 5] = [
        &["status"],
        &["status", "--json"],
        &["review"],
        &["review", "--json"],
        &["review", "superpowers", "--json"],
    ];
    let warm: Vec<_> = commands.iter().map(|c| run(c)).collect();
    for (args, before) in commands.iter().zip(&warm) {
        fs::remove_dir_all(env.source.join(".state/cache")).unwrap();
        assert_eq!(&run(args), before, "{args:?}");
    }
}

#[test]
fn ac9_04_many_changed_files_are_read_by_one_git_process() {
    let (env, repo, _) = held_source();
    for i in 0..300 {
        let dir = repo.join(format!("skills/s{i}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("---\nname: s{i}\n---\n{i}\n")).unwrap();
    }
    env.upstream_commit(&repo, "v4");
    let log = count_git(&env);
    env.ok(&["update"]);
    assert!(
        calls(&log, "cat-file") <= 2,
        "blobs are read in one batch, not one process per file: {} calls",
        calls(&log, "cat-file")
    );
}

#[test]
#[ignore = "timing: run by the smoke test against a release build"]
fn ac9_04_status_and_review_answer_in_under_a_second() {
    let (env, repo, _) = held_source();
    for c in 0..60 {
        for i in 0..5 {
            let dir = repo.join(format!("skills/s{c}-{i}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("SKILL.md"), format!("{c}{i}")).unwrap();
        }
        env.upstream_commit(&repo, &format!("c{c}"));
    }
    env.ok(&["update"]);
    for args in [&["status"][..], &["review"][..]] {
        let t = std::time::Instant::now();
        env.run(args);
        assert!(
            t.elapsed().as_millis() < 1000,
            "{args:?}: {:?}",
            t.elapsed()
        );
    }
}

#[test]
fn ac9_05_a_changed_hold_is_checked_again() {
    let (env, repo, v3) = held_source();
    env.ok(&["update"]);
    let first: Value =
        serde_json::from_str(&fs::read_to_string(cached(&env, "sources/superpowers")).unwrap())
            .unwrap();
    assert_eq!(first["identity"]["to"], v3.as_str());
    let v4 = env.upstream_commit(&repo, "v4");
    let log = count_git(&env);
    env.ok(&["update"]);
    let second: Value =
        serde_json::from_str(&fs::read_to_string(cached(&env, "sources/superpowers")).unwrap())
            .unwrap();
    assert_eq!(second["identity"]["to"], v4.as_str());
    assert_eq!(calls(&log, "diff --raw"), 1);
    reset(&log);
    env.run(&["review"]);
    assert_eq!(calls(&log, "diff --raw"), 0);
}

#[test]
fn ac9_06_approve_and_reject_remove_the_saved_check() {
    let (env, _, _) = held_source();
    held_skill(&env);
    env.ok(&["update"]);
    env.run(&["review"]);
    assert!(cached(&env, "sources/superpowers").is_file());
    assert!(cached(&env, "skills/twg-jira").is_file());
    env.ok(&["approve", "superpowers", "-y"]);
    env.ok(&["reject", "skills/twg-jira", "-y"]);
    assert!(!cached(&env, "sources/superpowers").exists());
    assert!(!cached(&env, "skills/twg-jira").exists());
    let status = env.status_json();
    assert_eq!(
        status["quarantine"].as_array().map_or(0, Vec::len),
        0,
        "{status}"
    );
}

#[test]
fn ac9_07_a_changed_pin_is_never_served_from_the_cache() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    env.run(&["review"]);
    let config = env.config();
    let t: toml::Table = toml::from_str(&config).unwrap();
    let rev = t["source"][0]["rev"].as_str().unwrap().to_string();
    let other = "0123456789abcdef0123456789abcdef01234567";
    env.write_config(&config.replace(&rev, other));
    let (_, out, err) = env.code(&["review", "superpowers"]);
    assert!(
        format!("{out}{err}").contains("pin changed since it was held"),
        "{out}{err}"
    );
}

#[test]
fn ac9_07_an_edited_held_copy_is_never_served_from_the_cache() {
    let env = machine();
    held_skill(&env);
    env.run(&["review"]);
    assert!(cached(&env, "skills/twg-jira").is_file());
    let snap = env
        .source
        .join(".state/quarantine/skills/twg-jira/SKILL.md");
    fs::write(snap, "edited").unwrap();
    let (_, out, err) = env.code(&["review", "--json"]);
    assert!(
        format!("{out}{err}").contains("changed since it was held"),
        "{out}{err}"
    );
}

#[test]
fn ac9_08_a_check_from_another_colctl_or_other_rules_is_redone() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    let path = cached(&env, "sources/superpowers");
    let log = count_git(&env);
    for field in ["colctl", "rules"] {
        let mut v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        v[field] = if field == "colctl" {
            Value::from("0.0.1")
        } else {
            Value::from(0)
        };
        fs::write(&path, v.to_string()).unwrap();
        reset(&log);
        env.run(&["review"]);
        assert_eq!(calls(&log, "diff --raw"), 1, "{field}");
        let again: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_ne!(
            again[field], v[field],
            "{field}: the new check replaced the old"
        );
    }
}

#[test]
fn ac9_09_a_diff_is_never_saved() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    let out = String::from_utf8_lossy(&env.run(&["review", "superpowers"]).stdout).into_owned();
    assert!(out.contains("Tidy up."), "the diff is shown: {out}");
    for entry in fs::read_dir(cache_dir(&env)).unwrap().flatten() {
        let text = fs::read_to_string(entry.path()).unwrap();
        assert!(!text.contains("Tidy up."), "{}", entry.path().display());
        assert!(!text.contains("\"diff\""), "{}", entry.path().display());
    }
}

#[test]
fn ac9_10_the_cache_is_never_committed() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    env.run(&["review"]);
    env.ok(&["approve", "superpowers", "-y"]);
    let tracked = env.git(&["ls-files"]);
    assert!(!tracked.contains(".state"), "{tracked}");
}

#[test]
fn ec9_01_ec9_02_ec9_03_a_missing_broken_or_misnamed_check_is_a_miss() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    let path = cached(&env, "sources/superpowers");
    let good = fs::read_to_string(&path).unwrap();
    let log = count_git(&env);
    let mut misnamed: Value = serde_json::from_str(&good).unwrap();
    misnamed["key"] = Value::from("sources/other");
    for broken in [
        "".to_string(),
        "{not json".to_string(),
        good[..good.len() / 2].to_string(),
        misnamed.to_string(),
    ] {
        fs::write(&path, broken).unwrap();
        reset(&log);
        let (code, out, err) = env.code(&["review"]);
        assert_eq!(code, 1, "held, as before: {out}{err}");
        assert_eq!(calls(&log, "diff --raw"), 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), good, "rewritten");
    }
    fs::remove_dir_all(env.source.join(".state/cache")).unwrap();
    reset(&log);
    env.run(&["review"]);
    assert_eq!(calls(&log, "diff --raw"), 1);
    assert!(path.is_file());
}

#[test]
fn ec9_05_processes_checking_at_once_never_read_half_a_file() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    let path = cached(&env, "sources/superpowers");
    for _ in 0..3 {
        fs::remove_dir_all(env.source.join(".state/cache")).unwrap();
        let children: Vec<_> = (0..4)
            .map(|_| {
                let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_colctl"));
                c.env_clear()
                    .env("HOME", &env.home)
                    .env("PATH", &env.path)
                    .current_dir(&env.home)
                    .arg("review")
                    .stdout(std::process::Stdio::null());
                for key in [
                    "LLVM_PROFILE_FILE",
                    "CARGO_LLVM_COV",
                    "CARGO_LLVM_COV_TARGET_DIR",
                ] {
                    if let Some(v) = std::env::var_os(key) {
                        c.env(key, v);
                    }
                }
                c.spawn().unwrap()
            })
            .collect();
        for mut c in children {
            assert_eq!(c.wait().unwrap().code(), Some(1), "held, as always");
        }
        let text = fs::read_to_string(&path).unwrap();
        serde_json::from_str::<Value>(&text).expect("a whole entry");
    }
}

#[test]
fn ec9_08_a_hold_that_goes_back_is_saved_and_served() {
    let env = machine();
    let (repo, url, v1) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
    env.upstream_commit(&repo, "v2");
    env.upstream_commit(&repo, "v3");
    env.ok(&["add", "source", &url]);
    env.ok(&["update", "superpowers", "--rev", &v1]);
    let first = String::from_utf8_lossy(&env.run(&["review"]).stdout).into_owned();
    assert!(first.contains("goes back 2 commits"), "{first}");
    let log = count_git(&env);
    let again = String::from_utf8_lossy(&env.run(&["review"]).stdout).into_owned();
    assert_eq!(again, first);
    assert_eq!(calls(&log, "diff --raw"), 0, "served from the saved check");
}

#[test]
fn ec9_10_a_missing_held_commit_is_an_error_not_a_saved_check() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    assert!(cached(&env, "sources/superpowers").is_file());
    let clone = env.source.join("plugins/superpowers");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&clone)
            .args(args)
            .output()
            .unwrap()
    };
    git(&["update-ref", "-d", "refs/colctl/candidate"]);
    let (_, out, err) = env.code(&["review", "--json"]);
    assert!(format!("{out}{err}").contains("is missing"), "{out}{err}");
}

#[test]
fn ec9_04_an_unwritable_cache_only_costs_speed() {
    let (env, _, _) = held_source();
    let cache = env.source.join(".state/cache");
    let _ = fs::remove_dir_all(&cache);
    fs::write(&cache, "not a folder").unwrap();
    let out = env.ok(&["update"]);
    assert!(!out.contains("cache"), "{out}");
    let (code, out, err) = env.code(&["review"]);
    assert_eq!(code, 1, "held, as before");
    assert!(!format!("{out}{err}").contains("cache"), "{out}{err}");
    let (_, _, err) = env.code(&["review", "-v"]);
    assert!(err.contains("cache"), "-v notes it: {err}");
}

#[test]
fn ec9_09_checks_for_versions_no_longer_held_are_pruned() {
    let (env, _, _) = held_source();
    env.ok(&["update"]);
    let stray = cached(&env, "sources/gone");
    fs::write(&stray, "{}").unwrap();
    env.run(&["review"]);
    assert!(!stray.exists());
    assert!(cached(&env, "sources/superpowers").is_file());
}

// ---------------------------------------------------------------- the TUI opens on the last state (T3)

mod saved_status {
    use super::*;
    use colctl::core::status;
    use colctl::ctx::Ctx;
    use colctl::tui::app::{self, Cmd, Model, Msg, Options, update};
    use colctl::tui::view;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::ffi::OsString;

    const PLAIN: Options = Options {
        color: false,
        motion: false,
    };

    fn ctx(env: &Env) -> Ctx {
        Ctx::from_vars(&[
            ("HOME".into(), OsString::from(&env.home)),
            ("PATH".into(), env.path.clone()),
        ])
        .unwrap()
    }

    fn screen(m: &Model) -> String {
        let mut t = Terminal::new(TestBackend::new(m.width, m.height)).unwrap();
        t.draw(|f| view::draw(m, f)).unwrap();
        view::text(t.backend().buffer())
    }

    fn saved_file(env: &Env) -> PathBuf {
        env.source.join(".state/cache/status.json")
    }

    #[test]
    fn ac9_11_the_first_frame_is_the_last_known_state() {
        let env = machine();
        let c = ctx(&env);
        let snap = status::snapshot(&c, None).unwrap();
        assert!(!saved_file(&env).exists(), "colctl status writes nothing");
        status::save(&c, &snap);
        assert!(saved_file(&env).is_file());
        let (saved, _) = status::last(&c).expect("the saved status reads back");
        let mut m = Model::new(PLAIN, 100, 30);
        m.show_saved(saved, 180);
        m.start();
        let first = screen(&m);
        assert!(first.contains("kilo-claude"), "{first}");
        assert!(first.contains("as of 3 min ago"), "{first}");
        let fresh = status::snapshot(&c, None).unwrap();
        update(
            &mut m,
            Msg::Loaded(app::Loaded::Snapshot(Box::new(Ok(fresh)))),
        );
        assert!(!screen(&m).contains("as of"), "{}", screen(&m));
    }

    #[test]
    fn ac9_11_ages_read_plainly() {
        let env = machine();
        let c = ctx(&env);
        status::save(&c, &status::snapshot(&c, None).unwrap());
        for (secs, text) in [
            (5, "as of just now"),
            (59, "as of just now"),
            (60, "as of 1 min ago"),
            (7200, "as of 2 h ago"),
            (3 * 86400, "as of 3 days ago"),
        ] {
            let (saved, _) = status::last(&c).unwrap();
            let mut m = Model::new(PLAIN, 100, 30);
            m.show_saved(saved, secs);
            m.start();
            assert!(screen(&m).contains(text), "{secs}: {}", screen(&m));
        }
    }

    #[test]
    fn ac9_12_no_readable_saved_status_loads_as_before() {
        let env = machine();
        let c = ctx(&env);
        assert!(status::last(&c).is_none(), "nothing saved yet");
        status::save(&c, &status::snapshot(&c, None).unwrap());
        let good = fs::read_to_string(saved_file(&env)).unwrap();
        fs::write(saved_file(&env), "{oops").unwrap();
        assert!(status::last(&c).is_none(), "unparsable");
        let mut v: Value = serde_json::from_str(&good).unwrap();
        v["colctl"] = Value::from("0.0.1");
        fs::write(saved_file(&env), v.to_string()).unwrap();
        assert!(status::last(&c).is_none(), "another colctl's");
    }

    #[test]
    fn ac9_12_the_tui_saves_each_fresh_status() {
        let env = machine();
        let c = ctx(&env);
        let msgs = colctl::tui::worker::run_now(&c, &Cmd::Load(app::Load::Snapshot), 80);
        assert!(!msgs.is_empty());
        assert!(saved_file(&env).is_file());
    }

    #[test]
    fn ac9_13_writes_never_act_on_the_saved_status() {
        let env = machine();
        let c = ctx(&env);
        status::save(&c, &status::snapshot(&c, None).unwrap());
        let (saved, _) = status::last(&c).unwrap();
        let mut stale = Model::new(PLAIN, 100, 30);
        stale.show_saved(saved, 600);
        stale.start();
        let key = Msg::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        let cmds = update(&mut stale, key);
        assert!(
            cmds.iter().all(|c| !matches!(c, Cmd::Run(_))),
            "sync asks first: {cmds:?}"
        );
        let screen = screen(&stale);
        assert!(
            !screen.contains("as of") || screen.contains("colctl status"),
            "{screen}"
        );
    }

    #[test]
    fn ec9_06_an_update_while_the_tui_shows_a_saved_status() {
        let (env, _, _) = held_source();
        let c = ctx(&env);
        status::save(&c, &status::snapshot(&c, None).unwrap());
        let (saved, _) = status::last(&c).unwrap();
        assert!(saved.held.is_empty());
        let mut m = Model::new(PLAIN, 100, 30);
        m.show_saved(saved, 60);
        let start = m.start();
        env.ok(&["update"]);
        for cmd in start {
            for msg in colctl::tui::worker::run_now(&c, &cmd, 80) {
                update(&mut m, msg);
            }
        }
        assert!(m.saved.is_none(), "the fresh status replaced it");
        assert_eq!(m.snapshot.ready().unwrap().held.len(), 1);
        assert_eq!(m.held.ready().map(Vec::len), Some(1));
    }

    #[test]
    fn ec9_07_a_saved_status_from_the_future_reads_earlier() {
        let env = machine();
        let c = ctx(&env);
        status::save(&c, &status::snapshot(&c, None).unwrap());
        let mut v: Value =
            serde_json::from_str(&fs::read_to_string(saved_file(&env)).unwrap()).unwrap();
        v["saved_at"] = Value::from(u64::MAX / 2);
        fs::write(saved_file(&env), v.to_string()).unwrap();
        let (saved, age) = status::last(&c).unwrap();
        assert_eq!(age, None, "no age for a time ahead of now");
        let mut m = Model::new(PLAIN, 100, 30);
        m.show_saved_at(saved, age);
        m.start();
        assert!(screen(&m).contains("as of earlier"), "{}", screen(&m));
    }
}

// ---------------------------------------------------------------- core calls for the rest (T4)

mod core_calls {
    use super::*;
    use colctl::core::observer::Quiet;
    use colctl::core::run::{self, Job};
    use colctl::core::{adopt, discover};
    use colctl::ctx::Ctx;
    use std::ffi::OsString;

    fn ctx(env: &Env) -> Ctx {
        Ctx::from_vars(&[
            ("HOME".into(), OsString::from(&env.home)),
            ("PATH".into(), env.path.clone()),
            ("FAKE_CLAUDE_LOG".into(), OsString::from(&env.log)),
            ("FAKE_CODEX_LOG".into(), OsString::from(&env.codex_log)),
        ])
        .unwrap()
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Everything that matters after a write: config, the source tree, history and what each
    /// profile has. (The fake tools log only for the CLI's processes, not in-process runs.)
    fn digest(env: &Env) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            let Ok(list) = fs::read_dir(dir) else { return };
            for e in list.flatten() {
                let p = e.path();
                let rel = p.strip_prefix(base).unwrap().display().to_string();
                if rel.contains(".git") || rel.contains(".state") || rel.contains(".lock") {
                    continue;
                }
                let meta = fs::symlink_metadata(&p).unwrap();
                if meta.file_type().is_symlink() {
                    let to = fs::read_link(&p).unwrap().display().to_string();
                    out.push(format!(
                        "{rel} -> {}",
                        to.replace(&base.display().to_string(), "~")
                    ));
                } else if meta.is_dir() {
                    walk(&p, base, out);
                } else {
                    out.push(rel);
                }
            }
        }
        let mut out = vec![env.config()];
        walk(&env.home, &env.home, &mut out);
        out.sort();
        let subjects: Vec<String> = env
            .git(&["log", "--format=%s"])
            .lines()
            .map(String::from)
            .collect();
        out.extend(subjects);
        let home = env.home.parent().unwrap().display().to_string();
        // Commit ids and temp folders differ between two homes built the same way.
        let sha = |w: &str| w.len() == 40 && w.chars().all(|c| c.is_ascii_hexdigit());
        out.into_iter()
            .map(|l| {
                l.replace(&home, "[ROOT]")
                    .split(['"', ' ', '\n'])
                    .map(|w| if sha(w) { "[SHA]" } else { w })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    fn run_job(env: &Env, job: &Job) -> run::Report {
        run::run(&ctx(env), job, 100, Box::new(Quiet)).unwrap()
    }

    fn parity(make: impl Fn() -> Env, job: Job, args: &[&str]) {
        let (tui, cli) = (make(), make());
        let report = run_job(&tui, &job);
        assert!(!report.lines.is_empty(), "{job:?} reports");
        cli.run(args);
        let (a, b) = (digest(&tui), digest(&cli));
        let only_core: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        let only_cli: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        assert!(
            only_core.is_empty() && only_cli.is_empty(),
            "{job:?} vs {args:?}\ncore only: {only_core:#?}\nCLI only: {only_cli:#?}"
        );
        let words: Vec<&str> = args.iter().copied().filter(|a| *a != "-y").collect();
        assert_eq!(job.command(), format!("colctl {}", words.join(" ")));
    }

    fn with_skill() -> Env {
        let env = machine();
        env.source_skill("alpha");
        env.ok(&["sync"]);
        env
    }

    fn with_foreign() -> Env {
        let env = machine();
        env.skill_dir(&env.home.join(".claude/skills"), "mine", "mine");
        env.skill_dir(&env.home.join(".claude/skills"), "yours", "yours");
        env
    }

    fn claude_only() -> Env {
        let env = machine();
        let repo = env.home.parent().unwrap().join("upstreams").join("cli");
        write(&repo.join("skills/edge/SKILL.md"), "---\nname: edge\n---\n");
        let (_, url, head) = env.upstream("cli", true, false, Some("cli"));
        env.add_source_entry("cli", &url, &head, "");
        env.ok(&["sync"]);
        env
    }

    fn converted() -> Env {
        let env = claude_only();
        env.ok(&["convert", "cli"]);
        env
    }

    fn taken_over() -> Env {
        let env = machine();
        held_skill(&env);
        env.ok(&["reject", "--all"]);
        let origin = env.home.join(".config/twg/skills/twg-jira");
        fs::write(origin.join("SKILL.md"), "v3").unwrap();
        env
    }

    #[test]
    fn ac9_14_rm_matches_the_cli() {
        parity(
            with_skill,
            Job::Rm {
                kind: "skill".into(),
                name: "alpha".into(),
                codex: false,
            },
            &["rm", "skill", "alpha", "-y"],
        );
    }

    #[test]
    fn ac9_14_adopt_matches_the_cli() {
        parity(
            with_foreign,
            Job::Adopt {
                profile: "claude".into(),
                items: Vec::new(),
            },
            &["adopt", "claude"],
        );
        parity(
            with_foreign,
            Job::Adopt {
                profile: "claude".into(),
                items: vec!["skills/mine".into()],
            },
            &["adopt", "claude", "skills/mine"],
        );
    }

    #[test]
    fn ac9_14_convert_and_undo_match_the_cli() {
        parity(claude_only, Job::Convert("cli".into()), &["convert", "cli"]);
        parity(
            converted,
            Job::ConvertUndo("cli".into()),
            &["convert", "cli", "--undo"],
        );
    }

    #[test]
    fn ac9_14_refresh_matches_the_cli() {
        parity(taken_over, Job::Refresh(Vec::new()), &["refresh"]);
        parity(
            taken_over,
            Job::Refresh(vec!["skills/twg-jira".into()]),
            &["refresh", "skills/twg-jira"],
        );
    }

    #[test]
    fn ac9_14_marketplace_refresh_matches_the_cli() {
        parity(
            machine,
            Job::MarketplaceRefresh,
            &["marketplace", "refresh"],
        );
    }

    fn json(env: &Env, args: &[&str]) -> Value {
        serde_json::from_slice(&env.run(args).stdout).unwrap()
    }

    #[test]
    fn ac9_15_reads_return_what_json_prints() {
        let env = claude_only();
        let c = ctx(&env);
        let markets = serde_json::to_value(discover::marketplaces(&c).unwrap()).unwrap();
        assert_eq!(
            markets,
            json(&env, &["marketplace", "list", "--json"])["marketplaces"]
        );
        let catalog = discover::catalog(&c).unwrap();
        let devs = serde_json::to_value(discover::developers(&catalog)).unwrap();
        assert_eq!(devs, json(&env, &["developers", "--json"])["developers"]);
        let conns = discover::connectors(&c).unwrap();
        let cli = json(&env, &["connectors", "--json", "--limit", "0"]);
        let ids: Vec<String> = conns.iter().map(|p| p.ids[0].clone()).collect();
        let cli_ids: Vec<String> = cli["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["ids"][0].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, cli_ids);
    }

    #[test]
    fn ac9_16_adopt_says_what_it_would_take_first() {
        let env = with_foreign();
        let before = digest(&env);
        let list = adopt::candidates(&ctx(&env), "claude").unwrap();
        let keys: Vec<&str> = list.iter().map(|c| c.address.as_str()).collect();
        assert_eq!(keys, ["skills/mine", "skills/yours"]);
        assert!(list.iter().all(|c| c.action == "adopt"), "{list:?}");
        assert_eq!(digest(&env), before, "changes nothing");
        assert!(adopt::candidates(&ctx(&env), "nope").is_err());
    }

    #[test]
    fn ec9_11_an_item_gone_before_adopt_is_skipped_with_the_reason() {
        let env = with_foreign();
        fs::remove_dir_all(env.home.join(".claude/skills/yours")).unwrap();
        let report = run_job(
            &env,
            &Job::Adopt {
                profile: "claude".into(),
                items: vec!["skills/yours".into(), "skills/mine".into()],
            },
        );
        let text = report.lines.join("\n");
        assert!(text.contains("skills/yours"), "{text}");
        assert!(
            env.source.join("skills/mine").is_dir(),
            "the rest is adopted"
        );
    }

    #[test]
    fn ec9_13_marketplace_refresh_reports_each_tool() {
        let env = Env::without_claude();
        env.native();
        env.codex();
        env.ok(&["init"]);
        let report = run_job(&env, &Job::MarketplaceRefresh);
        let text = report.lines.join("\n");
        assert!(text.contains("claude"), "{text}");
        assert!(text.contains("codex"), "{text}");
    }
}

// ---------------------------------------------------------------- follow-ups (T7)

mod follow_ups {
    use super::*;

    /// A held update where 12 skills each declare allowed-tools, and one adds a hook.
    fn noisy() -> Env {
        let env = machine();
        let (repo, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
        env.ok(&["add", "source", &url]);
        for i in 0..12 {
            let dir = repo.join(format!("skills/s{i:02}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: s{i:02}\nallowed-tools: Read, Grep\n---\nDo {i}.\n"),
            )
            .unwrap();
        }
        let hooks = repo.join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        fs::write(hooks.join("hooks.json"), r#"{"hooks": {}}"#).unwrap();
        env.upstream_commit(&repo, "v2");
        env.ok(&["update"]);
        env
    }

    #[test]
    fn ac9_25_allowed_tools_flags_are_grouped() {
        let env = noisy();
        let out = String::from_utf8_lossy(&env.run(&["review", "superpowers"]).stdout).into_owned();
        let flags: String = out
            .split("Risk flags")
            .nth(1)
            .unwrap_or_default()
            .split("Commits")
            .next()
            .unwrap_or_default()
            .to_string();
        assert!(
            Env::words(&out).contains("12 files declare allowed-tools"),
            "{out}"
        );
        assert!(
            !flags.contains("skills/s03/SKILL.md"),
            "one line, not twelve: {flags}"
        );
        assert!(
            flags.contains("hooks.json"),
            "other flags stay per file: {flags}"
        );
        // -v names them all; --json keeps a flag per file.
        let verbose =
            String::from_utf8_lossy(&env.run(&["review", "superpowers", "-v"]).stdout).into_owned();
        assert!(verbose.contains("skills/s03/SKILL.md"), "{verbose}");
        let json: Value =
            serde_json::from_slice(&env.run(&["review", "superpowers", "--json"]).stdout).unwrap();
        let n = json.to_string().matches("\"allowed-tools\"").count();
        assert!(n >= 12, "a flag per file in --json: {n}");
    }

    #[test]
    fn ac9_25_grouping_leaves_the_recommendation_alone() {
        let env = noisy();
        let json: Value = serde_json::from_slice(&env.run(&["review", "--json"]).stdout).unwrap();
        let before = json.to_string();
        assert!(before.contains("recommendation"), "{before}");
        let text =
            String::from_utf8_lossy(&env.run(&["review", "superpowers"]).stdout).into_owned();
        assert!(
            text.contains("Look before approving") || text.contains("approve"),
            "{text}"
        );
    }

    #[test]
    fn ac9_26_verbose_paths_start_from_home() {
        let env = machine();
        let (_, url, _) = env.upstream("superpowers", true, true, Some("superpowers-dev"));
        env.ok(&["add", "source", &url]);
        let home = env.home.display().to_string();
        fs::remove_dir_all(env.home.join(".claude/plugins")).ok();
        let (_, out, err) = env.code(&["sync", "-v"]);
        let calls: Vec<&str> = out
            .lines()
            .chain(err.lines())
            .filter(|l| l.contains("plugin marketplace add") || l.contains("plugin install"))
            .collect();
        assert!(!calls.is_empty(), "{out}{err}");
        for l in calls {
            assert!(!l.contains(&home), "{l}");
        }
        assert!(out.contains("~/.colctl/plugins/superpowers"), "{out}");
    }

    #[test]
    fn ac9_27_a_busy_executable_is_retried() {
        use colctl::runner::spawn_retrying;
        let mut tries = 0;
        let r: std::io::Result<u8> = spawn_retrying(|| {
            tries += 1;
            if tries < 3 {
                Err(std::io::Error::from_raw_os_error(26))
            } else {
                Ok(7)
            }
        });
        assert_eq!(r.unwrap(), 7);
        assert_eq!(tries, 3);
        let mut tries = 0;
        let r: std::io::Result<u8> = spawn_retrying(|| {
            tries += 1;
            Err(std::io::Error::from_raw_os_error(2))
        });
        assert!(r.is_err());
        assert_eq!(tries, 1, "only ETXTBSY is retried");
    }

    #[test]
    fn ac9_29_sync_says_what_is_wrong_and_the_fix() {
        let env = machine();
        env.skill_dir(&env.home.join(".claude/skills"), "mine", "mine");
        let (_, out, _) = env.code(&["sync"]);
        let first = out.lines().next().unwrap_or_default();
        assert!(!first.contains("attention"), "{first}");
        assert!(first.contains("mine"), "names it: {first}");
        assert!(
            first.contains("colctl adopt claude skills/mine"),
            "and its fix: {first}"
        );
    }

    #[test]
    fn ac9_31_marketplace_alone_prints_its_help() {
        let env = machine();
        let (code, out, err) = env.code(&["marketplace"]);
        assert_eq!(code, 2);
        let all = format!("{out}{err}");
        assert!(all.contains("colctl marketplace list"), "{all}");
        assert!(!all.contains("error: \n"), "{all}");
    }
}
