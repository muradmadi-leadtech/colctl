//! The commands: `init`, `status`, `sync`, `add`, `rm`, `adopt`. Each returns the exit code, or a
//! [`CliError`] that carries one.

use crate::adapter::{self, CLAUDE_MD, Root};
use crate::apply::{self, Outcome};
use crate::config::{self, CONFIG_FILE, Config, Harness, Kind, PluginList};
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_DRIFT};
use crate::fsx;
use crate::git::Git;
use crate::packaging::{self, ToolPackaging};
use crate::plan::{self, HarnessPlan, Inputs, Item, ItemKind, ItemState};
use crate::plan::{SourceInput, SourceTarget};
use crate::source::{self, AGENTS, AGENTS_MD, SKILLS, Source};
use crate::state::{self, Lock, Origin, STATE_DIR, State};
use crate::vault::{self, CloneState, Vault};
use std::collections::BTreeMap;

mod quarantine;
pub use quarantine::update;

/// Where source plugin clones live, and colctl-written manifests inside it.
pub const PLUGINS: &str = "plugins";
pub const MANIFESTS: &str = ".manifests";
use crate::tool::Tool;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Harnesses `init` looks for: (name, path relative to home, kind).
pub const KNOWN_HARNESSES: &[(&str, &str, Kind)] = &[
    ("claude", ".claude", Kind::Claude),
    (
        "kilo-claude",
        "Library/Application Support/kilo-proxy-headless/profiles/claude",
        Kind::Claude,
    ),
    ("codex", ".codex", Kind::Codex),
    (
        "kilo-codex",
        "Library/Application Support/kilo-proxy-headless/profiles/codex",
        Kind::Codex,
    ),
    ("agents", ".agents", Kind::Agents),
];

/// Everything read from the source at one moment.
struct Workspace {
    config: Config,
    config_text: String,
    state: State,
    source: Source,
    origin_changed: Vec<(String, PathBuf)>,
    /// Source plugins prepared for planning, and each clone's state.
    sources: Vec<SourceInput>,
    clones: Vec<(String, &'static str)>,
}

fn load(ctx: &Ctx) -> Result<Workspace, CliError> {
    let (config, config_text) = config::load(&ctx.source, &ctx.home)?;
    let state = state::load(&ctx.source)?;
    let origin_changed = origin_changes(ctx, &state);
    let git = Git::find(&ctx.path_var).ok();
    let (sources, clones) = prepare_sources(ctx, &config, git.as_ref(), &BTreeMap::new());
    Ok(Workspace {
        config,
        config_text,
        state,
        source: source::scan(&ctx.source),
        origin_changed,
        sources,
        clones,
    })
}

fn plugins_dir(ctx: &Ctx) -> PathBuf {
    ctx.source.join(PLUGINS)
}

fn manifests_dir(ctx: &Ctx, name: &str) -> PathBuf {
    plugins_dir(ctx).join(MANIFESTS).join(name)
}

fn source_target(p: ToolPackaging) -> SourceTarget {
    match p {
        ToolPackaging::Supported(t) => SourceTarget::Ready(t),
        ToolPackaging::Unsupported(r) => SourceTarget::Unsupported(r),
        ToolPackaging::Invalid(r) => SourceTarget::Invalid(r),
    }
}

/// Reads each source's clone (offline, never writing) and what each tool would install from it.
/// `errors` are failures from sync's clone step, reported instead of the clone's state.
fn prepare_sources(
    ctx: &Ctx,
    config: &Config,
    git: Option<&Git>,
    errors: &BTreeMap<String, String>,
) -> (Vec<SourceInput>, Vec<(String, &'static str)>) {
    let mut inputs = Vec::new();
    let mut clones = Vec::new();
    for s in &config.sources {
        let both = |t: SourceTarget| (t.clone(), t);
        let (state, (claude, codex)) = if let Some(err) = errors.get(&s.name) {
            ("invalid", both(SourceTarget::Invalid(err.clone())))
        } else if let Some(git) = git {
            let vault = Vault {
                git,
                dir: plugins_dir(ctx),
            };
            let state = vault.inspect(&s.name, &s.clone_url(), &s.rev);
            let targets = match &state {
                CloneState::Ok => {
                    let p = packaging::inspect_with(
                        false,
                        &vault.path(&s.name),
                        &manifests_dir(ctx, &s.name),
                        &s.name,
                        &s.plugin,
                        s.path.as_deref(),
                    );
                    (source_target(p.claude), source_target(p.codex))
                }
                CloneState::Missing => both(SourceTarget::Pending(
                    "not cloned yet; colctl sync clones it".into(),
                )),
                CloneState::Moved { .. } => both(SourceTarget::Pending(
                    "the clone isn't at the pin; colctl sync checks it out".into(),
                )),
                CloneState::Dirty => both(SourceTarget::Invalid(format!(
                    "the clone at {} has local changes",
                    vault.path(&s.name).display()
                ))),
                CloneState::Invalid(r) => both(SourceTarget::Invalid(r.clone())),
            };
            (state.as_str(), targets)
        } else {
            (
                "missing",
                both(SourceTarget::Pending("git isn't on PATH".into())),
            )
        };
        clones.push((s.name.clone(), state));
        inputs.push(SourceInput {
            name: s.name.clone(),
            repo: s.repo.clone(),
            rev: s.rev.clone(),
            replaces: s.replaces.clone(),
            claude,
            codex,
        });
    }
    (inputs, clones)
}

/// Sync's clone step: each source cloned and at its pin, with any manifests colctl writes.
/// Returns the failures, by source name.
fn ensure_sources(ctx: &Ctx, git: &Git, config: &Config) -> BTreeMap<String, String> {
    let vault = Vault {
        git,
        dir: plugins_dir(ctx),
    };
    let mut errors = BTreeMap::new();
    for s in &config.sources {
        match vault.ensure(&s.name, &s.clone_url(), &s.rev) {
            Ok(path) => {
                packaging::inspect(
                    &path,
                    &manifests_dir(ctx, &s.name),
                    &s.name,
                    &s.plugin,
                    s.path.as_deref(),
                );
            }
            Err(e) => {
                errors.insert(s.name.clone(), e);
            }
        }
    }
    errors
}

struct Planned {
    harness: Harness,
    plan: HarnessPlan,
}

fn check_harness(config: &Config, filter: Option<&str>) -> Result<(), CliError> {
    match filter {
        Some(name) if config.harness(name).is_none() => {
            let known: Vec<&str> = config.harnesses.iter().map(|h| h.name.as_str()).collect();
            Err(CliError::usage(format!(
                "unknown harness \"{name}\"; known harnesses: {}",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            )))
        }
        _ => Ok(()),
    }
}

fn plan_all(ctx: &Ctx, ws: &Workspace, filter: Option<&str>) -> Result<Vec<Planned>, CliError> {
    check_harness(&ws.config, filter)?;
    let display = ctx.source_display();
    let generated = adapter::generated_claude_md(&display);
    let codex_agents = codex_agents(ctx, &ws.source, &display);
    let inputs = Inputs {
        config: &ws.config,
        source: &ws.source,
        source_dir: &ctx.source,
        source_display: &display,
        generated: &generated,
        codex_agents: &codex_agents,
        sources: &ws.sources,
    };
    Ok(ws
        .config
        .harnesses
        .iter()
        .filter(|h| filter.is_none_or(|f| f == h.name))
        .map(|h| {
            let scan = adapter::scan_harness(h.kind, &h.path, &ctx.source);
            Planned {
                harness: h.clone(),
                plan: plan::plan(h, &scan, &ws.state.harness(&h.name), &inputs),
            }
        })
        .collect())
}

/// Every source agent rendered as Codex TOML, or why it can't be.
fn codex_agents(
    ctx: &Ctx,
    source: &Source,
    display: &str,
) -> std::collections::BTreeMap<String, Result<Vec<u8>, String>> {
    source
        .agents
        .iter()
        .map(|name| {
            let path = ctx.source.join(AGENTS).join(format!("{name}.md"));
            let rendered = fs::read_to_string(&path)
                .map_err(|e| format!("reading {}: {e}", path.display()))
                .and_then(|md| crate::convert::codex_agent(&md, name, display));
            (name.clone(), rendered)
        })
        .collect()
}

fn root_str(root: Root) -> &'static str {
    match root {
        Root::Ok => "ok",
        Root::Missing => "harness-missing",
        Root::Invalid => "invalid",
    }
}

fn root_hint(root: Root) -> &'static str {
    match root {
        Root::Ok => "",
        Root::Missing => "start that profile once so it creates its folder",
        Root::Invalid => "the path must be a real folder, not a file or a symlink",
    }
}

