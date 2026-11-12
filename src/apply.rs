//! Executes a harness's planned actions. A failing action never stops the others (AC-30), and the
//! state only records what actually succeeded.

use crate::adapter::{CLAUDE_MD, read_plugins};
use crate::config::{Harness, split_plugin_id};
use crate::ctx::Ctx;
use crate::fsx;
use crate::plan::{Action, marketplace_arg};
use crate::runner::{self, Cmd};
use crate::state::HarnessState;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// What happened to one action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Failed(String),
    Skipped(String),
}

impl Outcome {
    pub fn is_done(&self) -> bool {
        matches!(self, Outcome::Done)
    }
}

/// One executed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub description: String,
    pub outcome: Outcome,
}

/// True when `harness` is the default Claude profile, which must run without `CLAUDE_CONFIG_DIR`.
pub fn is_native(ctx: &Ctx, harness: &Harness) -> bool {
    fsx::canonical_or_normal(&harness.path) == fsx::canonical_or_normal(&ctx.home.join(".claude"))
}

/// Builds a `claude` call for `harness` with exactly its own environment (EC-28).
pub fn claude_cmd(ctx: &Ctx, program: PathBuf, harness: &Harness, args: &[&str]) -> Cmd {
    let mut cmd = Cmd::new(program, args);
    cmd.env_remove = ["CLAUDE_CONFIG_DIR", "CLAUDE_SECURESTORAGE_CONFIG_DIR"]
        .iter()
        .map(|s| s.to_string())
        .chain(
            ctx.env_keys
                .iter()
                .filter(|k| k.starts_with("ANTHROPIC_"))
                .cloned(),
        )
        .collect();
    if !is_native(ctx, harness) {
        cmd.env_set.push((
            "CLAUDE_CONFIG_DIR".into(),
            harness.path.display().to_string(),
        ));
    }
    cmd.cwd = Some(ctx.home.clone());
    cmd.timeout = Some(ctx.claude_timeout);
    cmd
}

fn run_claude(ctx: &Ctx, claude: &Option<PathBuf>, harness: &Harness, args: &[&str]) -> Outcome {
    let Some(program) = claude else {
        return Outcome::Skipped("claude not found".into());
    };
    let cmd = claude_cmd(ctx, program.clone(), harness, args);
    match cmd.run() {
        Err(err) => Outcome::Failed(format!("{}: {err}", cmd.display())),
        Ok(out) if out.timed_out => Outcome::Failed(format!(
            "{} timed out after {}s",
            cmd.display(),
            ctx.claude_timeout.as_secs()
        )),
        Ok(out) if !out.success() => Outcome::Failed(format!(
            "{} exited {}: {}",
            cmd.display(),
            out.code.map_or("by signal".to_string(), |c| c.to_string()),
            out.stderr.trim()
        )),
        Ok(_) => Outcome::Done,
    }
}

fn file_outcome(result: anyhow::Result<()>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(err) => Outcome::Failed(format!("{err:#}")),
    }
}

