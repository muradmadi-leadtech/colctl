//! The commands: `init`, `status`, `sync`, `add`, `rm`, `adopt`. Each returns the exit code, or a
//! [`CliError`] that carries one.

use crate::adapter::{self, CLAUDE_MD, Root};
use crate::apply::{self, Outcome};
use crate::config::{self, CONFIG_FILE, Config, Harness, PLUGIN_DIRECTORY};
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_DRIFT};
use crate::fsx;
use crate::git::Git;
use crate::plan::{self, HarnessPlan, Inputs, Item, ItemKind, ItemState};
use crate::source::{self, AGENTS, AGENTS_MD, SKILLS, Source};
use crate::state::{self, Lock, STATE_DIR, State};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Harnesses `init` looks for: (name, path relative to home).
pub const KNOWN_HARNESSES: &[(&str, &str)] = &[
    ("claude", ".claude"),
    (
        "kilo-claude",
        "Library/Application Support/kilo-proxy-headless/profiles/claude",
    ),
];

/// Everything read from the source at one moment.
struct Workspace {
    config: Config,
    config_text: String,
    state: State,
    source: Source,
}

fn load(ctx: &Ctx) -> Result<Workspace, CliError> {
    let (config, config_text) = config::load(&ctx.source, &ctx.home)?;
    Ok(Workspace {
        config,
        config_text,
        state: state::load(&ctx.source)?,
        source: source::scan(&ctx.source),
    })
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
    let inputs = Inputs {
        config: &ws.config,
        source: &ws.source,
        source_dir: &ctx.source,
        source_display: &display,
        generated: &generated,
    };
    Ok(ws
        .config
        .harnesses
        .iter()
        .filter(|h| filter.is_none_or(|f| f == h.name))
        .map(|h| {
            let scan = adapter::scan(&h.path, &ctx.source);
            Planned {
                harness: h.clone(),
                plan: plan::plan(h, &scan, &ws.state.harness(&h.name), &inputs),
            }
        })
        .collect())
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
        && planned.iter().all(|p| p.plan.in_sync())
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
            })).collect::<Vec<_>>(),
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
            println!("  would {}", action.describe());
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
    commit_pending(git, ctx)?;
    let mut ws = load(ctx)?;
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

fn detect(ctx: &Ctx) -> Vec<(String, String, PathBuf)> {
    KNOWN_HARNESSES
        .iter()
        .map(|(name, rel)| (name.to_string(), format!("~/{rel}"), ctx.home.join(rel)))
        .filter(|(_, _, path)| fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()))
        .collect()
}

