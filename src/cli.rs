//! Argument parsing and dispatch (interfaces.md § Command line).

use crate::commands::{self, RmKind};
use crate::config::PluginList;
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_USAGE};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::path::PathBuf;

/// The welcome help's groups (AC6-25, AC6-36): commands grouped by task, the first three as "Start
/// here". A test checks every subcommand is listed.
const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Start here",
        &[
            ("init", "Find your profiles and create ~/.colctl"),
            ("status", "See what's in sync and what needs you"),
            ("sync", "Make every profile match"),
        ],
    ),
    (
        "Change what you have",
        &[
            (
                "add",
                "Add a skill, agent, plugin or source plugin, then sync",
            ),
            ("rm", "Remove one, everywhere"),
            ("adopt", "Move what a profile already has into ~/.colctl"),
            ("convert", "Package a Claude-only source plugin for Codex"),
        ],
    ),
    (
        "Updates and review",
        &[
            (
                "update",
                "Fetch new versions of source plugins and hold them for review",
            ),
            (
                "refresh",
                "Hold changes from the folders taken-over skills came from",
            ),
            ("review", "See what a held version changes, with risk flags"),
            ("approve", "Install held versions exactly as reviewed"),
            ("reject", "Drop held versions for good"),
        ],
    ),
    (
        "Find plugins",
        &[
            ("search", "Search every plugin catalog your profiles know"),
            ("show", "One plugin in full"),
            ("developers", "Who publishes what"),
            ("marketplace", "List or refresh marketplaces"),
            ("connectors", "Codex's app connectors"),
        ],
    ),
    (
        "Other",
        &[
            ("completions", "Shell completions (bash, zsh, fish)"),
            ("version", "Print the version"),
            ("help", "Help for a command"),
        ],
    ),
];

const OPTIONS: &[(&str, &str)] = &[
    ("--color <when>", "auto, always or never"),
    ("-q, --quiet", "Only errors and the verdict line"),
    (
        "-v, --verbose",
        "Also show each tool call and how long it took",
    ),
    ("-y, --yes", "Answer yes to every confirmation"),
    ("--no-pager", "Never page long output"),
    ("-V, --version", "Print the version"),
];

const DOCS: &str = "https://github.com/muradmadi-leadtech/colctl/tree/main/docs";

/// The help someone new sees (AC6-36): what colctl does, where to start, every command by task,
/// the options in short, and where to learn more. Shown for `colctl`, `colctl help` and `--help`.
fn welcome(caps: &crate::ui::term::Caps) -> String {
    use crate::ui::render;
    use crate::ui::style::{Token, paint};
    let c = caps.color_out;
    let mut lines = render::wrap(
        "colctl keeps every AI coding profile on this machine in sync with one source of truth, \
         ~/.colctl: one AGENTS.md, one copy of each skill and agent, one plugin list.",
        caps.width,
        0,
        0,
    );
    lines.push(String::new());
    lines.push(format!(
        "{} colctl <command> [options]",
        paint("Usage:", Token::Heading, c)
    ));
    // One column for every description, so the page reads as one list ("Start here" names carry
    // the "colctl " prefix).
    let w = GROUPS
        .iter()
        .enumerate()
        .flat_map(|(g, (_, commands))| {
            commands
                .iter()
                .map(move |(n, _)| n.len() + if g == 0 { "colctl ".len() } else { 0 })
        })
        .chain(OPTIONS.iter().map(|(o, _)| o.len()))
        .max()
        .unwrap_or(0);
    let rows = |lines: &mut Vec<String>, rows: &[(String, &str)]| {
        for (name, what) in rows {
            let prefix = 2 + w + 3;
            let wrapped = render::wrap(what, caps.width, prefix, prefix);
            for (i, text) in wrapped.into_iter().enumerate() {
                if i == 0 {
                    let pad = " ".repeat(w - render::width(name));
                    lines.push(format!(
                        "  {}{pad}   {text}",
                        paint(name, Token::Command, c)
                    ));
                } else {
                    lines.push(text);
                }
            }
        }
    };
    for (i, (heading, commands)) in GROUPS.iter().enumerate() {
        lines.push(String::new());
        lines.push(paint(heading, Token::Heading, c));
        let named: Vec<(String, &str)> = commands
            .iter()
            .map(|(name, what)| {
                let name = if i == 0 {
                    format!("colctl {name}")
                } else {
                    (*name).to_string()
                };
                (name, *what)
            })
            .collect();
        rows(&mut lines, &named);
    }
    lines.push(String::new());
    lines.push(paint("Options", Token::Heading, c));
    let options: Vec<(String, &str)> = OPTIONS
        .iter()
        .map(|(o, w)| ((*o).to_string(), *w))
        .collect();
    rows(&mut lines, &options);
    lines.push(String::new());
    lines.extend(render::wrap(
        &format!(
            "More: {} shows one command in full, with examples.",
            paint("colctl <command> --help", Token::Command, c)
        ),
        caps.width + if c { 9 } else { 0 },
        0,
        0,
    ));
    lines.push(format!("Docs: {DOCS}"));
    lines.push("Set COLCTL_HOME to keep ~/.colctl somewhere else.".to_string());
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Styles for each command's own help, matching colctl's tokens: headings bold, commands cyan.
fn styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(Effects::BOLD.into())
        .usage(Effects::BOLD.into())
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(Effects::DIMMED.into())
}

