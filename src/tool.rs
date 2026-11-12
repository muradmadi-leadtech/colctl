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
}

impl Tool {
    /// The tool a harness kind uses for plugins, if any.
    pub fn for_kind(kind: Kind) -> Option<Tool> {
        match kind {
            Kind::Claude => Some(Tool::Claude),
        }
    }

    /// The program's name on `PATH`.
    pub fn program(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
        }
    }

    /// The argv (without the program) for a plugin action; `None` for actions that aren't.
    pub fn argv(self, action: &Action) -> Option<Vec<String>> {
        let words: Vec<&str> = match action {
            Action::AddMarketplace { source, .. } => {
                vec!["plugin", "marketplace", "add", marketplace_arg(source)]
            }
            Action::RemoveMarketplace { name } => vec!["plugin", "marketplace", "remove", name],
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
        }
    }

    /// The profile the tool uses when `home_var` is unset.
    fn default_home(self, ctx: &Ctx) -> PathBuf {
        match self {
            Tool::Claude => ctx.home.join(".claude"),
        }
    }

    /// Variables removed before every call, besides those with a scrubbed prefix.
    fn scrubbed(self) -> &'static [&'static str] {
        match self {
            Tool::Claude => &["CLAUDE_CONFIG_DIR", "CLAUDE_SECURESTORAGE_CONFIG_DIR"],
        }
    }

    /// Prefixes of variables removed before every call (credentials, endpoints).
    fn scrubbed_prefixes(self) -> &'static [&'static str] {
        match self {
            Tool::Claude => &["ANTHROPIC_"],
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
        cmd.timeout = Some(ctx.claude_timeout);
        cmd
    }

    /// Marketplace names the profile knows, read back after a `marketplace add`.
    pub fn known_marketplaces(self, harness: &Path) -> BTreeSet<String> {
        match self {
            Tool::Claude => adapter::read_plugins(harness)
                .map(|p| p.marketplaces.into_keys().collect())
                .unwrap_or_default(),
        }
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
