//! Quarantine (cycle 4): `update` and `refresh` hold new versions as candidates; `review`, `approve`
//! and `reject` decide them. Nothing held reaches a harness until it's approved.

use super::{
    Copier, EXIT_DRIFT, Workspace, commit, commit_pending, hash_item, load, plugins_dir, real_copy,
    write_config,
};
use crate::config;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::fsx;
use crate::git::Git;
use crate::review::{self, Direction, Review, escape, escape_body};
use crate::state::{self, Candidate, HeldCommit, HeldCopy, Lock, State};
use crate::ui::style::Token;
use crate::vault::{self, CloneState, Vault};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

/// The candidate key of a source plugin.
pub(super) fn source_key(name: &str) -> String {
    format!("sources/{name}")
}

/// "n new commits" or "goes back n commits" from `from` to `to`.
fn distance(vault: &Vault, name: &str, from: &str, to: &str) -> String {
    let ahead = vault.count_between(name, from, to);
    if ahead == 0 {
        let back = vault.count_between(name, to, from);
        let word = if back == 1 { "commit" } else { "commits" };
        return format!("goes back {back} {word}");
    }
    let word = if ahead == 1 { "commit" } else { "commits" };
    format!("{ahead} new {word}")
}

/// What `update` found for one source.
enum Found {
    UpToDate,
    Rejected,
    AlreadyHeld,
    New { replaces: Option<String> },
}

fn classify(ws: &Workspace, s: &config::Source, new: &str) -> Found {
    let key = source_key(&s.name);
    if new == s.rev {
        return Found::UpToDate;
    }
    if ws.state.is_rejected(&key, new) {
        return Found::Rejected;
    }
    match ws.state.quarantine.get(&key) {
        Some(Candidate::Source(held)) if held.to == new => Found::AlreadyHeld,
        Some(Candidate::Source(held)) => Found::New {
            replaces: Some(held.to.clone()),
        },
        _ => Found::New { replaces: None },
    }
}

/// `colctl update` (AC4-11…16): fetches each source and holds a newer commit for review. It never
/// moves a pin, commits, or calls `claude`/`codex`; `approve` does that.
pub fn update(
    ctx: &Ctx,
    names: &[String],
    check: bool,
    rev: Option<&str>,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    for n in names {
        if !ws.config.sources.iter().any(|s| &s.name == n) {
            return Err(CliError::usage(format!("no source named \"{n}\"")));
        }
    }
    if rev.is_some() && names.len() != 1 {
        return Err(CliError::usage("--rev needs exactly one source name"));
    }
    if ws.config.sources.is_empty() {
        ctx.out.line(crate::ui::render::verdict(
            &ctx.out.caps,
            Token::Info,
            "No sources declared",
            &[],
        ));
        return Ok(0);
    }
    let _lock = if check || dry_run {
        None
    } else {
        Some(Lock::acquire(&ctx.source)?)
    };
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    let mut st = state::load(&ctx.source)?;
    let mut failures = 0;
    let mut newer = 0;
    let mut said: Vec<(Token, String)> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    for s in ws
        .config
        .sources
        .iter()
        .filter(|s| names.is_empty() || names.contains(&s.name))
    {
        let looked = vault
            .fetch_clone(&s.name, &s.clone_url())
            .and_then(|_| match rev {
                Some(r) => vault.resolve(&s.name, r),
                None => vault.upstream_head(&s.name),
            });
        let new = match looked {
            Ok(new) => new,
            Err(e) if rev.is_some() => return Err(CliError::usage(e)),
            Err(e) => {
                failures += 1;
                said.push((Token::Failed, format!("{}: {e}", s.name)));
                continue;
            }
        };
        let found = classify(&ws, s, &new);
        if matches!(found, Found::UpToDate) {
            said.push((Token::Ok, format!("{}: up to date", s.name)));
            continue;
        }
        let change = format!(
            "{}: {} → {} ({})",
            s.name,
            vault::short(&s.rev),
            vault::short(&new),
            distance(&vault, &s.name, &s.rev, &new)
        );
        if check {
            let note = match found {
                Found::Rejected => " (rejected)",
                Found::AlreadyHeld => " (held for review)",
                _ => "",
            };
            let tone = if matches!(found, Found::Rejected) {
                Token::Info
            } else {
                newer += 1;
                if matches!(found, Found::AlreadyHeld) {
                    held.push(s.name.clone());
                }
                Token::Attention
            };
            said.push((tone, format!("{change}{note}")));
            continue;
        }
        match found {
            Found::Rejected => {
                said.push((
                    Token::Info,
                    format!(
                        "{}: {} was rejected; nothing held",
                        s.name,
                        vault::short(&new)
                    ),
                ));
            }
            Found::AlreadyHeld => {
                held.push(s.name.clone());
                said.push((
                    Token::Attention,
                    format!("{}: {} already held for review", s.name, vault::short(&new)),
                ));
            }
            Found::New { replaces } => {
                let replacing = replaces
                    .map(|old| format!(" (replaces {})", vault::short(&old)))
                    .unwrap_or_default();
                if dry_run {
                    said.push((Token::Info, format!("would hold {change}{replacing}")));
                    continue;
                }
                if let Err(e) = vault.hold(&s.name, &new) {
                    failures += 1;
                    said.push((Token::Failed, format!("{}: {e}", s.name)));
                    continue;
                }
                st.quarantine.insert(
                    source_key(&s.name),
                    Candidate::Source(HeldCommit {
                        from: s.rev.clone(),
                        to: new.clone(),
                        held_at: state::now(),
                    }),
                );
                state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
                held.push(s.name.clone());
                said.push((
                    Token::Attention,
                    format!(
                        "{change}, held for review{replacing}; colctl review {}",
                        s.name
                    ),
                ));
            }
            Found::UpToDate => {}
        }
    }
    // AC6-21: a verdict, a line per source, and what to run next.
    let plural = super::plural;
    let (tone, verdict) = if failures > 0 {
        (
            Token::Failed,
            format!("{} couldn't be checked", plural(failures, "source")),
        )
    } else if check && newer > 0 {
        (
            Token::Attention,
            format!("{} available", plural(newer, "update")),
        )
    } else if dry_run && said.iter().any(|(t, _)| *t == Token::Info) && !said.is_empty() {
        (Token::Info, "Would hold what's new for review".to_string())
    } else if !held.is_empty() {
        (
            Token::Attention,
            format!("{} held for review", plural(held.len(), "update")),
        )
    } else {
        (Token::Ok, "Everything up to date".to_string())
    };
    let mut lines = super::item_lines(ctx, tone, &verdict, &[], &said);
    if !ctx.out.caps.quiet {
        let next = match held.as_slice() {
            [one] => Some(format!("colctl review {one}")),
            [] if check && newer > 0 => Some("colctl update holds them for review".into()),
            [] => None,
            _ => Some("colctl review".into()),
        };
        if let Some(n) = next {
            lines.push(String::new());
            lines.push(crate::ui::render::hint(&ctx.out.caps, &n));
        }
    } else {
        lines.truncate(1);
    }
    ctx.out.lines(lines);
    Ok(if failures > 0 || (check && newer > 0) {
        EXIT_DRIFT
    } else {
        0
    })
}