fn ensure_layout(ctx: &Ctx) -> Result<(), CliError> {
    let io =
        |e: std::io::Error| CliError::usage(format!("setting up {}: {e}", ctx.source.display()));
    for dir in [SKILLS, AGENTS, STATE_DIR] {
        fs::create_dir_all(ctx.source.join(dir)).map_err(io)?;
    }
    let ignore = ctx.source.join(".gitignore");
    let text = fs::read_to_string(&ignore).unwrap_or_default();
    if !text
        .lines()
        .any(|l| l.trim() == ".state/" || l.trim() == ".state")
    {
        let mut text = text;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(".state/\n");
        fs::write(&ignore, text).map_err(io)?;
    }
    Ok(())
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
            for (name, written, _) in &found {
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
        let pairs: Vec<(String, String)> = found
            .iter()
            .map(|(n, w, _)| (n.clone(), w.clone()))
            .collect();
        fsx::atomic_write(&config_path, config::initial_text(&pairs).as_bytes())
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
        for (name, written, path) in &found {
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
            additions.push((unique, written.clone()));
        }
        if dry_run {
            for (name, written) in &additions {
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
            for (name, written) in &additions {
                updated = config::append_harness(&updated, name, written);
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

/// `colctl add plugin` (AC-22).
pub fn add_plugin(
    ctx: &Ctx,
    id: &str,
    marketplace: Option<&str>,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let (_, market) = config::split_plugin_id(id)
        .ok_or_else(|| CliError::usage(format!("plugin id \"{id}\" must be name@marketplace")))?;
    if ws.config.plugins.iter().any(|p| p == id) {
        return Err(CliError::usage(format!("plugin {id} is already enabled")));
    }
    let new_market = match (marketplace, ws.config.marketplaces.get(market)) {
        (Some(_), _) if market == PLUGIN_DIRECTORY => {
            return Err(CliError::usage(format!(
                "{PLUGIN_DIRECTORY} needs no --marketplace"
            )));
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
        (None, None) if market != PLUGIN_DIRECTORY => {
            return Err(CliError::usage(format!(
                "unknown marketplace \"{market}\"; pass --marketplace <github:owner/repo | URL | path>"
            )));
        }
        _ => None,
    };
    let text = config::add_plugin(&ws.config_text, id, new_market).map_err(CliError::usage)?;
    if dry_or_write(dry_run, &format!("enable plugin {id} in {CONFIG_FILE}")) {
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    write_config(ctx, &text)?;
    commit(&git, ctx, &format!("colctl: add plugin {id}"))?;
    println!("added plugin {id}");
    run_sync(ctx, &git, None)
}

/// What `rm` removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmKind {
    Skill,
    Agent,
    Plugin,
}

/// `colctl rm` (AC-23).
pub fn rm(ctx: &Ctx, kind: RmKind, name: &str, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let (label, path) = match kind {
        RmKind::Skill => ("skill", Some(ctx.source.join(SKILLS).join(name))),
        RmKind::Agent => (
            "agent",
            Some(ctx.source.join(AGENTS).join(format!("{name}.md"))),
        ),
        RmKind::Plugin => ("plugin", None),
    };
    let known = match &path {
        Some(p) => config::valid_item_name(name) && fsx::exists_any(p),
        None => ws.config.plugins.iter().any(|p| p == name),
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
            let text = config::remove_plugin(&ws.config_text, name).map_err(CliError::usage)?;
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
    Refused(String),
}

type Copier = dyn Fn(&Path, &Path) -> anyhow::Result<()>;

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

fn adopt_instructions(ctx: &Ctx, harness: &Harness) -> Adoption {
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
    let mut new_market = None;
    if market != PLUGIN_DIRECTORY && !ws.config.marketplaces.contains_key(market) {
        let source = adapter::read_plugins(&harness.path).ok().and_then(|p| {
            p.marketplaces
                .get(market)
                .and_then(adapter::marketplace_source)
        });
        match source {
            Some(src) if config::valid_marketplace_source(&src) => new_market = Some(src),
            _ => {
                return Adoption::Refused(format!(
                    "can't tell where marketplace \"{market}\" comes from; use colctl add plugin {id} --marketplace <source>"
                ));
            }
        }
    }
    let text = match config::add_plugin(
        &ws.config_text,
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

fn adopt_item(ctx: &Ctx, harness: &Harness, item: &Item) -> Adoption {
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
        ItemKind::Plugin => adopt_plugin(ctx, harness, &item.name),
        ItemKind::Marketplace => {
            Adoption::Refused("marketplaces are adopted with their plugins".into())
        }
    }
}

fn parse_item(words: &[String]) -> Result<Option<String>, CliError> {
    match words {
        [] => Ok(None),
        [one] if one == CLAUDE_MD || one.starts_with("skills/") || one.starts_with("agents/") => {
            Ok(Some(one.clone()))
        }
        [kind, id] if kind == "plugin" => Ok(Some(format!("plugin {id}"))),
        _ => Err(CliError::usage(format!(
            "unknown item \"{}\"; use skills/<name>, agents/<name>.md, CLAUDE.md or plugin <id>",
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
            for i in p
                .plan
                .items
                .iter()
                .filter(|i| i.state == ItemState::Foreign && matches(i))
            {
                println!("would adopt {} from {}", i.address(), p.harness.name);
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
                ItemState::Foreign => adopt_item(ctx, &p.harness, i),
                ItemState::ForeignLink => {
                    Adoption::Refused("remove or replace this link yourself".into())
                }
                other if wanted.is_some() => {
                    Adoption::Refused(format!("not a foreign item (state: {})", other.as_str()))
                }
                _ => continue,
            };
            match outcome {
                Adoption::Adopted | Adoption::Linked => {
                    let kind = i.kind.as_str();
                    let label = if i.kind == ItemKind::Instructions {
                        p.harness.name.as_str()
                    } else {
                        i.name.as_str()
                    };
                    commit(&git, ctx, &format!("colctl: adopt {kind} {label}"))?;
                    let verb = if outcome == Adoption::Adopted {
                        "adopted"
                    } else {
                        "linked (already identical)"
                    };
                    println!("{verb} {} from {}", i.address(), p.harness.name);
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
