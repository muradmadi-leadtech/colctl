//! Argument parsing and dispatch (interfaces.md § Command line).

use crate::commands::{self, RmKind};
use crate::config::PluginList;
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_USAGE};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
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
    /// When to use colour: auto (a terminal, unless NO_COLOR), always or never
    #[arg(long, global = true, value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,
    /// Only errors and the verdict line
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    /// Also show each tool call and how long it took
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Answer yes to every confirmation
    #[arg(short, long, global = true)]
    yes: bool,
    /// Never page long output
    #[arg(long, global = true)]
    no_pager: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ColorArg {
    Auto,
    Always,
    Never,
}

impl Cli {
    fn settings(&self, json: bool) -> crate::ui::term::Settings {
        use crate::ui::term::ColorChoice;
        crate::ui::term::Settings {
            color: match self.color {
                ColorArg::Auto => ColorChoice::Auto,
                ColorArg::Always => ColorChoice::Always,
                ColorArg::Never => ColorChoice::Never,
            },
            quiet: self.quiet,
            verbose: self.verbose,
            yes: self.yes,
            no_pager: self.no_pager,
            json,
        }
    }
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
        /// List every item, not only what needs attention
        #[arg(long)]
        all: bool,
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
    /// Move source plugins' pins to newer upstream commits (the only command that looks upstream)
    Update {
        /// Sources to update (default: all)
        names: Vec<String>,
        /// Only report what's newer; change nothing
        #[arg(long)]
        check: bool,
        /// Pin this tag, branch or commit (needs exactly one source)
        #[arg(long)]
        rev: Option<String>,
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
    /// Search every plugin catalog your harnesses know (offline)
    Search {
        /// Words that must all appear (name, keywords, description, developer)
        terms: Vec<String>,
        /// Only plugins for this tool
        #[arg(long, value_enum)]
        tool: Option<ToolArg>,
        /// Only plugins from this marketplace
        #[arg(long)]
        marketplace: Option<String>,
        /// Only plugins by this developer
        #[arg(long)]
        developer: Option<String>,
        /// Only plugins installed somewhere
        #[arg(long)]
        installed: bool,
        /// Show at most this many results (0 for all)
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Show one plugin from the catalogs in full
    Show {
        /// A plugin id from search, or a name only one plugin has
        id: String,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// List developers and their plugin counts, or one developer's plugins
    Developers {
        /// A developer (a repo owner such as obra, or an author name)
        name: Option<String>,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// The marketplaces your profiles know, or ask the tools to update them
    Marketplace {
        #[command(subcommand)]
        what: MarketplaceCmd,
    },
    /// Search Codex's app connectors (Gmail, Slack…), which have no repo
    Connectors {
        /// Words that must all appear
        terms: Vec<String>,
        /// Show at most this many results (0 for all)
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Install held new versions exactly as reviewed, then sync
    Approve {
        /// Source names, skills/<name> or agents/<name>.md
        names: Vec<String>,
        /// Every held version
        #[arg(long)]
        all: bool,
        /// Print what would be approved without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Drop held new versions and don't offer them again
    Reject {
        /// Source names, skills/<name> or agents/<name>.md
        names: Vec<String>,
        /// Every held version
        #[arg(long)]
        all: bool,
        /// Print what would be rejected without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// List held new versions, or show one: commits, changed files, risk flags and the diff
    Review {
        /// A source name, skills/<name> or agents/<name>.md
        name: Option<String>,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
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
    /// Add a plugin from its own repo, pinned to a commit (cloned into ~/.colctl/plugins)
    Source {
        /// The repo: a URL, git@host:path, an absolute path or github:owner/repo
        repo: String,
        /// A tag, branch or commit to pin (default: the default branch's head)
        #[arg(long)]
        rev: Option<String>,
        /// The source's name (default: the repo's name)
        #[arg(long)]
        name: Option<String>,
        /// The plugin's name in the repo's manifests (default: the source's name)
        #[arg(long)]
        plugin: Option<String>,
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

#[derive(Subcommand, Debug)]
enum MarketplaceCmd {
    /// Every marketplace any profile knows (offline)
    List {
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Ask claude and codex to update their marketplaces (uses the network)
    Refresh {
        /// Print the calls without running them
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum ToolArg {
    Claude,
    Codex,
}

impl ToolArg {
    fn as_str(self) -> &'static str {
        match self {
            ToolArg::Claude => "claude",
            ToolArg::Codex => "codex",
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Kind {
    Skill,
    Agent,
    Plugin,
    Source,
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
        Command::Status { harness, all, json } => {
            commands::status(ctx, harness.as_deref(), json, all)
        }
        Command::Sync { harness, dry_run } => commands::sync(ctx, harness.as_deref(), dry_run),
        Command::Add { what } => match what {
            Add::Skill {
                source,
                subdir,
                name,
                dry_run,
            } => commands::add_skill(ctx, &source, subdir.as_deref(), name.as_deref(), dry_run),
            Add::Agent { file, dry_run } => commands::add_agent(ctx, &file, dry_run),
            Add::Source {
                repo,
                rev,
                name,
                plugin,
                dry_run,
            } => commands::add_source(
                ctx,
                &repo,
                rev.as_deref(),
                name.as_deref(),
                plugin.as_deref(),
                dry_run,
            ),
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
                Kind::Source => RmKind::Source,
            };
            commands::rm(ctx, kind, &name, list(codex), dry_run)
        }
        Command::Adopt {
            harness,
            item,
            dry_run,
        } => commands::adopt(ctx, harness.as_deref(), &item, dry_run),
        Command::Refresh { items, dry_run } => commands::refresh(ctx, &items, dry_run),
        Command::Update {
            names,
            check,
            rev,
            dry_run,
        } => commands::update(ctx, &names, check, rev.as_deref(), dry_run),
        Command::Search {
            terms,
            tool,
            marketplace,
            developer,
            installed,
            limit,
            json,
        } => commands::search(
            ctx,
            &crate::search::Query {
                terms,
                tool: tool.map(|t| t.as_str().to_string()),
                marketplace,
                developer,
                installed,
                limit,
            },
            json,
        ),
        Command::Marketplace { what } => match what {
            MarketplaceCmd::List { json } => commands::marketplace_list(ctx, json),
            MarketplaceCmd::Refresh { dry_run } => commands::marketplace_refresh(ctx, dry_run),
        },
        Command::Connectors { terms, limit, json } => {
            commands::connectors(ctx, &terms, limit, json)
        }
        Command::Show { id, json } => commands::show(ctx, &id, json),
        Command::Developers { name, json } => commands::developers(ctx, name.as_deref(), json),
        Command::Approve {
            names,
            all,
            dry_run,
        } => commands::approve(ctx, &names, all, dry_run),
        Command::Reject {
            names,
            all,
            dry_run,
        } => commands::reject(ctx, &names, all, dry_run),
        Command::Review { name, json } => commands::review(ctx, name.as_deref(), json),
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
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let words: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let json = words.iter().any(|w| w == "--json");
    let command = words
        .iter()
        .find(|w| !w.starts_with('-'))
        .cloned()
        .unwrap_or_default();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err) => return parse_error(&err, &words),
    };
    let out = crate::ui::Out::new(&cli.settings(json));
    let result = Ctx::from_process().and_then(|mut ctx| {
        ctx.out = out;
        dispatch(&ctx, cli.command)
    });
    match result {
        Ok(code) => code,
        Err(err) => {
            let fallback = if command.is_empty() {
                "colctl --help".to_string()
            } else {
                format!("colctl {command} --help")
            };
            out.error(&err, &fallback);
            err.code
        }
    }
}

/// Argument errors in colctl's error layout (AC6-06, EC6-18); help and version print as they are.
fn parse_error(err: &clap::Error, words: &[String]) -> i32 {
    use crate::ui::term::{ColorChoice, Settings};
    let color = if words.iter().any(|w| w == "--color=never" || w == "never") {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };
    let out = crate::ui::Out::new(&Settings {
        color,
        ..Settings::default()
    });
    if !err.use_stderr() {
        out.raw(&err.render().to_string());
        return 0;
    }
    let text = err.render().to_string();
    let mut what = String::new();
    let mut hint = None;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("error: ") {
            what = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("tip: ") {
            hint = Some(
                rest.replace("a similar subcommand exists:", "did you mean")
                    .replace('\'', ""),
            );
        }
    }
    let command = words.iter().find(|w| !w.starts_with('-'));
    let fallback = match command {
        Some(c) if Cli::command().find_subcommand(c).is_some() => format!("colctl {c} --help"),
        _ => "colctl --help".to_string(),
    };
    let error = CliError::usage(what).hint(hint.unwrap_or(fallback));
    out.error(&error, "colctl --help");
    EXIT_USAGE
}