fn item_line(item: &Item) -> String {
    let hint = item
        .hint
        .as_ref()
        .map(|h| format!("  → {h}"))
        .unwrap_or_default();
    format!(
        "  {:<20} {:<13} {}{hint}",
        item.state.as_str(),
        item.kind.as_str(),
        item.name
    )
}

fn harness_heading(ctx: &Ctx, p: &Planned) -> String {
    let mut line = format!("{} ({})", p.harness.name, ctx.tilde(&p.harness.path));
    if p.plan.root != Root::Ok {
        line.push_str(&format!(
            "  {} → {}",
            root_str(p.plan.root),
            root_hint(p.plan.root)
        ));
    }
    line
}

fn in_sync(ws: &Workspace, planned: &[Planned], uncommitted: &[String]) -> bool {
    uncommitted.is_empty()
        && ws.source.invalid.is_empty()
        && ws.origin_changed.is_empty()
        && planned.iter().all(|p| p.plan.in_sync())
}

/// Adopted items whose origin moved on while the source copy stayed as adopted (AC2-26).
fn origin_changes(ctx: &Ctx, state: &State) -> Vec<(String, PathBuf)> {
    state
        .origins
        .iter()
        .filter(|(key, origin)| {
            let is_dir = key.starts_with("skills/");
            let now = hash_item(&origin.path, is_dir).ok();
            let ours = hash_item(&ctx.source.join(key.as_str()), is_dir).ok();
            now.is_some_and(|h| h != origin.hash) && ours.as_ref() == Some(&origin.hash)
        })
        .map(|(key, origin)| (key.clone(), origin.path.clone()))
        .collect()
}

