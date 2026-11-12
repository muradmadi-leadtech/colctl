//! Argument parsing and dispatch (interfaces.md § Command line).

use crate::commands::{self, RmKind};
use crate::config::PluginList;
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_USAGE};
use clap::{Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "colctl",
    version,
    about = "Keep every AI coding harness in sync with one source of truth in ~/.colctl",
    long_about = "Keep every AI coding harness in sync with one source of truth in ~/.colctl: one AGENTS.md, \
one copy of each skill and agent, one plugin list.\n\nSet COLCTL_HOME to keep the source somewhere else."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create the source and register the harnesses found on this machine
    Init {
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show what is in sync and what isn't; exits 1 when anything needs attention
    Status {
        /// Only this harness
        #[arg(long)]
        harness: Option<String>,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Make every harness match the source
    Sync {
        /// Only this harness
        #[arg(long)]
        harness: Option<String>,
        /// Print the plan without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Add a skill, agent or plugin to the source, then sync
    Add {
        #[command(subcommand)]
        what: Add,
    },
    /// Remove a skill, agent or plugin from the source and every harness
    Rm {
        kind: Kind,
        name: String,
        /// A plugin from the Codex list instead of Claude's
        #[arg(long)]
        codex: bool,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Move what a harness already has into the source, then sync
    Adopt {
        /// Only this harness
        harness: Option<String>,
        /// skills/<name>, agents/<name>.md, CLAUDE.md, or plugin <id>
        item: Vec<String>,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Pull adopted skills and agents up to date with where they came from
    Refresh {
        /// skills/<name> or agents/<name>.md; all with a recorded origin if omitted
        items: Vec<String>,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the version
    Version,
}

#[derive(Subcommand, Debug)]
enum Add {
    /// Add a skill folder (it must contain SKILL.md) from a path or a git URL
    Skill {
        /// A folder, or a git URL to clone
        source: String,
        /// The skill's folder inside the source
        #[arg(long)]
        subdir: Option<String>,
        /// The name to give the skill (default: the folder's name)
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add an agent file (<name>.md)
    Agent {
        file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Enable a plugin, name@marketplace
    Plugin {
        id: String,
        /// Where the marketplace comes from, if it's new: github:owner/repo, a URL or a path
        #[arg(long)]
        marketplace: Option<String>,
        /// The Codex plugin list instead of Claude's
        #[arg(long)]
        codex: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Kind {
    Skill,
    Agent,
    Plugin,
}

fn list(codex: bool) -> PluginList {
    if codex {
        PluginList::Codex
    } else {
        PluginList::Claude
    }
}

fn dispatch(ctx: &Ctx, command: Command) -> Result<i32, CliError> {
    match command {
        Command::Init { dry_run } => commands::init(ctx, dry_run),
        Command::Status { harness, json } => commands::status(ctx, harness.as_deref(), json),
        Command::Sync { harness, dry_run } => commands::sync(ctx, harness.as_deref(), dry_run),
        Command::Add { what } => match what {
            Add::Skill {
                source,
                subdir,
                name,
                dry_run,
            } => commands::add_skill(ctx, &source, subdir.as_deref(), name.as_deref(), dry_run),
            Add::Agent { file, dry_run } => commands::add_agent(ctx, &file, dry_run),
            Add::Plugin {
                id,
                marketplace,
                codex,
                dry_run,
            } => commands::add_plugin(ctx, &id, marketplace.as_deref(), list(codex), dry_run),
        },
        Command::Rm {
            kind,
            name,
            codex,
            dry_run,
        } => {
            let kind = match kind {
                Kind::Skill => RmKind::Skill,
                Kind::Agent => RmKind::Agent,
                Kind::Plugin => RmKind::Plugin,
            };
            commands::rm(ctx, kind, &name, list(codex), dry_run)
        }
        Command::Adopt {
            harness,
            item,
            dry_run,
        } => commands::adopt(ctx, harness.as_deref(), &item, dry_run),
        Command::Refresh { items, dry_run } => commands::refresh(ctx, &items, dry_run),
        Command::Version => {
            println!("colctl {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
    }
}

/// Runs colctl with `args` (including the program name) and returns the exit code.
pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return if err.use_stderr() { EXIT_USAGE } else { 0 };
        }
    };
    let result = Ctx::from_process().and_then(|ctx| dispatch(&ctx, cli.command));
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("colctl: {}", err.message);
            err.code
        }
    }
}