/// Runs `actions` against `harness`, updating `state` for each success.
pub fn apply(
    ctx: &Ctx,
    harness: &Harness,
    actions: &[Action],
    state: &mut HarnessState,
) -> Vec<Applied> {
    let claude = if actions.iter().any(Action::needs_claude) {
        runner::find_program("claude", &ctx.path_var)
    } else {
        None
    };
    let mut failed_markets: BTreeSet<String> = BTreeSet::new();
    let mut results = Vec::new();
    for action in actions {
        let outcome = match action {
            Action::CleanupTemp(path) => file_outcome(fsx::remove_any(path)),
            Action::Link { link, target } => file_outcome(fsx::atomic_symlink(target, link)),
            Action::Unlink(link) => file_outcome(fsx::remove_link(link)),
            Action::WriteInstructions { path, content } => {
                let outcome = file_outcome(fsx::atomic_write(path, content));
                if outcome.is_done() {
                    state
                        .generated
                        .insert(CLAUDE_MD.into(), fsx::hash_bytes(content));
                }
                outcome
            }
            Action::RemoveInstructions(path) => {
                let outcome = file_outcome(fsx::remove_any(path));
                if outcome.is_done() {
                    state.generated.remove(CLAUDE_MD);
                }
                outcome
            }
            Action::RecordInstructions { hash } => {
                state.generated.insert(CLAUDE_MD.into(), hash.clone());
                Outcome::Done
            }
            Action::AddMarketplace { name, source } => {
                let before: BTreeSet<String> = read_plugins(&harness.path)
                    .map(|p| p.marketplaces.into_keys().collect())
                    .unwrap_or_default();
                let mut outcome = run_claude(
                    ctx,
                    &claude,
                    harness,
                    &["plugin", "marketplace", "add", marketplace_arg(source)],
                );
                if outcome.is_done() {
                    let after: BTreeSet<String> = read_plugins(&harness.path)
                        .map(|p| p.marketplaces.into_keys().collect())
                        .unwrap_or_default();
                    if after.contains(name) {
                        state.marketplaces.insert(name.clone());
                    } else {
                        let new: Vec<&String> = after.difference(&before).collect();
                        let called = if new.is_empty() {
                            "something else".to_string()
                        } else {
                            new.iter()
                                .map(|n| format!("\"{n}\""))
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        outcome = Outcome::Failed(format!(
                            "config calls it \"{name}\", the marketplace calls itself {called}; rename it in config.toml"
                        ));
                    }
                }
                if !outcome.is_done() {
                    failed_markets.insert(name.clone());
                }
                outcome
            }
            Action::Install { id } => {
                let market = split_plugin_id(id).map(|(_, m)| m).unwrap_or_default();
                if claude.is_some() && failed_markets.contains(market) {
                    Outcome::Skipped(format!("marketplace \"{market}\" wasn't added"))
                } else {
                    let outcome = run_claude(ctx, &claude, harness, &["plugin", "install", id]);
                    if outcome.is_done() {
                        state.plugins.insert(id.clone());
                    }
                    outcome
                }
            }
            Action::Uninstall { id } => {
                let outcome = run_claude(ctx, &claude, harness, &["plugin", "uninstall", id]);
                if outcome.is_done() {
                    state.plugins.remove(id);
                }
                outcome
            }
            Action::RemoveMarketplace { name } => {
                let outcome = run_claude(
                    ctx,
                    &claude,
                    harness,
                    &["plugin", "marketplace", "remove", name],
                );
                if outcome.is_done() {
                    state.marketplaces.remove(name);
                }
                outcome
            }
            Action::ForgetPlugin { id } => {
                state.plugins.remove(id);
                Outcome::Done
            }
            Action::ForgetMarketplace { name } => {
                state.marketplaces.remove(name);
                Outcome::Done
            }
        };
        results.push(Applied {
            description: action.describe(),
            outcome,
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Kind;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    struct Fixture {
        t: tempfile::TempDir,
        ctx: Ctx,
        harness: Harness,
    }

    /// A shell stand-in for `claude` that logs its args and env, and mimics plugin files.
    const FAKE: &str = r#"#!/bin/sh
dir="${CLAUDE_CONFIG_DIR:-@HOME@/.claude}"
echo "$* | dir=${CLAUDE_CONFIG_DIR-unset} | anth=${ANTHROPIC_API_KEY-unset}" >> "@HOME@/claude.log"
case "$*" in
  *fail*) echo "boom" >&2; exit 4 ;;
  *slow*) sleep 5 ;;
  "plugin marketplace add o/wrongname") mkdir -p "$dir/plugins"; echo '{"other":{}}' > "$dir/plugins/known_marketplaces.json" ;;
  "plugin marketplace add o/r") mkdir -p "$dir/plugins"; echo '{"m":{}}' > "$dir/plugins/known_marketplaces.json" ;;
esac
exit 0
"#;

    fn fixture(native: bool, with_claude: bool) -> Fixture {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let bin = t.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        if with_claude {
            fs::write(
                bin.join("claude"),
                FAKE.replace("@HOME@", home.to_str().unwrap()),
            )
            .unwrap();
            fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = if native {
            home.join(".claude")
        } else {
            home.join("kilo")
        };
        fs::create_dir_all(&path).unwrap();
        let vars = vec![
            ("HOME".to_string(), OsString::from(&home)),
            ("PATH".to_string(), OsString::from(&bin)),
            ("ANTHROPIC_API_KEY".to_string(), OsString::from("secret")),
            (
                "COLCTL_CLAUDE_TIMEOUT_SECS".to_string(),
                OsString::from("1"),
            ),
        ];
        let ctx = Ctx::from_vars(&vars).unwrap();
        let harness = Harness {
            name: "h".into(),
            kind: Kind::Claude,
            path,
            exclude: Default::default(),
        };
        Fixture { t, ctx, harness }
    }

    fn log(f: &Fixture) -> String {
        fs::read_to_string(f.ctx.home.join("claude.log")).unwrap_or_default()
    }

    #[test]
    fn file_actions_and_state() {
        let f = fixture(false, false);
        let src = f.t.path().join("src/skills/foo");
        fs::create_dir_all(&src).unwrap();
        let link = f.harness.path.join("skills/foo");
        let claude_md = f.harness.path.join("CLAUDE.md");
        let temp = f.harness.path.join(".colctl-tmp-9-x");
        fs::write(&temp, "").unwrap();
        let mut state = HarnessState::default();
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::CleanupTemp(temp.clone()),
                Action::Link {
                    link: link.clone(),
                    target: src.clone(),
                },
                Action::WriteInstructions {
                    path: claude_md.clone(),
                    content: b"gen".to_vec(),
                },
            ],
            &mut state,
        );
        assert!(results.iter().all(|r| r.outcome.is_done()), "{results:?}");
        assert!(!temp.exists());
        assert_eq!(fs::read_link(&link).unwrap(), src);
        assert_eq!(state.generated[CLAUDE_MD], fsx::hash_bytes(b"gen"));

        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::Unlink(link.clone()),
                Action::RemoveInstructions(claude_md.clone()),
            ],
            &mut state,
        );
        assert!(results.iter().all(|r| r.outcome.is_done()));
        assert!(!fsx::exists_any(&link) && !claude_md.exists());
        assert!(state.generated.is_empty());

        apply(
            &f.ctx,
            &f.harness,
            &[Action::RecordInstructions { hash: "x".into() }],
            &mut state,
        );
        assert_eq!(state.generated[CLAUDE_MD], "x");
    }

    #[test]
    fn ec_22_one_failing_item_does_not_stop_the_rest() {
        let f = fixture(false, false);
        let real = f.harness.path.join("skills/real");
        fs::create_dir_all(&real).unwrap();
        let mut state = HarnessState::default();
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::Link {
                    link: real.clone(),
                    target: "/x".into(),
                },
                Action::Unlink(real.clone()),
                Action::Link {
                    link: f.harness.path.join("skills/ok"),
                    target: "/x".into(),
                },
                Action::RemoveInstructions(f.harness.path.join("CLAUDE.md")),
                Action::WriteInstructions {
                    path: real.clone(),
                    content: vec![],
                },
            ],
            &mut state,
        );
        let done: Vec<bool> = results.iter().map(|r| r.outcome.is_done()).collect();
        assert_eq!(done, vec![false, false, true, false, false]);
        assert!(real.is_dir());
        assert!(state.generated.is_empty());
    }

    #[test]
    fn ac_12_ec_28_plugin_calls_use_the_harness_env() {
        let f = fixture(false, true);
        let mut state = HarnessState::default();
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::AddMarketplace {
                    name: "m".into(),
                    source: "github:o/r".into(),
                },
                Action::Install { id: "p@m".into() },
                Action::Uninstall { id: "p@m".into() },
                Action::RemoveMarketplace { name: "m".into() },
            ],
            &mut state,
        );
        assert!(results.iter().all(|r| r.outcome.is_done()), "{results:?}");
        let log = log(&f);
        let dir = f.harness.path.display().to_string();
        assert!(
            log.contains(&format!(
                "plugin marketplace add o/r | dir={dir} | anth=unset"
            )),
            "{log}"
        );
        assert!(log.contains("plugin install p@m"));
        assert!(state.plugins.is_empty() && state.marketplaces.is_empty());
    }

    #[test]
    fn ec_20_native_profile_runs_without_claude_config_dir() {
        let f = fixture(true, true);
        assert!(is_native(&f.ctx, &f.harness));
        let mut state = HarnessState::default();
        apply(
            &f.ctx,
            &f.harness,
            &[Action::Install {
                id: "p@anthropic-plugin-directory".into(),
            }],
            &mut state,
        );
        assert!(log(&f).contains("dir=unset | anth=unset"), "{}", log(&f));
        assert!(state.plugins.contains("p@anthropic-plugin-directory"));
    }

    #[test]
    fn ac_30_failures_carry_command_code_and_stderr() {
        let f = fixture(false, true);
        let mut state = HarnessState::default();
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::Install {
                    id: "fail@m".into(),
                },
                Action::Install { id: "ok@m".into() },
            ],
            &mut state,
        );
        assert_eq!(
            results[0].outcome,
            Outcome::Failed("claude plugin install fail@m exited 4: boom".into())
        );
        assert!(results[1].outcome.is_done());
        assert_eq!(state.plugins, BTreeSet::from(["ok@m".to_string()]));
        let r = apply(
            &f.ctx,
            &f.harness,
            &[Action::Uninstall {
                id: "fail@m".into(),
            }],
            &mut state,
        );
        assert!(!r[0].outcome.is_done());
        let r = apply(
            &f.ctx,
            &f.harness,
            &[Action::RemoveMarketplace {
                name: "fail".into(),
            }],
            &mut state,
        );
        assert!(!r[0].outcome.is_done());
    }

    #[test]
    fn ec_25_slow_claude_times_out() {
        let f = fixture(false, true);
        let r = apply(
            &f.ctx,
            &f.harness,
            &[Action::Install {
                id: "slow@m".into(),
            }],
            &mut HarnessState::default(),
        );
        assert_eq!(
            r[0].outcome,
            Outcome::Failed("claude plugin install slow@m timed out after 1s".into())
        );
    }

    #[test]
    fn ec_11_marketplace_name_mismatch_skips_its_plugins() {
        let f = fixture(false, true);
        let mut state = HarnessState::default();
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::AddMarketplace {
                    name: "mine".into(),
                    source: "github:o/wrongname".into(),
                },
                Action::Install {
                    id: "p@mine".into(),
                },
            ],
            &mut state,
        );
        assert_eq!(
            results[0].outcome,
            Outcome::Failed(
                "config calls it \"mine\", the marketplace calls itself \"other\"; rename it in config.toml".into()
            )
        );
        assert_eq!(
            results[1].outcome,
            Outcome::Skipped("marketplace \"mine\" wasn't added".into())
        );
        assert!(state.marketplaces.is_empty());

        // Claude succeeding without recording anything at all.
        let r = apply(
            &f.ctx,
            &f.harness,
            &[Action::AddMarketplace {
                name: "x".into(),
                source: "/nothing".into(),
            }],
            &mut state,
        );
        assert!(matches!(&r[0].outcome, Outcome::Failed(m) if m.contains("something else")));
    }

    #[test]
    fn ac_31_claude_missing_skips_plugin_actions() {
        let f = fixture(false, false);
        let mut state = HarnessState::default();
        state.plugins.insert("gone@m".into());
        state.marketplaces.insert("gone".into());
        let results = apply(
            &f.ctx,
            &f.harness,
            &[
                Action::Install { id: "p@m".into() },
                Action::ForgetPlugin {
                    id: "gone@m".into(),
                },
                Action::ForgetMarketplace {
                    name: "gone".into(),
                },
            ],
            &mut state,
        );
        assert_eq!(
            results[0].outcome,
            Outcome::Skipped("claude not found".into())
        );
        assert!(results[1].outcome.is_done() && results[2].outcome.is_done());
        assert!(state.is_empty());
    }

    #[test]
    fn spawn_failure_is_reported() {
        let f = fixture(false, false);
        let missing = Some(Path::new("/no/such/claude").to_path_buf());
        match run_claude(&f.ctx, &missing, &f.harness, &["plugin", "install", "x"]) {
            Outcome::Failed(m) => assert!(m.starts_with("claude plugin install x:")),
            other => panic!("{other:?}"),
        }
    }
}