/// Copies `origin` into `snapshot` through a staging path and checks the copy hashes to `expected`.
/// On any failure nothing is left behind (EC4-18).
pub(super) fn snapshot(
    origin: &Path,
    snapshot: &Path,
    is_dir: bool,
    expected: &str,
    copy: &Copier,
) -> Result<(), String> {
    let parent = snapshot
        .parent()
        .ok_or_else(|| format!("{} has no parent", snapshot.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
    let name = snapshot
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staging = parent.join(format!("{}hold-{name}", fsx::TMP_PREFIX));
    let _ = fsx::remove_any(&staging);
    let copied = copy(origin, &staging)
        .ok()
        .and_then(|()| hash_item(&staging, is_dir).ok());
    if copied.as_deref() != Some(expected) {
        let _ = fsx::remove_any(&staging);
        return Err("the copy didn't match the origin; nothing held".into());
    }
    let _ = fsx::remove_any(snapshot);
    fs::rename(&staging, snapshot).map_err(|e| {
        let _ = fsx::remove_any(&staging);
        format!("moving the snapshot into place: {e}")
    })
}

/// What `refresh` does with one taken-over item.
fn refresh_one(ctx: &Ctx, st: &mut State, key: &str, copy: &Copier) -> Result<String, String> {
    let Some(origin) = st.origins.get(key).cloned() else {
        return Err(format!("{key} has no recorded origin"));
    };
    let is_dir = key.starts_with("skills/");
    let source_item = ctx.source.join(key);
    if !fsx::exists_any(&origin.path) {
        return Err(format!("origin is gone ({})", origin.path.display()));
    }
    let (Ok(theirs), Ok(ours)) = (
        hash_item(&origin.path, is_dir),
        hash_item(&source_item, is_dir),
    ) else {
        return Err("couldn't read the origin or the source copy".into());
    };
    if theirs == origin.hash {
        return Ok(format!(
            "{key} is up to date with {}",
            origin.path.display()
        ));
    }
    if ours != origin.hash {
        return Err(format!(
            "changed in both places; compare {} and {}, keep one, then adopt again",
            origin.path.display(),
            source_item.display()
        ));
    }
    if st.is_rejected(key, &theirs) {
        return Ok(format!("{key}: this version was rejected; nothing held"));
    }
    let replacing = match st.quarantine.get(key) {
        Some(Candidate::Skill(c) | Candidate::Agent(c)) if c.hash == theirs => {
            return Ok(format!("{key} already held for review"));
        }
        Some(_) => " (replaces the copy held before)",
        None => "",
    };
    snapshot(
        &origin.path,
        &state::snapshot_path(&ctx.source, key),
        is_dir,
        &theirs,
        copy,
    )?;
    let held = HeldCopy {
        origin: origin.path.clone(),
        base: ours,
        hash: theirs,
        held_at: state::now(),
    };
    st.quarantine.insert(
        key.to_string(),
        if is_dir {
            Candidate::Skill(held)
        } else {
            Candidate::Agent(held)
        },
    );
    Ok(format!(
        "held {key} for review{replacing}; colctl review {key}"
    ))
}

/// `colctl refresh` (AC4-17, AC4-18): snapshots taken-over items whose origin changed and holds them
/// for review. The source copy, its recorded hash and the history stay as they are.
pub fn refresh(ctx: &Ctx, items: &[String], dry_run: bool) -> Result<i32, CliError> {
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
    if dry_run {
        let said: Vec<(Token, String)> = ws
            .origin_changed
            .iter()
            .filter(|(k, _)| keys.contains(k))
            .map(|(key, origin)| {
                (
                    Token::Info,
                    format!("would hold {key} from {}", ctx.tilde(origin)),
                )
            })
            .collect();
        let verdict = if said.is_empty() {
            "Nothing changed in the origins".to_string()
        } else {
            format!(
                "Would hold {} for review",
                super::plural(said.len(), "item")
            )
        };
        ctx.out
            .lines(super::item_lines(ctx, Token::Info, &verdict, &[], &said));
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    if keys.is_empty() {
        ctx.out.line(crate::ui::render::verdict(
            &ctx.out.caps,
            Token::Info,
            "Nothing to refresh: no adopted item has a recorded origin",
            &[],
        ));
        return Ok(0);
    }
    let mut st = state::load(&ctx.source)?;
    let mut refused = 0;
    let mut said: Vec<(Token, String)> = Vec::new();
    let mut held: Vec<String> = Vec::new();
    for key in &keys {
        let is_dir = key.starts_with("skills/");
        match refresh_one(ctx, &mut st, key, &real_copy(is_dir)) {
            Ok(line) if line.starts_with("held ") => {
                held.push(key.clone());
                said.push((Token::Attention, line));
            }
            Ok(line) if line.contains("already held") => {
                held.push(key.clone());
                said.push((Token::Attention, line));
            }
            Ok(line) if line.contains("rejected") => said.push((Token::Info, line)),
            Ok(line) => said.push((Token::Ok, line)),
            Err(why) => {
                refused += 1;
                said.push((Token::Failed, format!("refused {key}: {why}")));
            }
        }
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    // AC6-21: a verdict, a line per item, and what to run next.
    let (tone, verdict) = match (held.len(), refused) {
        (_, r) if r > 0 => (
            Token::Failed,
            format!("Refused {}", super::plural(r, "item")),
        ),
        (0, _) => (
            Token::Ok,
            "Everything up to date with its origin".to_string(),
        ),
        (h, _) => (
            Token::Attention,
            format!("{} held for review", super::plural(h, "item")),
        ),
    };
    let mut lines = super::item_lines(ctx, tone, &verdict, &[], &said);
    let next = match held.as_slice() {
        [one] => Some(format!("colctl review {one}")),
        [] => None,
        _ => Some("colctl review".into()),
    };
    if ctx.out.caps.quiet {
        lines.truncate(1);
    } else if let Some(n) = next {
        lines.push(String::new());
        lines.push(crate::ui::render::hint(&ctx.out.caps, &n));
    }
    ctx.out.lines(lines);
    Ok(if refused > 0 { EXIT_DRIFT } else { 0 })
}

/// A held candidate as `review`, `status` and `approve` see it.
pub(super) struct View {
    pub key: String,
    pub name: String,
    pub candidate: Candidate,
    pub repo: Option<String>,
    pub review: Option<Review>,
    /// Why the candidate can't be used (EC4-12, EC4-13).
    pub error: Option<String>,
    /// Something to know before approving (EC4-14).
    pub warning: Option<String>,
    /// A held conversion's package, rebuilt from the clone (cycle 7).
    pub package: Option<crate::repackage::Package>,
}

/// The short name shown for a key: the source name, or the key itself for skills and agents.
fn name_of(key: &str) -> String {
    key.strip_prefix("sources/")
        .or_else(|| key.strip_prefix("conversions/"))
        .unwrap_or(key)
        .to_string()
}

/// Turns a name from the command line into a candidate key, checking it names something colctl
/// knows (EC4-03).
pub(super) fn resolve(ws: &Workspace, name: &str) -> Result<String, CliError> {
    if name.contains('/') {
        if ws.state.origins.contains_key(name) || ws.state.quarantine.contains_key(name) {
            return Ok(name.to_string());
        }
        return Err(CliError::usage(format!(
            "no taken-over item \"{name}\"; use a source name, skills/<name> or agents/<name>.md"
        )));
    }
    if ws.config.sources.iter().any(|s| s.name == name) {
        // A source's held conversion answers to the source's name when no update is held.
        let conversion = super::convert::conversion_key(name);
        if !ws.state.quarantine.contains_key(&source_key(name))
            && ws.state.quarantine.contains_key(&conversion)
        {
            return Ok(conversion);
        }
        return Ok(source_key(name));
    }
    Err(CliError::usage(format!("no source named \"{name}\"")))
}

/// A held conversion: what it changes against the installed package (none yet, the first time),
/// and its contents rebuilt from the clone, which must give the held digest.
fn inspect_conversion(
    ctx: &Ctx,
    ws: &Workspace,
    git: &Git,
    key: &str,
    c: &state::HeldConversion,
    with_diff: bool,
    view: &mut View,
) -> Result<Review, String> {
    let name = view.name.clone();
    let held = state::snapshot_path(&ctx.source, key);
    if !held.is_dir() {
        return Err(format!(
            "the held package for {name} is missing; run colctl convert {name}"
        ));
    }
    if let Some(s) = ws.config.sources.iter().find(|s| s.name == name) {
        view.repo = Some(s.repo.clone());
        match super::convert::package_for(ctx, s) {
            Ok(pkg) => {
                if pkg.digest() != c.digest || s.rev != c.rev {
                    view.warning = Some(format!(
                        "{name} changed since its package was held; run colctl convert {name} again"
                    ));
                }
                view.package = Some(pkg);
            }
            Err(e) => view.warning = Some(e.message),
        }
    }
    let installed = plugins_dir(ctx).join(super::CONVERTED).join(&name);
    let empty = tempfile::tempdir().map_err(|e| e.to_string())?;
    let installed = if installed.is_dir() {
        installed
    } else {
        empty.path().to_path_buf()
    };
    review::copies(git, &ctx.source, &installed, &held, true, with_diff)
}

/// Reads one candidate: checks it's intact, and gathers its review (with the diff when asked).
pub(super) fn inspect(
    ctx: &Ctx,
    ws: &Workspace,
    git: &Git,
    key: &str,
    candidate: &Candidate,
    with_diff: bool,
) -> View {
    let name = name_of(key);
    let mut view = View {
        key: key.to_string(),
        name: name.clone(),
        candidate: candidate.clone(),
        repo: None,
        review: None,
        error: None,
        warning: None,
        package: None,
    };
    let result = match candidate {
        Candidate::Source(c) => {
            let declared = ws.config.sources.iter().find(|s| s.name == name);
            view.repo = declared.map(|s| s.repo.clone());
            if let Some(s) = declared.filter(|s| s.rev != c.from) {
                view.warning = Some(format!(
                    "{name}'s pin changed since it was held (now {}); run colctl update {name} again",
                    vault::short(&s.rev)
                ));
            }
            let vault = Vault {
                git,
                dir: plugins_dir(ctx),
            };
            if vault.held(&name).as_deref() != Some(c.to.as_str()) {
                Err(format!(
                    "the held commit for {name} is missing; run colctl update {name}"
                ))
            } else {
                review::commits(git, &vault.path(&name), &c.from, &c.to, with_diff)
            }
        }
        Candidate::Skill(c) | Candidate::Agent(c) => {
            let is_dir = matches!(candidate, Candidate::Skill(_));
            let held = state::snapshot_path(&ctx.source, key);
            if hash_item(&held, is_dir).ok().as_deref() != Some(c.hash.as_str()) {
                Err(format!(
                    "the held copy of {key} changed since it was held; reject it and refresh again"
                ))
            } else {
                review::copies(
                    git,
                    &ctx.source,
                    &ctx.source.join(key),
                    &held,
                    is_dir,
                    with_diff,
                )
            }
        }
        Candidate::Conversion(c) => inspect_conversion(ctx, ws, git, key, c, with_diff, &mut view),
    };
    match result {
        Ok(r) => view.review = Some(r),
        Err(e) => view.error = Some(e),
    }
    view
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "from → to" for a candidate: short commits, or the origin for a held copy.
fn change_text(ctx: &Ctx, c: &Candidate) -> String {
    match c {
        Candidate::Source(c) => format!("{} → {}", vault::short(&c.from), vault::short(&c.to)),
        Candidate::Skill(c) | Candidate::Agent(c) => ctx.tilde(&c.origin),
        Candidate::Conversion(c) => format!("Codex package from {}", vault::short(&c.rev)),
    }
}

/// How big a candidate is: commits (or "goes back n commits") for a source, files for a copy.
fn size_text(v: &View, r: &Review) -> String {
    match (&v.candidate, r.direction) {
        (Candidate::Source(_), Direction::Back) => {
            format!("goes back {}", plural(r.commits.len(), "commit", "commits"))
        }
        (Candidate::Source(_), Direction::Forward) => plural(r.commits.len(), "commit", "commits"),
        _ => plural(r.files.len(), "file", "files"),
    }
}

/// The list view (AC6-22): one row per held version, aligned.
fn list_lines(ctx: &Ctx, views: &[View]) -> Vec<String> {
    use crate::ui::render::{self, Cell};
    let caps = &ctx.out.caps;
    let verdict = render::verdict(
        caps,
        Token::Attention,
        &format!(
            "{} held for review",
            plural(views.len(), "version", "versions")
        ),
        &[],
    );
    let rows: Vec<Vec<Cell>> = views
        .iter()
        .map(|v| {
            let what = escape(&change_text(ctx, &v.candidate));
            match &v.review {
                Some(r) => {
                    let flagged = flagged_files(r);
                    vec![
                        Cell::styled(v.key.clone(), Token::Name),
                        Cell::plain(what),
                        Cell::plain(size_text(v, r)),
                        if flagged == 0 {
                            Cell::styled("no flags", Token::Dim)
                        } else {
                            Cell::styled(
                                plural(flagged, "file flagged", "files flagged"),
                                Token::Attention,
                            )
                        },
                    ]
                }
                None => vec![
                    Cell::styled(v.key.clone(), Token::Name),
                    Cell::plain(what),
                    Cell::styled(
                        format!(
                            "✗ error: {}",
                            escape(v.error.as_deref().unwrap_or_default())
                        ),
                        Token::Failed,
                    ),
                ],
            }
        })
        .collect();
    let mut lines = vec![verdict];
    if caps.quiet {
        return lines;
    }
    lines.push(String::new());
    lines.extend(render::table(caps, 2, &rows, 1));
    lines.push(String::new());
    let next = match views {
        [one] => format!("colctl review {}", escape(&one.name)),
        _ => "colctl review <name> shows one in full".into(),
    };
    lines.push(render::hint(caps, &next));
    lines
}

/// Files with at least one flag.
fn flagged_files(r: &Review) -> usize {
    r.flags
        .iter()
        .map(|f| f.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// Risk flags grouped per file (AC6-22): kinds joined with " · ", files ordered by their most
/// serious kind, then path.
fn flag_rows(r: &Review) -> Vec<(String, String)> {
    let mut by_file: Vec<(String, Vec<&crate::risk::Flag>)> = Vec::new();
    for f in &r.flags {
        match by_file.iter_mut().find(|(p, _)| *p == f.path) {
            Some((_, list)) => list.push(f),
            None => by_file.push((f.path.clone(), vec![f])),
        }
    }
    for (_, list) in &mut by_file {
        list.sort_by_key(|f| f.kind);
    }
    by_file.sort_by(|(pa, a), (pb, b)| (a[0].kind, pa).cmp(&(b[0].kind, pb)));
    by_file
        .into_iter()
        .map(|(path, list)| {
            let kinds = list
                .iter()
                .map(|f| f.kind.as_str())
                .collect::<Vec<_>>()
                .join(" · ");
            let mut reasons: Vec<&str> = Vec::new();
            for f in &list {
                if !reasons.contains(&f.reason.as_str()) {
                    reasons.push(&f.reason);
                }
            }
            (
                escape(&path),
                format!("{kinds}: {}", escape(&reasons.join("; "))),
            )
        })
        .collect()
}

/// The detail view (AC6-22): verdict, risk flags first, then commits, files, diff and what to do.
fn detail_lines(ctx: &Ctx, v: &View) -> Vec<String> {
    use crate::ui::render::{self, Cell};
    let caps = &ctx.out.caps;
    let c = caps.color_out;
    let mut lines: Vec<String> = Vec::new();
    let head = match &v.candidate {
        Candidate::Source(h) => format!(
            "{}  {} → {}",
            escape(&v.name),
            vault::short(&h.from),
            vault::short(&h.to)
        ),
        Candidate::Skill(h) | Candidate::Agent(h) => {
            format!("{}  from {}", escape(&v.key), escape(&ctx.tilde(&h.origin)))
        }
        Candidate::Conversion(h) => format!(
            "{}  Codex package from {}",
            escape(&v.name),
            vault::short(&h.rev)
        ),
    };
    let Some(r) = &v.review else {
        lines.push(render::verdict(
            caps,
            Token::Failed,
            &head,
            &["can't be reviewed".into()],
        ));
        if let Some(e) = &v.error {
            lines.push(format!("  {}", escape(e)));
        }
        if let Some(w) = &v.warning {
            lines.push(render::warning(caps, &escape(w)));
        }
        return lines;
    };
    let flagged = flagged_files(r);
    let mut facts = vec![size_text(v, r)];
    facts.push(if flagged == 0 {
        "no flags".to_string()
    } else {
        plural(flagged, "file flagged", "files flagged")
    });
    lines.push(render::verdict(caps, Token::Attention, &head, &facts));
    if let Some(repo) = &v.repo {
        lines.push(format!("  {}", render::footer(caps, &escape(repo))));
    }
    if let Some(w) = &v.warning {
        lines.push(format!("  {}", render::warning(caps, &escape(w))));
    }
    let section = |lines: &mut Vec<String>, title: &str| {
        lines.push(String::new());
        lines.push(render::heading(caps, title));
    };
    if let Some(pkg) = &v.package {
        section(&mut lines, "Skills");
        lines.extend(
            render::wrap(&escape(&pkg.skills.join(", ")), caps.width, 2, 2)
                .into_iter()
                .enumerate()
                .map(|(i, l)| if i == 0 { format!("  {l}") } else { l }),
        );
        section(&mut lines, &format!("Hooks ({})", pkg.hooks.len()));
        if pkg.hooks.is_empty() {
            lines.push(format!("  {} none", crate::ui::style::mark(Token::Info, c)));
        }
        let cells: Vec<Vec<Cell>> = pkg
            .hooks
            .iter()
            .map(|h| {
                vec![
                    Cell::styled(escape(&h.event), Token::Name),
                    Cell::plain(escape(h.matcher.as_deref().unwrap_or(""))),
                    Cell::styled(escape(&h.command), Token::Dim),
                ]
            })
            .collect();
        lines.extend(render::table(caps, 2, &cells, 2));
        for (title, items) in [
            ("Codex never runs", pkg.report.never_runs.clone()),
            (
                "Not carried over",
                pkg.report
                    .not_carried
                    .iter()
                    .map(|n| format!("{} {}", n.kind, n.name))
                    .collect(),
            ),
        ] {
            if items.is_empty() {
                continue;
            }
            section(&mut lines, title);
            for i in items {
                lines.push(format!("  {}", escape(&i)));
            }
        }
    }
    section(&mut lines, "Risk flags");
    let rows = flag_rows(r);
    if rows.is_empty() {
        lines.push(format!("  {} none", crate::ui::style::mark(Token::Info, c)));
    } else {
        // Flags are never cut: a reason that doesn't fit goes on the line below its file.
        let path_w = rows
            .iter()
            .map(|(p, _)| render::width(p))
            .max()
            .unwrap_or(0)
            .min(caps.width * 2 / 5);
        for (path, what) in rows {
            let shown = render::cut_middle(&path, path_w);
            let pad = " ".repeat(path_w - render::width(&shown));
            let what_p = crate::ui::style::paint(&what, Token::Attention, c);
            let path_p = crate::ui::style::paint(&shown, Token::Name, c);
            if 2 + path_w + 2 + render::width(&what) <= caps.width {
                lines.push(format!("  {path_p}{pad}  {what_p}"));
            } else {
                lines.push(format!("  {path_p}"));
                lines.push(format!("      {what_p}"));
            }
        }
    }
    if let Candidate::Source(_) = v.candidate {
        let title = match r.direction {
            Direction::Forward => format!("Commits ({})", r.commits.len()),
            Direction::Back => format!(
                "Goes back {} (removed)",
                plural(r.commits.len(), "commit", "commits")
            ),
        };
        section(&mut lines, &title);
        let cells: Vec<Vec<Cell>> = r
            .commits
            .iter()
            .map(|cm| {
                vec![
                    Cell::styled(vault::short(&cm.rev), Token::Dim),
                    Cell::plain(escape(&cm.subject)),
                ]
            })
            .collect();
        lines.extend(render::table(caps, 2, &cells, 1));
    }
    section(&mut lines, &format!("Files ({})", r.files.len()));
    for f in &r.files {
        let extra = match (&f.symlink, f.executable) {
            (Some(t), _) => format!("  -> {}", escape(t)),
            (None, true) => "  (+x)".into(),
            _ => String::new(),
        };
        let renamed = f
            .old_path
            .as_ref()
            .map(|o| format!("{} → ", escape(o)))
            .unwrap_or_default();
        lines.push(format!(
            "  {}  {renamed}{}{}",
            f.status.letter(),
            escape(&f.path),
            render::footer(caps, &extra)
        ));
    }
    if let Some(diff) = &r.diff {
        section(&mut lines, "Diff");
        for l in escape_body(diff).lines() {
            let token = if l.starts_with('+') && !l.starts_with("+++") {
                Some(Token::Added)
            } else if l.starts_with('-') && !l.starts_with("---") {
                Some(Token::Removed)
            } else {
                None
            };
            lines.push(match token {
                Some(t) => crate::ui::style::paint(l, t, c),
                None => l.to_string(),
            });
        }
    }
    lines.push(String::new());
    let name = escape(&v.name);
    lines.push(format!(
        "{}   or   {}",
        render::hint(caps, &format!("colctl approve {name}")),
        crate::ui::style::paint(&format!("colctl reject {name}"), Token::Command, c)
    ));
    lines
}

/// The JSON object for one candidate (interfaces.md § JSON).
pub(super) fn view_json(v: &View, full: bool) -> Value {
    let (from, to, origin, held_at) = match &v.candidate {
        Candidate::Source(c) => (c.from.clone(), c.to.clone(), None, c.held_at),
        Candidate::Skill(c) | Candidate::Agent(c) => (
            c.base.clone(),
            c.hash.clone(),
            Some(c.origin.display().to_string()),
            c.held_at,
        ),
        Candidate::Conversion(c) => (c.rev.clone(), c.digest.clone(), None, c.held_at),
    };
    let mut out = json!({
        "key": v.key,
        "kind": v.candidate.kind(),
        "name": match &v.candidate {
            Candidate::Source(_) | Candidate::Conversion(_) => v.name.clone(),
            _ => v.key.rsplit('/').next().unwrap_or_default().trim_end_matches(".md").to_string(),
        },
        "from": from,
        "to": to,
        "held_at": held_at,
        "error": v.error,
        "warning": v.warning,
    });
    if let Candidate::Source(_) = v.candidate {
        out["repo"] = json!(v.repo);
        out["direction"] = json!(v.review.as_ref().map(|r| r.direction));
    } else {
        out["origin"] = json!(origin);
    }
    let r = v.review.as_ref();
    if full {
        out["commits"] = json!(r.map(|r| r.commits.clone()).unwrap_or_default());
        out["files"] = json!(r.map(|r| r.files.clone()).unwrap_or_default());
        out["flags"] = json!(r.map(|r| r.flags.clone()).unwrap_or_default());
    } else {
        out["flags"] = json!(r.map_or(0, |r| r.flags.len()));
    }
    if let Some(diff) = r.and_then(|r| r.diff.clone()) {
        out["diff"] = json!(diff);
    }
    if let (Some(pkg), Value::Object(o)) = (&v.package, &mut out)
        && let Value::Object(more) = super::convert::package_json(pkg)
    {
        o.extend(more);
    }
    out
}

/// Every held candidate, for `status` (no diff). Without git, candidates are listed unchecked.
pub(super) fn held_views(ctx: &Ctx, ws: &Workspace, git: Option<&Git>) -> Vec<View> {
    ws.state
        .quarantine
        .iter()
        .map(|(key, c)| match git {
            Some(git) => inspect(ctx, ws, git, key, c, false),
            None => View {
                key: key.clone(),
                name: name_of(key),
                candidate: c.clone(),
                repo: None,
                review: None,
                error: None,
                warning: None,
                package: None,
            },
        })
        .collect()
}

/// The `status --json` entry for a held candidate (AC4-29).
pub(super) fn status_entry(v: &View) -> Value {
    let mut out = view_json(v, false);
    out["state"] = json!("pending-review");
    out
}

/// `colctl review [<name>] [--json]` (AC4-19…22): offline, read-only.
pub fn review(ctx: &Ctx, name: Option<&str>, pick: bool, as_json: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    // AC6-19: `review -i` picks one held version in a terminal.
    let picked;
    let name = match name {
        None if pick && !as_json && ctx.out.caps.interactive => {
            let keys: Vec<String> = ws.state.quarantine.keys().cloned().collect();
            let labels: Vec<String> = keys
                .iter()
                .map(|k| format!("{k}  {}", change_text(ctx, &ws.state.quarantine[k])))
                .collect();
            match ctx.out.pick_one("Which held version?", &labels) {
                Some(i) => {
                    picked = name_of(&keys[i]);
                    Some(picked.as_str())
                }
                None => None,
            }
        }
        other => other,
    };
    let Some(name) = name else {
        let views: Vec<View> = ws
            .state
            .quarantine
            .iter()
            .map(|(k, c)| inspect(ctx, &ws, &git, k, c, false))
            .collect();
        if as_json {
            let list: Vec<Value> = views.iter().map(|v| view_json(v, true)).collect();
            ctx.out.line(
                serde_json::to_string_pretty(&json!({"version": 1, "candidates": list}))
                    .unwrap_or_default(),
            );
        } else if views.is_empty() {
            ctx.out.line(crate::ui::render::verdict(
                &ctx.out.caps,
                Token::Ok,
                "Nothing held for review",
                &[],
            ));
        } else {
            ctx.out.lines(list_lines(ctx, &views));
        }
        return Ok(if views.is_empty() { 0 } else { EXIT_DRIFT });
    };
    let key = resolve(&ws, name)?;
    let Some(candidate) = ws.state.quarantine.get(&key) else {
        return Err(CliError::usage(format!("nothing held for {name}")));
    };
    let view = inspect(ctx, &ws, &git, &key, candidate, true);
    if as_json {
        ctx.out.line(
            serde_json::to_string_pretty(
                &json!({"version": 1, "candidates": [view_json(&view, true)]}),
            )
            .unwrap_or_default(),
        );
    } else {
        ctx.out.page(detail_lines(ctx, &view));
    }
    Ok(if view.error.is_some() { EXIT_DRIFT } else { 0 })
}

/// What a picker gave `approve`/`reject` when no names were given (AC6-19, EC6-13).
enum Picked {
    /// Names were given (or `--all`), or there's no terminal: carry on as before.
    Given,
    /// The names picked.
    Names(Vec<String>),
    /// Nothing to do: exit with this code.
    Done(i32),
}

fn picked_names(ctx: &Ctx, ws: &Workspace, names: &[String], all: bool, verb: &str) -> Picked {
    if !names.is_empty() || all || !ctx.out.caps.interactive {
        return Picked::Given;
    }
    let keys: Vec<String> = ws.state.quarantine.keys().cloned().collect();
    if keys.is_empty() {
        ctx.out.line(crate::ui::render::verdict(
            &ctx.out.caps,
            Token::Info,
            "Nothing held for review",
            &[],
        ));
        return Picked::Done(0);
    }
    let labels: Vec<String> = keys
        .iter()
        .map(|k| format!("{k}  {}", change_text(ctx, &ws.state.quarantine[k])))
        .collect();
    match ctx.out.pick_many(
        &format!("Which held versions do you want to {verb}?"),
        &labels,
    ) {
        Some(chosen) if !chosen.is_empty() => {
            Picked::Names(chosen.into_iter().map(|i| name_of(&keys[i])).collect())
        }
        _ => {
            ctx.out.line(crate::ui::render::verdict(
                &ctx.out.caps,
                Token::Info,
                "Nothing changed",
                &[],
            ));
            Picked::Done(EXIT_DRIFT)
        }
    }
}

/// The candidate keys named on the command line, or every held one with `--all` (EC4-02, EC4-03).
fn chosen(
    ws: &Workspace,
    names: &[String],
    all: bool,
    verb: &str,
) -> Result<Vec<String>, CliError> {
    match (names.is_empty(), all) {
        (true, false) => {
            return Err(CliError::usage(format!(
                "name what to {verb} (a source name, skills/<name> or agents/<name>.md), or use --all"
            )));
        }
        (false, true) => {
            return Err(CliError::usage(format!(
                "{verb} takes names or --all, not both"
            )));
        }
        (true, true) => return Ok(ws.state.quarantine.keys().cloned().collect()),
        (false, false) => {}
    }
    let mut keys = Vec::new();
    for name in names {
        let key = resolve(ws, name)?;
        if !ws.state.quarantine.contains_key(&key) {
            return Err(CliError::usage(format!("nothing held for {name}")));
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// Approves one candidate: checks it, then moves the pin or replaces the source copy, and commits.
fn approve_one(
    ctx: &Ctx,
    ws: &Workspace,
    git: &Git,
    st: &mut State,
    config_text: &mut String,
    key: &str,
) -> Result<String, String> {
    let Some(candidate) = st.quarantine.get(key).cloned() else {
        return Err("nothing held".into());
    };
    let view = inspect(ctx, ws, git, key, &candidate, false);
    if let Some(e) = view.error {
        return Err(e);
    }
    let commit_err = |e: CliError| e.message;
    match &candidate {
        Candidate::Conversion(_) => {
            let name = &view.name;
            if view.warning.is_some() {
                return Err(format!(
                    "{name} changed since its package was held; run colctl convert {name} again"
                ));
            }
            let text = config::set_source_convert(config_text, name, true)?;
            write_config(ctx, &text).map_err(commit_err)?;
            *config_text = text;
            commit(git, ctx, &format!("colctl: convert {name} for Codex")).map_err(commit_err)?;
            st.quarantine.remove(key);
            let _ = fsx::remove_any(&state::snapshot_path(&ctx.source, key));
            Ok(format!("approved {name}'s Codex package"))
        }
        Candidate::Source(c) => {
            let name = &view.name;
            if view.warning.is_some() {
                return Err(format!(
                    "{name}'s pin changed since it was held; run colctl update {name} again"
                ));
            }
            let declared = ws
                .config
                .sources
                .iter()
                .find(|s| &s.name == name)
                .ok_or_else(|| format!("no source named {name}"))?;
            let vault = Vault {
                git,
                dir: plugins_dir(ctx),
            };
            match vault.inspect(name, &declared.clone_url(), &declared.rev) {
                CloneState::Dirty => {
                    return Err(format!(
                        "the clone at {} has local changes",
                        ctx.tilde(&vault.path(name))
                    ));
                }
                CloneState::Invalid(reason) => return Err(reason),
                _ => {}
            }
            let text = config::set_source_rev(config_text, name, &c.to)?;
            write_config(ctx, &text).map_err(commit_err)?;
            *config_text = text;
            commit(
                git,
                ctx,
                &format!("colctl: approve source {name} at {}", vault::short(&c.to)),
            )
            .map_err(commit_err)?;
            st.quarantine.remove(key);
            Ok(format!(
                "approved {name}: {} → {}",
                vault::short(&c.from),
                vault::short(&c.to)
            ))
        }
        Candidate::Skill(c) | Candidate::Agent(c) => {
            let is_dir = matches!(candidate, Candidate::Skill(_));
            let item = ctx.source.join(key);
            if hash_item(&item, is_dir).ok().as_deref() != Some(c.base.as_str()) {
                return Err(format!(
                    "changed in both places; {} was edited since {key} was held. Reject it, then refresh",
                    ctx.tilde(&item)
                ));
            }
            let held = state::snapshot_path(&ctx.source, key);
            snapshot(
                &held,
                &staging_for(&item),
                is_dir,
                &c.hash,
                &real_copy(is_dir),
            )?;
            fsx::remove_any(&item)
                .and_then(|()| fs::rename(staging_for(&item), &item).map_err(Into::into))
                .map_err(|e| format!("{e:#}"))?;
            let _ = fsx::remove_any(&held);
            if let Some(o) = st.origins.get_mut(key) {
                o.hash = c.hash.clone();
            }
            st.quarantine.remove(key);
            state::save(&ctx.source, st).map_err(|e| format!("{e:#}"))?;
            let (kind, stem) = if is_dir {
                ("skill", key.trim_start_matches("skills/"))
            } else {
                (
                    "agent",
                    key.trim_start_matches("agents/").trim_end_matches(".md"),
                )
            };
            commit(git, ctx, &format!("colctl: approve {kind} {stem}")).map_err(commit_err)?;
            Ok(format!("approved {key} from {}", ctx.tilde(&c.origin)))
        }
    }
}

/// Where an approved copy is staged before it replaces the source copy.
fn staging_for(item: &Path) -> std::path::PathBuf {
    let name = item
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    item.with_file_name(format!("{}approve-{name}", fsx::TMP_PREFIX))
}

/// `colctl approve` (AC4-23…26): installs exactly the held candidates, one commit each, then syncs.
pub fn approve(ctx: &Ctx, names: &[String], all: bool, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let picked;
    let names = match picked_names(ctx, &ws, names, all, "approve") {
        Picked::Names(n) => {
            picked = n;
            &picked
        }
        Picked::Given => names,
        Picked::Done(code) => return Ok(code),
    };
    let keys = chosen(&ws, names, all, "approve")?;
    if keys.is_empty() {
        ctx.out.line(crate::ui::render::verdict(
            &ctx.out.caps,
            Token::Info,
            "Nothing held for review",
            &[],
        ));
        return Ok(0);
    }
    if dry_run {
        let said: Vec<(Token, String)> = keys
            .iter()
            .map(|key| {
                (
                    Token::Info,
                    format!(
                        "would approve {key}: {}",
                        change_text(ctx, &ws.state.quarantine[key])
                    ),
                )
            })
            .collect();
        let verdict = format!(
            "Would approve {}",
            super::plural(keys.len(), "held version")
        );
        ctx.out
            .lines(super::item_lines(ctx, Token::Info, &verdict, &[], &said));
        return Ok(0);
    }
    if all {
        let listed: Vec<(Token, String)> = keys
            .iter()
            .map(|k| {
                (
                    Token::Info,
                    format!("{k}: {}", change_text(ctx, &ws.state.quarantine[k])),
                )
            })
            .collect();
        if ctx.out.caps.interactive && !ctx.out.caps.yes {
            ctx.out.lines(super::item_lines(
                ctx,
                Token::Info,
                &format!("{} held", super::plural(keys.len(), "version")),
                &[],
                &listed,
            ));
        }
        if !super::confirmed(
            ctx,
            &format!("Approve {}?", super::plural(keys.len(), "held version")),
        ) {
            return Ok(EXIT_DRIFT);
        }
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let ws = load(ctx)?;
    let mut st = ws.state.clone();
    let mut text = ws.config_text.clone();
    let mut refused = 0;
    let mut approved = Vec::new();
    let mut said: Vec<(Token, String)> = Vec::new();
    for key in &keys {
        match approve_one(ctx, &ws, &git, &mut st, &mut text, key) {
            Ok(line) => {
                said.push((Token::Ok, line));
                approved.push(key.clone());
            }
            Err(why) => {
                refused += 1;
                said.push((Token::Attention, format!("refused {key}: {why}")));
            }
        }
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    let (tone, verdict) = match (approved.len(), refused) {
        (0, r) => (
            Token::Attention,
            format!("Refused {}", super::plural(r, "held version")),
        ),
        (a, 0) => (
            Token::Ok,
            format!("Approved {}", super::plural(a, "held version")),
        ),
        (a, r) => (
            Token::Attention,
            format!(
                "Approved {} · refused {r}",
                super::plural(a, "held version")
            ),
        ),
    };
    let lead = super::item_lines(ctx, tone, &verdict, &[], &said);
    if approved.is_empty() {
        ctx.out.lines(lead);
        return Ok(EXIT_DRIFT);
    }
    let code = super::run_sync_led(ctx, &git, None, &lead)?;
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    for key in approved.iter().filter_map(|k| k.strip_prefix("sources/")) {
        let _ = vault.release(key);
    }
    Ok(if refused > 0 { EXIT_DRIFT } else { code })
}

/// `colctl reject` (AC4-27): drops held candidates and remembers their versions, so `update` and
/// `refresh` don't hold them again.
pub fn reject(ctx: &Ctx, names: &[String], all: bool, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let picked;
    let names = match picked_names(ctx, &ws, names, all, "reject") {
        Picked::Names(n) => {
            picked = n;
            &picked
        }
        Picked::Given => names,
        Picked::Done(code) => return Ok(code),
    };
    let keys = chosen(&ws, names, all, "reject")?;
    if keys.is_empty() {
        ctx.out.line(crate::ui::render::verdict(
            &ctx.out.caps,
            Token::Info,
            "Nothing held for review",
            &[],
        ));
        return Ok(0);
    }
    if dry_run {
        let said: Vec<(Token, String)> = keys
            .iter()
            .map(|key| {
                (
                    Token::Info,
                    format!(
                        "would reject {key}: {}",
                        change_text(ctx, &ws.state.quarantine[key])
                    ),
                )
            })
            .collect();
        let verdict = format!("Would reject {}", super::plural(keys.len(), "held version"));
        ctx.out
            .lines(super::item_lines(ctx, Token::Info, &verdict, &[], &said));
        return Ok(0);
    }
    if all {
        let listed: Vec<(Token, String)> = keys
            .iter()
            .map(|k| {
                (
                    Token::Info,
                    format!("{k}: {}", change_text(ctx, &ws.state.quarantine[k])),
                )
            })
            .collect();
        if ctx.out.caps.interactive && !ctx.out.caps.yes {
            ctx.out.lines(super::item_lines(
                ctx,
                Token::Info,
                &format!("{} held", super::plural(keys.len(), "version")),
                &[],
                &listed,
            ));
        }
        if !super::confirmed(
            ctx,
            &format!("Reject {}?", super::plural(keys.len(), "held version")),
        ) {
            return Ok(EXIT_DRIFT);
        }
    }
    let _lock = Lock::acquire(&ctx.source)?;
    let mut st = state::load(&ctx.source)?;
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    let mut said: Vec<(Token, String)> = Vec::new();
    for key in &keys {
        let Some(candidate) = st.quarantine.remove(key) else {
            continue;
        };
        st.rejected
            .entry(key.clone())
            .or_default()
            .insert(candidate.version().to_string());
        drop_held(ctx, &vault, key);
        said.push((
            Token::Ok,
            format!("rejected {key}: {}", change_text(ctx, &candidate)),
        ));
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    let verdict = format!(
        "Rejected {} · they won't be offered again",
        super::plural(said.len(), "held version")
    );
    ctx.out
        .lines(super::item_lines(ctx, Token::Ok, &verdict, &[], &said));
    Ok(0)
}

/// Deletes what keeps a candidate: the clone's ref, or the snapshot.
fn drop_held(ctx: &Ctx, vault: &Vault, key: &str) {
    match key.strip_prefix("sources/") {
        Some(name) => {
            let _ = vault.release(name);
        }
        None => {
            let _ = fsx::remove_any(&state::snapshot_path(&ctx.source, key));
        }
    }
}

/// Forgets everything held or rejected for an item that's being removed (AC4-28).
pub(super) fn forget(ctx: &Ctx, git: &Git, key: &str) -> Result<(), CliError> {
    let mut st = state::load(&ctx.source)?;
    if !st.quarantine.contains_key(key) && !st.rejected.contains_key(key) {
        return Ok(());
    }
    let vault = Vault {
        git,
        dir: plugins_dir(ctx),
    };
    drop_held(ctx, &vault, key);
    st.forget(key);
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec4_18_a_bad_snapshot_copy_holds_nothing_and_leaves_nothing() {
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("o/skill");
        fs::create_dir_all(&origin).unwrap();
        fs::write(origin.join("SKILL.md"), "real").unwrap();
        let expected = fsx::hash_tree(&origin).unwrap();
        let snap = t.path().join("q/skills/skill");
        let corrupting = |from: &Path, to: &Path| -> anyhow::Result<()> {
            fsx::copy_tree(from, to, false)?;
            fs::write(to.join("SKILL.md"), "tampered")?;
            Ok(())
        };
        assert_eq!(
            snapshot(&origin, &snap, true, &expected, &corrupting),
            Err("the copy didn't match the origin; nothing held".into())
        );
        let failing = |_: &Path, _: &Path| -> anyhow::Result<()> { anyhow::bail!("disk full") };
        assert!(snapshot(&origin, &snap, true, &expected, &failing).is_err());
        assert!(!snap.exists());
        assert!(fsx::find_temps(snap.parent().unwrap()).is_empty());
        snapshot(&origin, &snap, true, &expected, &real_copy(true)).unwrap();
        assert_eq!(fs::read_to_string(snap.join("SKILL.md")).unwrap(), "real");
        // A second hold replaces the first.
        fs::write(origin.join("SKILL.md"), "newer").unwrap();
        let newer = fsx::hash_tree(&origin).unwrap();
        snapshot(&origin, &snap, true, &newer, &real_copy(true)).unwrap();
        assert_eq!(fs::read_to_string(snap.join("SKILL.md")).unwrap(), "newer");
    }
}