fn uncommitted(ctx: &Ctx, git: Option<&Git>) -> Vec<String> {
    match git {
        Some(git) if Git::is_repo(&ctx.source) => git.dirty_paths(&ctx.source).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn status_json(ctx: &Ctx, ws: &Workspace, planned: &[Planned], uncommitted: &[String]) -> Value {
    json!({
        "version": 1,
        "source": {
            "path": ctx.source,
            "uncommitted": uncommitted,
            "invalid": ws.source.invalid.iter().map(|i| json!({
                "kind": i.kind, "name": i.name, "reason": i.reason,
            })).collect::<Vec<_>>(),
            "origin_changed": ws.origin_changed.iter().map(|(item, origin)| json!({
                "item": item, "origin": origin,
            })).collect::<Vec<_>>(),
        },
        "harnesses": planned.iter().map(|p| json!({
            "name": p.harness.name,
            "kind": p.harness.kind.as_str(),
            "path": p.harness.path,
            "state": root_str(p.plan.root),
            "items": p.plan.items.iter().map(|i| json!({
                "kind": i.kind.as_str(),
                "name": i.name,
                "state": i.state.as_str(),
                "hint": i.hint,
                "target": i.target,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "sources": ws.config.sources.iter().zip(&ws.clones).map(|(s, (_, clone))| json!({
            "name": s.name, "repo": s.repo, "rev": s.rev, "clone": clone,
        })).collect::<Vec<_>>(),
        "in_sync": in_sync(ws, planned, uncommitted),
    })
}

fn print_status(ctx: &Ctx, ws: &Workspace, planned: &[Planned], uncommitted: &[String]) {
    println!("source {}", ctx.tilde(&ctx.source));
    for path in uncommitted {
        println!("  {:<20} {path}  → colctl sync commits it", "uncommitted");
    }
    for i in &ws.source.invalid {
        println!(
            "  {:<20} {:<13} {}  → {}",
            "invalid", i.kind, i.name, i.reason
        );
    }
    for (item, origin) in &ws.origin_changed {
        println!(
            "  {:<20} {item}  → {} changed; colctl refresh {item}",
            "origin-changed",
            origin.display()
        );
    }
    if planned.is_empty() {
        println!("no harnesses registered; add one to {CONFIG_FILE}");
    }
    for p in planned {
        println!("{}", harness_heading(ctx, p));
        for item in &p.plan.items {
            println!("{}", item_line(item));
        }
    }
    println!(
        "{}",
        if in_sync(ws, planned, uncommitted) {
            "in sync"
        } else {
            "not in sync"
        }
    );
}

/// `colctl status` (AC-08). Never writes anything.
pub fn status(ctx: &Ctx, filter: Option<&str>, as_json: bool) -> Result<i32, CliError> {
    let ws = load(ctx)?;
    let planned = plan_all(ctx, &ws, filter)?;
    let git = Git::find(&ctx.path_var).ok();
    let dirty = uncommitted(ctx, git.as_ref());
    if as_json {
        let mut text = serde_json::to_string_pretty(&status_json(ctx, &ws, &planned, &dirty))
            .map_err(|e| CliError::usage(e.to_string()))?;
        text.push('\n');
        print!("{text}");
    } else {
        print_status(ctx, &ws, &planned, &dirty);
    }
    Ok(if in_sync(&ws, &planned, &dirty) {
        0
    } else {
        EXIT_DRIFT
    })
}

fn print_plan(ctx: &Ctx, planned: &[Planned]) {
    for p in planned {
        println!("{}", harness_heading(ctx, p));
        if p.plan.actions.is_empty() {
            println!("  nothing to do");
        }
        for action in &p.plan.actions {
            println!(
                "  would {}",
                action.describe(Tool::for_kind(p.harness.kind))
            );
        }
    }
}

fn cleanup_source_temps(ctx: &Ctx) {
    for dir in [
        ctx.source.clone(),
        ctx.source.join(SKILLS),
        ctx.source.join(AGENTS),
    ] {
        for temp in fsx::find_temps(&dir) {
            let _ = fsx::remove_any(&temp);
        }
    }
}

fn commit(git: &Git, ctx: &Ctx, message: &str) -> Result<bool, CliError> {
    git.commit_all(&ctx.source, message)
        .map_err(|e| CliError::usage(format!("{e:#}")))
}

/// Commits edits made in the source since the last colctl run (AC-17).
fn commit_pending(git: &Git, ctx: &Ctx) -> Result<(), CliError> {
    cleanup_source_temps(ctx);
    if Git::is_repo(&ctx.source) {
        commit(git, ctx, "colctl: sync")?;
    }
    Ok(())
}

/// The body of `sync`, run with the lock held.
fn run_sync(ctx: &Ctx, git: &Git, filter: Option<&str>) -> Result<i32, CliError> {
    // Layout first, so a new ignore rule (e.g. plugins/ for sources) is part of this commit.
    ensure_layout(ctx)?;
    commit_pending(git, ctx)?;
    let mut ws = load(ctx)?;
    let errors = ensure_sources(ctx, git, &ws.config);
    (ws.sources, ws.clones) = prepare_sources(ctx, &ws.config, Some(git), &errors);
    let planned = plan_all(ctx, &ws, filter)?;
    let mut failures = 0;
    for p in &planned {
        if p.plan.root != Root::Ok {
            continue;
        }
        if p.plan.actions.is_empty() {
            continue;
        }
        println!("{}", harness_heading(ctx, p));
        let results = apply::apply(
            ctx,
            &p.harness,
            &p.plan.actions,
            ws.state.harness_mut(&p.harness.name),
        );
        for r in results {
            match r.outcome {
                Outcome::Done => println!("  ✓ {}", r.description),
                Outcome::Failed(reason) => {
                    failures += 1;
                    println!("  ✗ {} — {reason}", r.description);
                }
                Outcome::Skipped(reason) => {
                    failures += 1;
                    println!("  - {} — skipped: {reason}", r.description);
                }
            }
        }
    }
    remove_stale_clones(ctx, &ws.config);
    state::save(&ctx.source, &ws.state)
        .map_err(|e| CliError::usage(format!("saving state: {e:#}")))?;

    let after = load(ctx)?;
    let replanned = plan_all(ctx, &after, filter)?;
    let mut remaining = Vec::new();
    for p in &replanned {
        if p.plan.root != Root::Ok {
            remaining.push(harness_heading(ctx, p));
        }
        for item in p.plan.items.iter().filter(|i| i.state != ItemState::Ok) {
            remaining.push(format!(
                "{}:{}",
                p.harness.name,
                item_line(item).trim_start_matches(' ')
            ));
        }
    }
    for i in &after.source.invalid {
        remaining.push(format!(
            "source: invalid {} {}  → {}",
            i.kind, i.name, i.reason
        ));
    }
    for (item, _) in &after.origin_changed {
        remaining.push(format!(
            "source: origin-changed {item}  → colctl refresh {item}"
        ));
    }
    if !remaining.is_empty() {
        println!("needs attention:");
        for line in &remaining {
            println!("  {line}");
        }
    }
    let ok = failures == 0 && in_sync(&after, &replanned, &[]);
    println!("{}", if ok { "in sync" } else { "not in sync" });
    Ok(if ok { 0 } else { EXIT_DRIFT })
}

/// `colctl sync` (AC-09 … AC-19).
pub fn sync(ctx: &Ctx, filter: Option<&str>, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    check_harness(&ws.config, filter)?;
    if dry_run {
        print_plan(ctx, &plan_all(ctx, &ws, filter)?);
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    run_sync(ctx, &git, filter)
}

fn detect(ctx: &Ctx) -> Vec<(String, String, PathBuf, Kind)> {
    KNOWN_HARNESSES
        .iter()
        .map(|(name, rel, kind)| {
            (
                name.to_string(),
                format!("~/{rel}"),
                ctx.home.join(rel),
                *kind,
            )
        })
        .filter(|(_, _, path, _)| fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()))
        .collect()
}

fn ensure_layout(ctx: &Ctx) -> Result<(), CliError> {
    let io =
        |e: std::io::Error| CliError::usage(format!("setting up {}: {e}", ctx.source.display()));
    for dir in [SKILLS, AGENTS, STATE_DIR] {
        fs::create_dir_all(ctx.source.join(dir)).map_err(io)?;
    }
    let ignore = ctx.source.join(".gitignore");
    let mut text = fs::read_to_string(&ignore).unwrap_or_default();
    let mut changed = false;
    for entry in [".state/", "plugins/"] {
        let bare = entry.trim_end_matches('/');
        if !text.lines().any(|l| l.trim() == entry || l.trim() == bare) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(entry);
            text.push('\n');
            changed = true;
        }
    }
    if changed {
        fs::write(&ignore, text).map_err(io)?;
    }
    Ok(())
}

/// Deletes clones (and colctl-written manifests) of sources no longer declared (AC3-13).
fn remove_stale_clones(ctx: &Ctx, config: &Config) {
    let declared: Vec<&str> = config.sources.iter().map(|s| s.name.as_str()).collect();
    for name in source::visible_entries(&plugins_dir(ctx)) {
        if !declared.contains(&name.as_str()) && vault::is_clone(&plugins_dir(ctx).join(&name)) {
            let _ = fsx::remove_any(&plugins_dir(ctx).join(&name));
        }
    }
    for name in source::visible_entries(&plugins_dir(ctx).join(MANIFESTS)) {
        if !declared.contains(&name.as_str()) {
            let _ = fsx::remove_any(&manifests_dir(ctx, &name));
        }
    }
}

/// `colctl init` (AC-04 … AC-07).
pub fn init(ctx: &Ctx, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let found = detect(ctx);
    let config_path = ctx.source.join(CONFIG_FILE);
    if !config_path.exists() {
        if dry_run {
            println!(
                "would create {} with {CONFIG_FILE}, skills/, agents/, .state/, .gitignore and a git repo",
                ctx.tilde(&ctx.source)
            );
            for (name, written, _, _) in &found {
                println!("would register harness {name} ({written})");
            }
            if found.is_empty() {
                println!("found no known harnesses");
            }
            return Ok(0);
        }
        fs::create_dir_all(ctx.source.join(STATE_DIR))
            .map_err(|e| CliError::usage(format!("creating {}: {e}", ctx.source.display())))?;
        let _lock = Lock::acquire(&ctx.source)?;
        ensure_layout(ctx)?;
        let harnesses: Vec<(String, String, Kind)> = found
            .iter()
            .map(|(n, w, _, k)| (n.clone(), w.clone(), *k))
            .collect();
        fsx::atomic_write(
            &config_path,
            config::initial_text_with(&harnesses).as_bytes(),
        )
        .map_err(|e| CliError::usage(format!("{e:#}")))?;
        if !Git::is_repo(&ctx.source) {
            git.init(&ctx.source)
                .map_err(|e| CliError::usage(format!("{e:#}")))?;
        }
        commit(&git, ctx, "colctl: init")?;
        println!("created {}", ctx.tilde(&ctx.source));
    } else {
        let (config, text) = config::load(&ctx.source, &ctx.home)?;
        let registered: Vec<PathBuf> = config
            .harnesses
            .iter()
            .map(|h| fsx::canonical_or_normal(&h.path))
            .collect();
        let mut taken: Vec<String> = config.harnesses.iter().map(|h| h.name.clone()).collect();
        let mut additions = Vec::new();
        for (name, written, path, kind) in &found {
            if registered.contains(&fsx::canonical_or_normal(path)) {
                continue;
            }
            let mut unique = name.clone();
            let mut n = 2;
            while taken.contains(&unique) {
                unique = format!("{name}-{n}");
                n += 1;
            }
            taken.push(unique.clone());
            additions.push((unique, written.clone(), *kind));
        }
        if dry_run {
            for (name, written, _) in &additions {
                println!("would register harness {name} ({written})");
            }
            if additions.is_empty() {
                println!(
                    "nothing to add; {} is already set up",
                    ctx.tilde(&ctx.source)
                );
            }
            return Ok(0);
        }
        let _lock = Lock::acquire(&ctx.source)?;
        ensure_layout(ctx)?;
        if !additions.is_empty() {
            let mut updated = text.clone();
            for (name, written, kind) in &additions {
                updated = config::append_harness_kind(&updated, name, written, *kind);
                println!("registered harness {name} ({written})");
            }
            fsx::atomic_write(&config_path, updated.as_bytes())
                .map_err(|e| CliError::usage(format!("{e:#}")))?;
        }
        if !Git::is_repo(&ctx.source) {
            git.init(&ctx.source)
                .map_err(|e| CliError::usage(format!("{e:#}")))?;
        }
        commit(&git, ctx, "colctl: init")?;
    }
    status(ctx, None, false)?;
    Ok(0)
}

fn dry_or_write(dry_run: bool, what: &str) -> bool {
    if dry_run {
        println!("would {what}, commit, then sync");
    }
    dry_run
}

/// The name a skill gets from where it's added from.
fn skill_name_from(source: &str, subdir: Option<&str>) -> String {
    let base = subdir.unwrap_or(source).trim_end_matches('/');
    let last = base.rsplit(['/', ':']).next().unwrap_or(base);
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

fn is_url(source: &str) -> bool {
    source.contains("://") || source.starts_with("git@")
}

/// `colctl add skill` (AC-20).
pub fn add_skill(
    ctx: &Ctx,
    from: &str,
    subdir: Option<&str>,
    name: Option<&str>,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    load(ctx)?;
    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| skill_name_from(from, subdir));
    if !config::valid_item_name(&name) {
        return Err(CliError::usage(format!(
            "invalid skill name \"{name}\"; pass --name with letters, digits, '.', '_' or '-'"
        )));
    }
    let dest = ctx.source.join(SKILLS).join(&name);
    if fsx::exists_any(&dest) {
        return Err(CliError::usage(format!(
            "skill \"{name}\" is already managed"
        )));
    }
    let shown = match subdir {
        Some(s) => format!("{from} ({s})"),
        None => from.to_string(),
    };
    if dry_or_write(dry_run, &format!("add skill {name} from {shown}")) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let clone_dir = tempfile::tempdir().map_err(|e| CliError::usage(e.to_string()))?;
    let root = if is_url(from) {
        let repo = clone_dir.path().join("repo");
        git.shallow_clone(from, &repo)
            .map_err(|e| CliError::usage(format!("cloning {from}: {e:#}")))?;
        repo
    } else {
        PathBuf::from(from)
    };
    let base = match subdir {
        Some(s) => root.join(s),
        None => root,
    };
    if !base.join("SKILL.md").is_file() {
        return Err(CliError::usage(format!(
            "no SKILL.md in {}",
            base.display()
        )));
    }
    fsx::copy_tree(&base, &dest, true).map_err(|e| CliError::usage(format!("{e:#}")))?;
    commit(&git, ctx, &format!("colctl: add skill {name}"))?;
    println!("added skill {name}");
    run_sync(ctx, &git, None)
}

/// `colctl add agent` (AC-21).
pub fn add_agent(ctx: &Ctx, file: &Path, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    load(ctx)?;
    let stem = file
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".md"))
        .ok_or_else(|| CliError::usage(format!("{} must be a .md file", file.display())))?;
    if !config::valid_item_name(stem) {
        return Err(CliError::usage(format!("invalid agent name \"{stem}\"")));
    }
    if !file.is_file() {
        return Err(CliError::usage(format!("{} isn't a file", file.display())));
    }
    let dest = ctx.source.join(AGENTS).join(format!("{stem}.md"));
    if fsx::exists_any(&dest) {
        return Err(CliError::usage(format!(
            "agent \"{stem}\" is already managed"
        )));
    }
    if dry_or_write(
        dry_run,
        &format!("add agent {stem} from {}", file.display()),
    ) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    fs::create_dir_all(dest.parent().unwrap_or(&ctx.source))
        .and_then(|_| fs::copy(file, &dest))
        .map_err(|e| CliError::usage(format!("copying {}: {e}", file.display())))?;
    commit(&git, ctx, &format!("colctl: add agent {stem}"))?;
    println!("added agent {stem}");
    run_sync(ctx, &git, None)
}

fn write_config(ctx: &Ctx, text: &str) -> Result<(), CliError> {
    config::parse(text, &ctx.home).map_err(CliError::usage)?;
    fsx::atomic_write(&ctx.source.join(CONFIG_FILE), text.as_bytes())
        .map_err(|e| CliError::usage(format!("{e:#}")))
}

/// `colctl add plugin` (AC-22), and with `--codex` for the Codex list (AC2-17).
pub fn add_plugin(
    ctx: &Ctx,
    id: &str,
    marketplace: Option<&str>,
    list: PluginList,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let (_, market) = config::split_plugin_id(id)
        .ok_or_else(|| CliError::usage(format!("plugin id \"{id}\" must be name@marketplace")))?;
    let (markets, plugins) = ws.config.plugin_list(list);
    if plugins.iter().any(|p| p == id) {
        return Err(CliError::usage(format!("plugin {id} is already enabled")));
    }
    let builtin = list.builtin();
    let new_market = match (marketplace, markets.get(market)) {
        (Some(_), _) if market == builtin => {
            return Err(CliError::usage(format!("{builtin} needs no --marketplace")));
        }
        (Some(src), Some(existing)) if src != existing => {
            return Err(CliError::usage(format!(
                "marketplace \"{market}\" is already {existing} in {CONFIG_FILE}"
            )));
        }
        (Some(src), None) => {
            if !config::valid_marketplace_source(src) {
                return Err(CliError::usage(format!(
                    "marketplace source \"{src}\" must be github:owner/repo, an http(s) URL or an absolute path"
                )));
            }
            Some((market, src))
        }
        (None, None) if market != builtin => {
            return Err(CliError::usage(format!(
                "unknown marketplace \"{market}\"; pass --marketplace <github:owner/repo | URL | path>"
            )));
        }
        _ => None,
    };
    let text =
        config::add_plugin_to(&ws.config_text, list, id, new_market).map_err(CliError::usage)?;
    let label = plugin_label(list);
    if dry_or_write(dry_run, &format!("enable {label} {id} in {CONFIG_FILE}")) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    write_config(ctx, &text)?;
    commit(&git, ctx, &format!("colctl: add {label} {id}"))?;
    println!("added {label} {id}");
    run_sync(ctx, &git, None)
}

fn plugin_label(list: PluginList) -> &'static str {
    match list {
        PluginList::Claude => "plugin",
        PluginList::Codex => "codex plugin",
    }
}

/// What `rm` removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmKind {
    Skill,
    Agent,
    Plugin,
    Source,
}

/// `colctl rm` (AC-23).
pub fn rm(
    ctx: &Ctx,
    kind: RmKind,
    name: &str,
    list: PluginList,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    if list == PluginList::Codex && kind != RmKind::Plugin {
        return Err(CliError::usage("--codex only applies to plugins"));
    }
    let (label, path) = match kind {
        RmKind::Skill => ("skill", Some(ctx.source.join(SKILLS).join(name))),
        RmKind::Agent => (
            "agent",
            Some(ctx.source.join(AGENTS).join(format!("{name}.md"))),
        ),
        RmKind::Plugin => (plugin_label(list), None),
        RmKind::Source => ("source", None),
    };
    let known = match (&path, kind) {
        (Some(p), _) => config::valid_item_name(name) && fsx::exists_any(p),
        (None, RmKind::Source) => ws.config.sources.iter().any(|s| s.name == name),
        (None, _) => ws.config.plugin_list(list).1.iter().any(|p| p == name),
    };
    if !known {
        return Err(CliError::usage(format!(
            "no managed {label} named \"{name}\""
        )));
    }
    if dry_or_write(dry_run, &format!("remove {label} {name}")) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    match &path {
        Some(p) => fsx::remove_any(p).map_err(|e| CliError::usage(format!("{e:#}")))?,
        None => {
            let text = if kind == RmKind::Source {
                config::remove_source(&ws.config_text, name)
            } else {
                config::remove_plugin_from(&ws.config_text, list, name)
            }
            .map_err(CliError::usage)?;
            write_config(ctx, &text)?;
        }
    }
    commit(&git, ctx, &format!("colctl: rm {label} {name}"))?;
    println!("removed {label} {name}");
    run_sync(ctx, &git, None)
}

/// The result of adopting one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adoption {
    /// Copied into the source and replaced with a link.
    Adopted,
    /// Already identical in the source; replaced with a link.
    Linked,
    /// A tool's link pointed back at the source copy; the note says what's left to do (AC2-27).
    Relinked(String),
    Refused(String),
}

type Copier = dyn Fn(&Path, &Path) -> anyhow::Result<()>;
type ReadPlugins = fn(&Path) -> Result<adapter::Plugins, String>;
type ToSource = fn(&Value) -> Option<String>;

fn hash_item(path: &Path, is_dir: bool) -> anyhow::Result<String> {
    if is_dir {
        fsx::hash_tree(path)
    } else {
        fsx::hash_file(path)
    }
}

/// Adopts a real skill folder or agent file at `item` into `target` (AC-24, AC-25, EC-29).
pub fn adopt_entry(item: &Path, target: &Path, is_dir: bool, copy: &Copier) -> Adoption {
    let refuse = |e: anyhow::Error| Adoption::Refused(format!("{e:#}"));
    let original = match hash_item(item, is_dir) {
        Ok(h) => h,
        Err(e) => return refuse(e),
    };
    if fsx::exists_any(target) {
        return match hash_item(target, is_dir) {
            Ok(h) if h == original => match fsx::replace_with_link(item, target) {
                Ok(()) => Adoption::Linked,
                Err(e) => refuse(e),
            },
            _ => Adoption::Refused(format!(
                "differs from the source copy; both kept: {} and {}",
                item.display(),
                target.display()
            )),
        };
    }
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staging = target.with_file_name(format!("{}adopt-{name}", fsx::TMP_PREFIX));
    let _ = fsx::remove_any(&staging);
    if let Err(e) = copy(item, &staging) {
        let _ = fsx::remove_any(&staging);
        return refuse(e);
    }
    if hash_item(&staging, is_dir).ok().as_ref() != Some(&original) {
        let _ = fsx::remove_any(&staging);
        return Adoption::Refused("the copy didn't match the original; nothing changed".into());
    }
    if let Err(e) = fs::rename(&staging, target) {
        let _ = fsx::remove_any(&staging);
        return Adoption::Refused(format!("moving the copy into place: {e}"));
    }
    match fsx::replace_with_link(item, target) {
        Ok(()) => Adoption::Adopted,
        Err(e) => refuse(e),
    }
}

fn real_copy(is_dir: bool) -> impl Fn(&Path, &Path) -> anyhow::Result<()> {
    move |from: &Path, to: &Path| {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        if is_dir {
            fsx::copy_tree(from, to, false)
        } else {
            fs::copy(from, to).map(|_| ()).map_err(Into::into)
        }
    }
}

/// A Codex profile's real `AGENTS.md`: its text becomes the source's, and it becomes a link (AC2-06).
fn adopt_codex_instructions(ctx: &Ctx, harness: &Harness) -> Adoption {
    let item = harness.path.join(crate::adapter::codex::AGENTS_MD);
    let agents_md = ctx.source.join(AGENTS_MD);
    let content = match fs::read(&item) {
        Ok(c) => c,
        Err(e) => return Adoption::Refused(format!("reading {}: {e}", item.display())),
    };
    let mut result = Adoption::Adopted;
    if agents_md.exists() {
        if fs::read(&agents_md).ok().as_deref() != Some(content.as_slice()) {
            return Adoption::Refused(format!(
                "{AGENTS_MD} already exists with different text; both kept: {} and {}",
                item.display(),
                agents_md.display()
            ));
        }
        result = Adoption::Linked;
    } else if let Err(e) = fsx::atomic_write(&agents_md, &content) {
        return Adoption::Refused(format!("{e:#}"));
    }
    match fsx::replace_with_link(&item, &agents_md) {
        Ok(()) => result,
        Err(e) => Adoption::Refused(format!("{e:#}")),
    }
}

fn adopt_instructions(ctx: &Ctx, harness: &Harness) -> Adoption {
    if harness.kind == Kind::Codex {
        return adopt_codex_instructions(ctx, harness);
    }
    let claude_md = harness.path.join(CLAUDE_MD);
    let agents_md = ctx.source.join(AGENTS_MD);
    let content = match fs::read(&claude_md) {
        Ok(c) => c,
        Err(e) => return Adoption::Refused(format!("reading {}: {e}", claude_md.display())),
    };
    let mut result = Adoption::Adopted;
    if agents_md.exists() {
        if fs::read(&agents_md).ok().as_deref() != Some(content.as_slice()) {
            return Adoption::Refused(format!(
                "{} already exists with different text; both kept: {} and {}",
                AGENTS_MD,
                claude_md.display(),
                agents_md.display()
            ));
        }
        result = Adoption::Linked;
    } else if let Err(e) = fsx::atomic_write(&agents_md, &content) {
        return Adoption::Refused(format!("{e:#}"));
    }
    let generated = adapter::generated_claude_md(&ctx.source_display());
    if let Err(e) = fsx::atomic_write(&claude_md, &generated) {
        return Adoption::Refused(format!("{e:#}"));
    }
    let mut st = match state::load(&ctx.source) {
        Ok(s) => s,
        Err(e) => return Adoption::Refused(e.message),
    };
    st.harness_mut(&harness.name)
        .generated
        .insert(CLAUDE_MD.into(), fsx::hash_bytes(&generated));
    if let Err(e) = state::save(&ctx.source, &st) {
        return Adoption::Refused(format!("{e:#}"));
    }
    result
}

fn adopt_plugin(ctx: &Ctx, harness: &Harness, id: &str) -> Adoption {
    let ws = match load(ctx) {
        Ok(ws) => ws,
        Err(e) => return Adoption::Refused(e.message),
    };
    let Some((_, market)) = config::split_plugin_id(id) else {
        return Adoption::Refused(format!("\"{id}\" isn't a plugin id"));
    };
    let Some(list) = PluginList::for_kind(harness.kind) else {
        return Adoption::Refused(format!(
            "{} harnesses have no plugins",
            harness.kind.as_str()
        ));
    };
    let mut new_market = None;
    if market != list.builtin() && !ws.config.plugin_list(list).0.contains_key(market) {
        let (read, to_source): (ReadPlugins, ToSource) = match list {
            PluginList::Claude => (adapter::read_plugins, adapter::marketplace_source),
            PluginList::Codex => (
                adapter::codex::read_plugins,
                adapter::codex::marketplace_source,
            ),
        };
        let source = read(&harness.path)
            .ok()
            .and_then(|p| p.marketplaces.get(market).and_then(to_source));
        match source {
            Some(src) if config::valid_marketplace_source(&src) => new_market = Some(src),
            _ => {
                return Adoption::Refused(format!(
                    "can't tell where marketplace \"{market}\" comes from; use colctl add plugin {id} --marketplace <source>"
                ));
            }
        }
    }
    let text = match config::add_plugin_to(
        &ws.config_text,
        list,
        id,
        new_market.as_deref().map(|s| (market, s)),
    ) {
        Ok(t) => t,
        Err(e) => return Adoption::Refused(e),
    };
    match write_config(ctx, &text) {
        Ok(()) => Adoption::Adopted,
        Err(e) => Adoption::Refused(e.message),
    }
}

fn codex_agent_refusal(ctx: &Ctx) -> Adoption {
    Adoption::Refused(format!(
        "Codex agents can't be converted back to .md; recreate it in {}/agents",
        ctx.source_display()
    ))
}

/// Takes over a skill or agent that a tool linked in from its own folder (AC2-25…28).
fn take_over(ctx: &Ctx, item: &Item, link: &Path) -> Adoption {
    let is_dir = item.kind == ItemKind::Skill;
    let key = if is_dir {
        format!("{SKILLS}/{}", item.name)
    } else {
        format!("{AGENTS}/{}.md", item.name)
    };
    let shown = fsx::link_target(link)
        .map(|t| t.display().to_string())
        .unwrap_or_default();
    let Ok(target) = fs::canonicalize(link) else {
        return Adoption::Refused(format!("it links to {shown}, which doesn't exist"));
    };
    if target.is_dir() != is_dir {
        return Adoption::Refused(if is_dir {
            "it links to a file, not a skill folder".into()
        } else {
            "it links to a folder, not an agent file".into()
        });
    }
    let refuse = |e: anyhow::Error| Adoption::Refused(format!("{e:#}"));
    let theirs = match hash_item(&target, is_dir) {
        Ok(h) => h,
        Err(e) => return refuse(e),
    };
    let mut st = match state::load(&ctx.source) {
        Ok(s) => s,
        Err(e) => return Adoption::Refused(e.message),
    };
    let source_item = ctx.source.join(&key);
    let outcome = if fsx::exists_any(&source_item) {
        let ours = hash_item(&source_item, is_dir).ok();
        if ours.as_ref() == Some(&theirs) {
            Adoption::Linked
        } else if st.origins.get(&key).is_some_and(|o| o.path == target) {
            Adoption::Relinked(format!("origin has changes; run colctl refresh {key}"))
        } else {
            return Adoption::Refused(format!(
                "differs from the source copy; both kept: {} and {}",
                target.display(),
                source_item.display()
            ));
        }
    } else {
        let staging = source_item.with_file_name(format!("{}adopt-{}", fsx::TMP_PREFIX, item.name));
        let _ = fsx::remove_any(&staging);
        if let Err(e) = real_copy(is_dir)(&target, &staging) {
            let _ = fsx::remove_any(&staging);
            return refuse(e);
        }
        if hash_item(&staging, is_dir).ok().as_ref() != Some(&theirs) {
            let _ = fsx::remove_any(&staging);
            return Adoption::Refused("the copy didn't match the original; nothing changed".into());
        }
        if let Err(e) = fs::rename(&staging, &source_item) {
            let _ = fsx::remove_any(&staging);
            return Adoption::Refused(format!("moving the copy into place: {e}"));
        }
        Adoption::Adopted
    };
    if let Err(e) = fsx::atomic_symlink(&source_item, link) {
        return refuse(e);
    }
    st.origins.entry(key).or_insert(Origin {
        path: target,
        hash: theirs,
    });
    match state::save(&ctx.source, &st) {
        Ok(()) => outcome,
        Err(e) => refuse(e),
    }
}

fn adopt_link(ctx: &Ctx, harness: &Harness, item: &Item) -> Adoption {
    if item.kind == ItemKind::Agent && harness.kind == Kind::Codex {
        return codex_agent_refusal(ctx);
    }
    if !item.can_take_over() {
        return Adoption::Refused("remove or replace this link yourself".into());
    }
    let link = harness.path.join(&item.address);
    take_over(ctx, item, &link)
}

fn adopt_item(ctx: &Ctx, harness: &Harness, item: &Item) -> Adoption {
    if item.kind == ItemKind::Agent && harness.kind == Kind::Codex {
        return codex_agent_refusal(ctx);
    }
    match item.kind {
        ItemKind::Skill => adopt_entry(
            &harness.path.join(SKILLS).join(&item.name),
            &ctx.source.join(SKILLS).join(&item.name),
            true,
            &real_copy(true),
        ),
        ItemKind::Agent => adopt_entry(
            &harness.path.join(AGENTS).join(format!("{}.md", item.name)),
            &ctx.source.join(AGENTS).join(format!("{}.md", item.name)),
            false,
            &real_copy(false),
        ),
        ItemKind::Instructions => adopt_instructions(ctx, harness),
        ItemKind::Plugin => match adopt_plugin_from_source(ctx, harness, &item.name) {
            Some(outcome) => outcome,
            None => adopt_plugin(ctx, harness, &item.name),
        },
        ItemKind::Marketplace => {
            Adoption::Refused("marketplaces are adopted with their plugins".into())
        }
        ItemKind::Source => Adoption::Refused("source plugins are already colctl's".into()),
    }
}

fn parse_item(words: &[String]) -> Result<Option<String>, CliError> {
    match words {
        [] => Ok(None),
        [one]
            if one == CLAUDE_MD
                || one == AGENTS_MD
                || one.starts_with("skills/")
                || one.starts_with("agents/") =>
        {
            Ok(Some(one.clone()))
        }
        [kind, id] if kind == "plugin" => Ok(Some(format!("plugin {id}"))),
        _ => Err(CliError::usage(format!(
            "unknown item \"{}\"; use skills/<name>, agents/<name>.md|.toml, CLAUDE.md, AGENTS.md or plugin <id>",
            words.join(" ")
        ))),
    }
}

/// `colctl adopt` (AC-24 … AC-28).
pub fn adopt(
    ctx: &Ctx,
    harness: Option<&str>,
    item: &[String],
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    check_harness(&ws.config, harness)?;
    let wanted = parse_item(item)?;
    if wanted.is_some() && harness.is_none() {
        return Err(CliError::usage(
            "name the harness before the item: colctl adopt <harness> <item>",
        ));
    }
    let matches = |i: &Item| wanted.as_ref().is_none_or(|w| *w == i.address());
    if dry_run {
        for p in plan_all(ctx, &ws, harness)? {
            for i in p.plan.items.iter().filter(|i| matches(i)) {
                match i.state {
                    ItemState::Foreign | ItemState::FromAggregator => {
                        println!("would adopt {} from {}", i.address(), p.harness.name)
                    }
                    ItemState::Ok if i.kind == ItemKind::Plugin && i.target.is_some() => println!(
                        "would move {} in {} to its source",
                        i.address(),
                        p.harness.name
                    ),
                    ItemState::ForeignLink if i.can_take_over() => println!(
                        "would take over {} in {} from {}",
                        i.address(),
                        p.harness.name,
                        i.target
                            .as_ref()
                            .map(|t| t.display().to_string())
                            .unwrap_or_default()
                    ),
                    _ => {}
                }
            }
        }
        println!("then sync");
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let mut refused = 0;
    let mut seen = false;
    let names: Vec<String> = ws
        .config
        .harnesses
        .iter()
        .filter(|h| harness.is_none_or(|f| f == h.name))
        .map(|h| h.name.clone())
        .collect();
    for name in names {
        let ws = load(ctx)?;
        let planned = plan_all(ctx, &ws, Some(&name))?;
        let p = &planned[0];
        for i in p.plan.items.iter().filter(|i| matches(i)) {
            seen = true;
            let outcome = match i.state {
                ItemState::Foreign | ItemState::FromAggregator => adopt_item(ctx, &p.harness, i),
                // AC3-29: a list plugin whose source is known moves to that source.
                ItemState::Ok if i.kind == ItemKind::Plugin && i.target.is_some() => {
                    adopt_item(ctx, &p.harness, i)
                }
                ItemState::ForeignLink => adopt_link(ctx, &p.harness, i),
                other if wanted.is_some() => {
                    Adoption::Refused(format!("not a foreign item (state: {})", other.as_str()))
                }
                _ => continue,
            };
            match outcome {
                Adoption::Adopted | Adoption::Linked | Adoption::Relinked(_) => {
                    let kind = match (i.kind, p.harness.kind) {
                        (ItemKind::Plugin, Kind::Codex) => "codex plugin",
                        (k, _) => k.as_str(),
                    };
                    let label = if i.kind == ItemKind::Instructions {
                        p.harness.name.as_str()
                    } else {
                        i.name.as_str()
                    };
                    commit(&git, ctx, &format!("colctl: adopt {kind} {label}"))?;
                    match &outcome {
                        Adoption::Adopted => {
                            println!("adopted {} from {}", i.address(), p.harness.name)
                        }
                        Adoption::Relinked(note) => println!(
                            "relinked {} in {} to the source copy; {note}",
                            i.address(),
                            p.harness.name
                        ),
                        _ => println!(
                            "linked (already identical) {} from {}",
                            i.address(),
                            p.harness.name
                        ),
                    }
                }
                Adoption::Refused(reason) => {
                    refused += 1;
                    println!("refused {} in {}: {reason}", i.address(), p.harness.name);
                }
            }
        }
    }
    if let (Some(w), false) = (&wanted, seen) {
        refused += 1;
        println!("refused {w}: not found in {}", harness.unwrap_or_default());
    }
    let code = run_sync(ctx, &git, None)?;
    Ok(if refused > 0 { EXIT_DRIFT } else { code })
}

fn head_of_dir(git: &Git, dir: &Path) -> Option<String> {
    git.run(dir, &["rev-parse", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| config::valid_rev(s))
}

fn same_repo(a: &str, b: &str) -> bool {
    let norm = |u: &str| {
        config::clone_url(u)
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_string()
    };
    norm(a) == norm(b)
}

/// Clones `repo` as source `name`, pins `rev` (or upstream's head), and checks the packaging.
/// Returns the pinned commit and the packaging; on failure the clone is removed again.
fn clone_and_check(
    ctx: &Ctx,
    git: &Git,
    name: &str,
    repo: &str,
    rev: Option<&str>,
    plugin: &str,
    subpath: Option<&str>,
) -> Result<(String, packaging::Packaging), String> {
    let vault = Vault {
        git,
        dir: plugins_dir(ctx),
    };
    let url = config::clone_url(repo);
    let cleanup = || {
        let _ = vault.remove(name);
        let _ = fsx::remove_any(&manifests_dir(ctx, name));
    };
    let result = (|| {
        vault.fetch_clone(name, &url)?;
        let pinned = match rev {
            Some(r) if config::valid_rev(r) => r.to_string(),
            Some(r) => vault.resolve(name, r)?,
            None => vault.upstream_head(name)?,
        };
        let path = vault.ensure(name, &url, &pinned)?;
        let p = packaging::inspect(&path, &manifests_dir(ctx, name), name, plugin, subpath);
        if !p.any_supported() {
            return Err(format!(
                "{repo} has no Claude or Codex packaging for plugin {plugin}"
            ));
        }
        Ok((pinned, p))
    })();
    if result.is_err() {
        cleanup();
    }
    result
}

/// `colctl add source` (AC3-17).
pub fn add_source(
    ctx: &Ctx,
    repo: &str,
    rev: Option<&str>,
    name: Option<&str>,
    plugin: Option<&str>,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    if !config::valid_repo(repo) {
        return Err(CliError::usage(format!(
            "repo \"{repo}\" must be a URL, git@host:path, an absolute path or github:owner/repo"
        )));
    }
    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| skill_name_from(repo, None));
    if !config::valid_item_name(&name) {
        return Err(CliError::usage(format!(
            "invalid source name \"{name}\"; pass --name"
        )));
    }
    if ws.config.sources.iter().any(|s| s.name == name) {
        return Err(CliError::usage(format!("source \"{name}\" already exists")));
    }
    let plugin = plugin
        .map(str::to_string)
        .unwrap_or_else(|| skill_name_from(repo, None));
    if dry_or_write(dry_run, &format!("clone {repo} as source {name}")) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let (pinned, _) =
        clone_and_check(ctx, &git, &name, repo, rev, &plugin, None).map_err(CliError::usage)?;
    let source = config::Source {
        name: name.clone(),
        repo: repo.to_string(),
        rev: pinned.clone(),
        plugin,
        replaces: Vec::new(),
        path: None,
    };
    let text = config::add_source(&ws.config_text, &source).map_err(CliError::usage)?;
    write_config(ctx, &text)?;
    commit(&git, ctx, &format!("colctl: add source {name}"))?;
    println!("added source {name} at {}", vault::short(&pinned));
    run_sync(ctx, &git, None)
}

/// Moves a plugin to its source when the source can be found (AC3-18…21); `None` to fall back
/// to adopting it into a plugin list (AC3-22).
fn adopt_plugin_from_source(ctx: &Ctx, harness: &Harness, id: &str) -> Option<Adoption> {
    let git = Git::find(&ctx.path_var).ok()?;
    let head = |dir: &Path| head_of_dir(&git, dir);
    let found = match harness.kind {
        Kind::Claude => crate::origin::claude(&harness.path, id, &head),
        Kind::Codex => crate::origin::codex(&harness.path, id, &head),
        Kind::Agents => None,
    }?;
    let ws = match load(ctx) {
        Ok(ws) => ws,
        Err(e) => return Some(Adoption::Refused(e.message)),
    };
    let list = PluginList::for_kind(harness.kind)?;
    let mut text = ws.config_text.clone();
    if ws.config.plugin_list(list).1.iter().any(|p| p == id) {
        text = match config::remove_plugin_from(&text, list, id) {
            Ok(t) => t,
            Err(e) => return Some(Adoption::Refused(e)),
        };
    }
    if let Some(existing) = ws
        .config
        .sources
        .iter()
        .find(|s| same_repo(&s.repo, &found.repo))
    {
        text = match config::add_replaces(&text, &existing.name, id) {
            Ok(t) => t,
            Err(e) => return Some(Adoption::Refused(e)),
        };
        return Some(match write_config(ctx, &text) {
            Ok(()) => Adoption::Adopted,
            Err(e) => Adoption::Refused(e.message),
        });
    }
    let name = found.plugin.clone();
    if ws.config.sources.iter().any(|s| s.name == name) {
        return Some(Adoption::Refused(format!(
            "a source named {name} already exists for another repo"
        )));
    }
    let (pinned, packaging) = match clone_and_check(
        ctx,
        &git,
        &name,
        &found.repo,
        found.rev.as_deref(),
        &found.plugin,
        found.subpath.as_deref(),
    ) {
        Ok(r) => r,
        Err(e) => return Some(Adoption::Refused(e)),
    };
    let here = match harness.kind {
        Kind::Codex => &packaging.codex,
        _ => &packaging.claude,
    };
    if !matches!(here, ToolPackaging::Supported(_)) {
        let _ = Vault {
            git: &git,
            dir: plugins_dir(ctx),
        }
        .remove(&name);
        return Some(Adoption::Refused(format!(
            "{} has no packaging for {}",
            found.repo,
            harness.kind.as_str()
        )));
    }
    if found.rev.is_none() {
        println!(
            "{id}: installed commit unknown; pinned to {}",
            vault::short(&pinned)
        );
    }
    let source = config::Source {
        name: name.clone(),
        repo: found.repo.clone(),
        rev: pinned,
        plugin: found.plugin.clone(),
        replaces: vec![id.to_string()],
        path: found.subpath.clone(),
    };
    text = match config::add_source(&text, &source) {
        Ok(t) => t,
        Err(e) => return Some(Adoption::Refused(e)),
    };
    if let Err(e) = write_config(ctx, &text) {
        return Some(Adoption::Refused(e.message));
    }
    if let Err(e) = commit(&git, ctx, &format!("colctl: adopt plugin {id} from source")) {
        return Some(Adoption::Refused(e.message));
    }
    Some(Adoption::Adopted)
}

/// `colctl refresh` (AC2-26): pulls adopted items' origins into the source.
pub fn refresh(ctx: &Ctx, items: &[String], dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let keys: Vec<String> = if items.is_empty() {
        ws.state.origins.keys().cloned().collect()
    } else {
        items.to_vec()
    };
    for key in &keys {
        if !ws.state.origins.contains_key(key) {
            return Err(CliError::usage(format!("{key} has no recorded origin")));
        }
    }
    if keys.is_empty() {
        println!("nothing to refresh: no adopted item has a recorded origin");
    }
    if dry_run {
        for (key, origin) in ws.origin_changed.iter().filter(|(k, _)| keys.contains(k)) {
            println!("would refresh {key} from {}", origin.display());
        }
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let mut refused = 0;
    for key in &keys {
        let mut st = state::load(&ctx.source)?;
        let Some(origin) = st.origins.get(key).cloned() else {
            continue;
        };
        let is_dir = key.starts_with("skills/");
        let source_item = ctx.source.join(key);
        let mut refuse = |why: String| {
            refused += 1;
            println!("refused {key}: {why}");
        };
        if !fsx::exists_any(&origin.path) {
            refuse(format!("origin is gone ({})", origin.path.display()));
            continue;
        }
        let (Ok(theirs), Ok(ours)) = (
            hash_item(&origin.path, is_dir),
            hash_item(&source_item, is_dir),
        ) else {
            refuse("couldn't read the origin or the source copy".into());
            continue;
        };
        if theirs == origin.hash {
            println!("{key} is up to date with {}", origin.path.display());
            continue;
        }
        if ours != origin.hash {
            refuse(format!(
                "changed in both places; compare {} and {}, keep one, then adopt again",
                origin.path.display(),
                source_item.display()
            ));
            continue;
        }
        let name = source_item
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let staging = source_item.with_file_name(format!("{}refresh-{name}", fsx::TMP_PREFIX));
        let _ = fsx::remove_any(&staging);
        let copied = real_copy(is_dir)(&origin.path, &staging)
            .ok()
            .and_then(|()| hash_item(&staging, is_dir).ok());
        if copied.as_ref() != Some(&theirs) {
            let _ = fsx::remove_any(&staging);
            refuse("the copy didn't match the origin; nothing changed".into());
            continue;
        }
        fsx::remove_any(&source_item)
            .and_then(|()| fs::rename(&staging, &source_item).map_err(Into::into))
            .map_err(|e| CliError::usage(format!("{e:#}")))?;
        if let Some(o) = st.origins.get_mut(key) {
            o.hash = theirs;
        }
        state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
        let (kind, stem) = match key.split_once('/') {
            Some((SKILLS, n)) => ("skill", n.to_string()),
            Some((_, n)) => ("agent", n.trim_end_matches(".md").to_string()),
            None => ("item", key.clone()),
        };
        commit(&git, ctx, &format!("colctl: refresh {kind} {stem}"))?;
        println!("refreshed {key} from {}", origin.path.display());
    }
    let code = run_sync(ctx, &git, None)?;
    Ok(if refused > 0 { EXIT_DRIFT } else { code })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec_29_bad_copy_leaves_the_original_untouched() {
        let t = tempfile::tempdir().unwrap();
        let item = t.path().join("h/skills/foo");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("SKILL.md"), "real").unwrap();
        let target = t.path().join("src/skills/foo");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let corrupting = |from: &Path, to: &Path| -> anyhow::Result<()> {
            fsx::copy_tree(from, to, false)?;
            fs::write(to.join("SKILL.md"), "corrupted")?;
            Ok(())
        };
        let outcome = adopt_entry(&item, &target, true, &corrupting);
        assert_eq!(
            outcome,
            Adoption::Refused("the copy didn't match the original; nothing changed".into())
        );
        assert!(item.is_dir() && !fsx::is_symlink(&item));
        assert!(!target.exists());
        assert!(fsx::find_temps(target.parent().unwrap()).is_empty());

        let failing = |_: &Path, _: &Path| -> anyhow::Result<()> { anyhow::bail!("disk full") };
        assert_eq!(
            adopt_entry(&item, &target, true, &failing),
            Adoption::Refused("disk full".into())
        );
        assert!(matches!(
            adopt_entry(&t.path().join("nope"), &target, true, &real_copy(true)),
            Adoption::Refused(_)
        ));
    }

    #[test]
    fn ac_24_adopt_entry_copies_then_links() {
        let t = tempfile::tempdir().unwrap();
        let item = t.path().join("h/agents/rev.md");
        fs::create_dir_all(item.parent().unwrap()).unwrap();
        fs::write(&item, "agent").unwrap();
        let target = t.path().join("src/agents/rev.md");
        assert_eq!(
            adopt_entry(&item, &target, false, &real_copy(false)),
            Adoption::Adopted
        );
        assert_eq!(fs::read_link(&item).unwrap(), target);
        assert_eq!(fs::read_to_string(&target).unwrap(), "agent");
    }

    #[test]
    fn names_from_sources() {
        assert_eq!(
            skill_name_from("https://github.com/o/forge-skill.git", None),
            "forge-skill"
        );
        assert_eq!(
            skill_name_from("https://x/repo", Some(".claude/skills/forge")),
            "forge"
        );
        assert_eq!(skill_name_from("./foo/", None), "foo");
        assert_eq!(skill_name_from("git@github.com:o/r.git", None), "r");
        assert!(is_url("git@github.com:o/r.git") && is_url("https://x") && !is_url("./x"));
    }

    #[test]
    fn item_words() {
        assert_eq!(parse_item(&[]).unwrap(), None);
        assert_eq!(
            parse_item(&["skills/a".into()]).unwrap().unwrap(),
            "skills/a"
        );
        assert_eq!(
            parse_item(&["plugin".into(), "p@m".into()])
                .unwrap()
                .unwrap(),
            "plugin p@m"
        );
        assert_eq!(parse_item(&["bogus".into()]).unwrap_err().code, 2);
    }
}
