//! Everything about calling a harness's own plugin CLI: which program, the argv for each plugin
//! action, the environment it must (and must not) see, and how to read its state back.

use crate::adapter;
use crate::config::{Harness, Kind};
use crate::ctx::Ctx;
use crate::fsx;
use crate::plan::{Action, marketplace_arg};
use crate::runner::Cmd;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A plugin CLI colctl drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Claude,
    Codex,
}

impl Tool {
    /// The tool a harness kind uses for plugins, if any.
    pub fn for_kind(kind: Kind) -> Option<Tool> {
        match kind {
            Kind::Claude => Some(Tool::Claude),
            Kind::Codex => Some(Tool::Codex),
            Kind::Agents => None,
        }
    }

    /// The program's name on `PATH`.
    pub fn program(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Codex => "codex",
        }
    }

    /// Every call (argv without the program) a plugin action needs, in order; empty for actions that
    /// aren't plugin calls.
    pub fn argvs(self, action: &Action) -> Vec<Vec<String>> {
        let own = |w: Vec<&str>| w.into_iter().map(str::to_string).collect::<Vec<_>>();
        match action {
            Action::AddSourceMarketplace { dir, .. } => {
                vec![own(vec![
                    "plugin",
                    "marketplace",
                    "add",
                    &dir.to_string_lossy(),
                ])]
            }
            Action::InstallSource { id, .. } => self
                .argv(&Action::Install { id: id.clone() })
                .into_iter()
                .collect(),
            Action::UpdateSource {
                id, marketplace, ..
            } => match self {
                Tool::Claude => vec![
                    own(vec!["plugin", "marketplace", "update", marketplace]),
                    own(vec!["plugin", "update", id]),
                ],
                Tool::Codex => vec![own(vec!["plugin", "add", id])],
            },
            Action::UninstallSource { id, .. } => self
                .argv(&Action::Uninstall { id: id.clone() })
                .into_iter()
                .collect(),
            Action::RemoveSourceMarketplace { name } => {
                vec![own(vec!["plugin", "marketplace", "remove", name])]
            }
            other => self.argv(other).into_iter().collect(),
        }
    }

    /// The argv (without the program) for a single-call plugin action; `None` for actions that aren't.
    pub fn argv(self, action: &Action) -> Option<Vec<String>> {
        let words: Vec<&str> = match action {
            Action::AddMarketplace { source, .. } => {
                vec!["plugin", "marketplace", "add", marketplace_arg(source)]
            }
            Action::RemoveMarketplace { name } => vec!["plugin", "marketplace", "remove", name],
            Action::Install { id } if self == Tool::Codex => vec!["plugin", "add", id],
            Action::Uninstall { id } if self == Tool::Codex => vec!["plugin", "remove", id],
            Action::Install { id } => vec!["plugin", "install", id],
            Action::Uninstall { id } => vec!["plugin", "uninstall", id],
            _ => return None,
        };
        Some(words.into_iter().map(str::to_string).collect())
    }

    /// The variable that points the tool at a profile.
    fn home_var(self) -> &'static str {
        match self {
            Tool::Claude => "CLAUDE_CONFIG_DIR",
            Tool::Codex => "CODEX_HOME",
        }
    }

    /// The profile the tool uses when `home_var` is unset.
    fn default_home(self, ctx: &Ctx) -> PathBuf {
        match self {
            Tool::Claude => ctx.home.join(".claude"),
            Tool::Codex => ctx.home.join(".codex"),
        }
    }

    /// Variables removed before every call, besides those with a scrubbed prefix.
    fn scrubbed(self) -> &'static [&'static str] {
        match self {
            Tool::Claude => &["CLAUDE_CONFIG_DIR", "CLAUDE_SECURESTORAGE_CONFIG_DIR"],
            Tool::Codex => &["CODEX_HOME"],
        }
    }

    /// Prefixes of variables removed before every call (credentials, endpoints).
    fn scrubbed_prefixes(self) -> &'static [&'static str] {
        match self {
            Tool::Claude => &["ANTHROPIC_"],
            Tool::Codex => &["OPENAI_"],
        }
    }

    /// True when `harness` is the tool's default profile, which must run without the home variable.
    pub fn is_native(self, ctx: &Ctx, harness: &Harness) -> bool {
        fsx::canonical_or_normal(&harness.path) == fsx::canonical_or_normal(&self.default_home(ctx))
    }

    /// Builds a call for `harness` with exactly its own environment (EC-28).
    pub fn command(self, ctx: &Ctx, program: PathBuf, harness: &Harness, args: &[String]) -> Cmd {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut cmd = Cmd::new(program, &words);
        cmd.env_remove = self
            .scrubbed()
            .iter()
            .map(|s| s.to_string())
            .chain(
                ctx.env_keys
                    .iter()
                    .filter(|k| self.scrubbed_prefixes().iter().any(|p| k.starts_with(p)))
                    .cloned(),
            )
            .collect();
        if !self.is_native(ctx, harness) {
            cmd.env_set
                .push((self.home_var().into(), harness.path.display().to_string()));
        }
        cmd.cwd = Some(ctx.home.clone());
        cmd.timeout = Some(ctx.tool_timeout);
        cmd
    }

    /// Marketplace names the profile knows, read back after a `marketplace add`.
    pub fn known_marketplaces(self, harness: &Path) -> BTreeSet<String> {
        match self {
            Tool::Claude => adapter::read_plugins(harness),
            Tool::Codex => adapter::codex::read_plugins(harness),
        }
        .map(|p| p.marketplaces.into_keys().collect())
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_argv_and_names() {
        let t = Tool::Claude;
        assert_eq!(t.program(), "claude");
        assert_eq!(Tool::for_kind(Kind::Claude), Some(Tool::Claude));
        let argv = |a: Action| t.argv(&a).unwrap().join(" ");
        assert_eq!(
            argv(Action::AddMarketplace {
                name: "m".into(),
                source: "github:o/r".into()
            }),
            "plugin marketplace add o/r"
        );
        assert_eq!(
            argv(Action::RemoveMarketplace { name: "m".into() }),
            "plugin marketplace remove m"
        );
        assert_eq!(
            argv(Action::Install { id: "p@m".into() }),
            "plugin install p@m"
        );
        assert_eq!(
            argv(Action::Uninstall { id: "p@m".into() }),
            "plugin uninstall p@m"
        );
        assert!(t.argv(&Action::ForgetPlugin { id: "p@m".into() }).is_none());
    }
}