#[derive(Parser, Debug)]
#[command(
    disable_help_subcommand = true,
    name = "colctl",
    version,
    about = "Keep every AI coding harness in sync with one source of truth in ~/.colctl",
    long_about = "Keep every AI coding harness in sync with one source of truth in ~/.colctl: one AGENTS.md, \
one copy of each skill and agent, one plugin list.\n\nSet COLCTL_HOME to keep the source somewhere else."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// When to use colour: auto (a terminal, unless NO_COLOR), always or never
    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = ColorArg::Auto,
        hide_possible_values = true,
        hide_default_value = true
    )]
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
    /// Create ~/.colctl and register the profiles on this machine
    #[command(after_help = "Examples:\n  colctl init\n  colctl init --dry-run")]
    Init {
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// What's in sync, what isn't, and what to run (exits 1 when something needs attention)
    #[command(
        after_help = "Examples:\n  colctl status\n  colctl status --all\n  colctl status --harness kilo-claude --json"
    )]
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
    /// Make every profile match ~/.colctl
    #[command(after_help = "Examples:\n  colctl sync\n  colctl sync --dry-run\n  colctl sync -v")]
    Sync {
        /// Only this harness
        #[arg(long)]
        harness: Option<String>,
        /// Print the plan without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Add a skill, agent, plugin or source plugin, then sync
    #[command(
        after_help = "Examples:\n  colctl add skill ./my-skill\n  colctl add source superpowers@claude-plugins-official\n  colctl add plugin sadr@sadr --marketplace github:muradmadi/sadr"
    )]
    Add {
        #[command(subcommand)]
        what: Add,
    },
    /// Remove a skill, agent, plugin or source plugin from ~/.colctl and every profile
    #[command(
        after_help = "Examples:\n  colctl rm skill old-skill\n  colctl rm source databricks --yes\n  colctl rm plugin x@openai-curated --codex"
    )]
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
    /// Move what a profile already has into ~/.colctl, then sync
    #[command(
        after_help = "Examples:\n  colctl adopt\n  colctl adopt claude skills/foo\n  colctl adopt claude plugin superpowers@anthropic-plugin-directory"
    )]
    Adopt {
        /// Only this harness
        harness: Option<String>,
        /// skills/<name>, agents/<name>.md, CLAUDE.md, or plugin <id>
        item: Vec<String>,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Fetch new versions of source plugins and hold them for review (nothing installs until you approve)
    #[command(
        after_help = "Examples:\n  colctl update --check\n  colctl update\n  colctl update superpowers --rev v7.0.2"
    )]
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
    /// Hold changes from the folders taken-over skills and agents came from, for review
    #[command(
        after_help = "Examples:\n  colctl refresh\n  colctl refresh skills/twg-jira\n  colctl refresh --dry-run"
    )]
    Refresh {
        /// skills/<name> or agents/<name>.md; all with a recorded origin if omitted
        items: Vec<String>,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Search every plugin catalog your profiles know (offline)
    #[command(
        after_help = "Examples:\n  colctl search api security\n  colctl search --developer obra\n  colctl search --tool codex --limit 0 --json"
    )]
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
    /// One plugin from the catalogs in full
    #[command(
        after_help = "Examples:\n  colctl show superpowers\n  colctl show 42crunch-api-security-testing@claude-plugins-official --json"
    )]
    Show {
        /// A plugin id from search, or a name only one plugin has (in a terminal, omit it to pick)
        id: Option<String>,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Who publishes what: developers and their plugin counts, or one developer's plugins
    #[command(after_help = "Examples:\n  colctl developers\n  colctl developers obra")]
    Developers {
        /// A developer (a repo owner such as obra, or an author name)
        name: Option<String>,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// List the marketplaces your profiles know, or ask the tools to refresh them
    #[command(
        after_help = "Examples:\n  colctl marketplace list\n  colctl marketplace refresh --dry-run"
    )]
    Marketplace {
        #[command(subcommand)]
        what: MarketplaceCmd,
    },
    /// Codex's app connectors (Gmail, Slack…), which have no repo
    #[command(after_help = "Examples:\n  colctl connectors\n  colctl connectors gmail --json")]
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
    /// Install held versions exactly as reviewed, then sync
    #[command(
        after_help = "Examples:\n  colctl approve superpowers\n  colctl approve skills/twg-jira\n  colctl approve --all --yes"
    )]
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
    /// Drop held versions for good; newer ones are still held
    #[command(after_help = "Examples:\n  colctl reject superpowers\n  colctl reject --all")]
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
    /// See what a held version changes: risk flags, commits, files and the diff
    #[command(
        after_help = "Examples:\n  colctl review\n  colctl review superpowers\n  colctl review -i"
    )]
    Review {
        /// A source name, skills/<name> or agents/<name>.md
        name: Option<String>,
        /// Pick a held version from a list (in a terminal)
        #[arg(short, long)]
        interactive: bool,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Print a shell completion script (bash, zsh, fish, elvish, powershell)
    #[command(
        after_help = "Examples:\n  colctl completions fish > ~/.config/fish/completions/colctl.fish\n  colctl completions zsh > ~/.zfunc/_colctl"
    )]
    Completions {
        /// The shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Help for colctl, or for one command
    #[command(
        after_help = "Examples:\n  colctl help\n  colctl help sync\n  colctl help add source"
    )]
    Help {
        /// The command, and subcommand if any
        command: Vec<String>,
    },
    /// Package a source plugin that only packages for Claude for Codex, held for review
    #[command(
        after_help = "Examples:\n  colctl convert qdrant-skills\n  colctl convert qdrant-skills --dry-run\n  colctl convert --undo qdrant-skills"
    )]
    Convert {
        /// The source's name
        source: String,
        /// Stop converting it: uninstall it from Codex profiles
        #[arg(long)]
        undo: bool,
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
        /// Print one JSON document
        #[arg(long)]
        json: bool,
    },
    /// Print the version
    #[command(after_help = "Examples:\n  colctl version\n  colctl --version")]
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
        Command::Show { id, json } => commands::show(ctx, id.as_deref(), json),
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
        Command::Review {
            name,
            interactive,
            json,
        } => commands::review(ctx, name.as_deref(), interactive, json),
        Command::Completions { shell } => {
            let mut script = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "colctl", &mut script);
            ctx.out.raw(&String::from_utf8_lossy(&script));
            Ok(0)
        }
        Command::Convert {
            source,
            undo,
            dry_run,
            json,
        } => commands::convert(ctx, &source, undo, dry_run, json),
        // Handled in `run`, before a context is needed.
        Command::Help { .. } => Ok(0),
        Command::Version => {
            ctx.out
                .line(format!("colctl {}", env!("CARGO_PKG_VERSION")));
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
    let early = crate::ui::Out::new(&crate::ui::term::Settings {
        color: color_word(&words),
        ..crate::ui::term::Settings::default()
    });
    let parsed = Cli::command()
        .styles(styles())
        .term_width(early.caps.width)
        .try_get_matches_from(&args)
        .and_then(|m| <Cli as clap::FromArgMatches>::from_arg_matches(&m));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(err) => return parse_error(&err, &words, &early),
    };
    let out = crate::ui::Out::new(&cli.settings(json));
    let subcommand = match cli.command {
        // EC6-23: `colctl` alone is a welcome, not an error, and touches nothing.
        None => {
            out.raw(&welcome(&out.caps));
            return 0;
        }
        Some(Command::Help { command }) if command.is_empty() => {
            out.raw(&welcome(&out.caps));
            return 0;
        }
        // `colctl help <command>` is `colctl <command> --help`, global flags kept.
        Some(Command::Help { command }) => {
            let mut again: Vec<OsString> = vec!["colctl".into()];
            again.extend(command.into_iter().map(OsString::from));
            again.push("--help".into());
            again.extend(
                words
                    .iter()
                    .skip_while(|w| *w != "help")
                    .filter(|w| w.starts_with('-') && *w != "--color")
                    .map(OsString::from),
            );
            if let Some(c) = words.iter().position(|w| w == "--color") {
                again.extend([
                    "--color".into(),
                    words.get(c + 1).cloned().unwrap_or_default().into(),
                ]);
            }
            return run(again);
        }
        Some(c) => c,
    };
    let result = Ctx::from_process().and_then(|mut ctx| {
        ctx.out = out;
        dispatch(&ctx, subcommand)
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

/// The `--color` choice in raw arguments, for output before they're parsed.
fn color_word(words: &[String]) -> crate::ui::term::ColorChoice {
    use crate::ui::term::ColorChoice;
    let value = words.iter().enumerate().find_map(|(i, w)| {
        w.strip_prefix("--color=")
            .map(str::to_string)
            .or_else(|| (w == "--color").then(|| words.get(i + 1).cloned().unwrap_or_default()))
    });
    match value.as_deref() {
        Some("always") => ColorChoice::Always,
        Some("never") => ColorChoice::Never,
        _ => ColorChoice::Auto,
    }
}

/// Argument errors in colctl's error layout (AC6-06, EC6-18). Help for colctl itself is the welcome
/// (AC6-36); a command's help and the version print as clap renders them, styled when colour is on.
fn parse_error(err: &clap::Error, words: &[String], out: &crate::ui::Out) -> i32 {
    if !err.use_stderr() {
        let named = words
            .iter()
            .enumerate()
            .filter(|(i, w)| !w.starts_with('-') && (*i == 0 || words[i - 1] != "--color"))
            .any(|(_, w)| w != "help" && Cli::command().find_subcommand(w).is_some());
        if err.kind() == clap::error::ErrorKind::DisplayHelp && !named {
            out.raw(&welcome(&out.caps));
        } else if out.caps.color_out {
            out.raw(&err.render().ansi().to_string());
        } else {
            out.raw(&err.render().to_string());
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac6_25_the_grouped_help_lists_every_command() {
        let listed: Vec<&str> = GROUPS
            .iter()
            .flat_map(|(_, commands)| commands.iter().map(|(name, _)| *name))
            .collect();
        for sub in Cli::command().get_subcommands() {
            let name = sub.get_name();
            assert!(
                listed.contains(&name),
                "{name} is missing from the grouped help"
            );
        }
        let help = welcome(&crate::ui::term::Caps::plain());
        for name in listed {
            assert!(help.contains(&format!(" {name} ")), "{name} in {help}");
        }
    }

    #[test]
    fn ac6_26_every_command_has_examples() {
        for sub in Cli::command().get_subcommands() {
            let after = sub
                .get_after_help()
                .map(|h| h.to_string())
                .unwrap_or_default();
            assert!(
                after.starts_with("Examples:\n  colctl "),
                "{} has no examples",
                sub.get_name()
            );
        }
    }
}
