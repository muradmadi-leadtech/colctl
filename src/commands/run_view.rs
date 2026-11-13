//! What a run did, as text (cycle 6, interfaces.md § Running commands): outcomes in past tense per
//! profile, informational notes that never count as problems, failures with their reason, and what
//! still needs attention. Tool calls appear with `-v`.

use super::Planned;
use super::status_view::{Attention, attention_rows, rows_with_fixes};

use crate::adapter::Root;
use crate::apply::{Applied, Outcome};
use crate::ctx::Ctx;
use crate::plan::{Action, ItemState};
use crate::review::escape;
use crate::ui::render::{self, Cell};
use crate::ui::style::Token;
use std::path::Path;

/// What an action is about: (kind, name). `None` for housekeeping nobody needs to see.
fn subject(action: &Action) -> Option<(&'static str, String)> {
    let item = |p: &Path| -> (&'static str, String) {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match p
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
        {
            Some("skills") => ("skill", name),
            Some("agents") => ("agent", name.trim_end_matches(".md").to_string()),
            _ => ("file", name),
        }
    };
    let plugin = |id: &str| ("plugin", id.split('@').next().unwrap_or(id).to_string());
    Some(match action {
        Action::Link { link, .. } | Action::Unlink(link) => item(link),
        Action::WriteGenerated { rel, .. } | Action::RemoveGenerated { rel, .. } => {
            ("file", rel.clone())
        }
        Action::AddMarketplace { name, .. }
        | Action::AddSourceMarketplace { name, .. }
        | Action::RemoveMarketplace { name }
        | Action::RemoveSourceMarketplace { name } => ("marketplace", name.clone()),
        Action::Install { id }
        | Action::InstallSource { id, .. }
        | Action::UpdateSource { id, .. }
        | Action::Uninstall { id }
        | Action::UninstallSource { id, .. } => plugin(id),
        Action::CleanupTemp(_)
        | Action::RecordGenerated { .. }
        | Action::ForgetMarketplace { .. }
        | Action::ForgetPlugin { .. }
        | Action::ForgetSource { .. } => return None,
    })
}

/// The verb for an action: (past tense, -ing, future).
fn verb(action: &Action) -> (&'static str, &'static str, &'static str) {
    match action {
        Action::Link { .. } => ("linked", "linking", "link"),
        Action::Unlink(_) | Action::RemoveGenerated { .. } => ("removed", "removing", "remove"),
        Action::WriteGenerated { .. } => ("wrote", "writing", "write"),
        Action::AddMarketplace { .. } | Action::AddSourceMarketplace { .. } => {
            ("added", "adding", "add")
        }
        Action::RemoveMarketplace { .. } | Action::RemoveSourceMarketplace { .. } => {
            ("removed", "removing", "remove")
        }
        Action::Install { .. } | Action::InstallSource { .. } => {
            ("installed", "installing", "install")
        }
        Action::UpdateSource { .. } => ("updated", "updating", "update"),
        Action::Uninstall { .. } | Action::UninstallSource { .. } => {
            ("uninstalled", "uninstalling", "uninstall")
        }
        _ => ("did", "doing", "do"),
    }
}

/// What a profile is doing right now, for the live view.
pub(super) fn doing(action: &Action) -> String {
    match subject(action) {
        Some((kind, name)) if kind != "plugin" => format!("{} {kind} {name}", verb(action).1),
        Some((_, name)) => format!("{} {name}", verb(action).1),
        None => "tidying up".into(),
    }
}

/// Outcomes grouped by verb and kind, counted when more than one (AC6-12):
/// "installed superpowers · linked 2 skills". `tense` picks past (0) or future (2).
/// A group: (verb, kind), the names, and the commit for an update.
type Group<'a> = ((&'a str, &'a str), Vec<String>, Option<String>);

fn phrases<'a>(actions: impl Iterator<Item = &'a Action>, tense: usize) -> (Vec<String>, usize) {
    let mut groups: Vec<Group> = Vec::new();
    let mut count = 0;
    for a in actions {
        let Some((kind, name)) = subject(a) else {
            continue;
        };
        count += 1;
        let v = verb(a);
        let word = [v.0, v.1, v.2][tense];
        let rev = match a {
            Action::UpdateSource { rev, .. } => Some(crate::vault::short(rev).to_string()),
            _ => None,
        };
        match groups.iter_mut().find(|(k, _, _)| *k == (word, kind)) {
            Some((_, names, _)) => names.push(name),
            None => groups.push(((word, kind), vec![name], rev)),
        }
    }
    let out = groups
        .into_iter()
        .map(|((word, kind), names, rev)| {
            let prefix = if tense == 2 { "would " } else { "" };
            match (names.as_slice(), kind) {
                ([one], "plugin") => match rev {
                    Some(r) => format!("{prefix}{word} {} to {r}", escape(one)),
                    None => format!("{prefix}{word} {}", escape(one)),
                },
                ([one], k) => format!("{prefix}{word} {k} {}", escape(one)),
                // A few are named; more are counted.
                (few, "plugin") if few.len() <= 3 => {
                    format!("{prefix}{word} {}", escape(&few.join(", ")))
                }
                (few, k) if few.len() <= 3 => {
                    format!("{prefix}{word} {k}s {}", escape(&few.join(", ")))
                }
                (many, k) => format!("{prefix}{word} {} {k}s", many.len()),
            }
        })
        .collect();
    (out, count)
}

/// The first line of a tool's error, for a summary.
fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    escape(&render::cut(line, 100))
}

/// The rest of a tool's error after its first line, escaped, up to 30 lines.
fn more_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .skip_while(|l| l.trim().is_empty())
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .take(30)
        .map(escape)
        .collect()
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

/// A short summary for the live view's finished line.
pub(super) fn short_summary(results: &[Applied]) -> String {
    let failed = results.iter().filter(|r| !r.outcome.is_done()).count();
    let changes = results
        .iter()
        .filter(|r| r.outcome.is_done() && subject(&r.action).is_some())
        .count();
    match (failed, changes) {
        (0, 0) => "· nothing to change".into(),
        (0, n) => format!("✓ {n} change{}", if n == 1 { "" } else { "s" }),
        (f, _) => format!("✗ {f} failed"),
    }
}

/// The result of a run (AC6-12 – 17). `lead` is printed first (a command's own verdict); then one
/// line per profile, failures, informational notes, what still needs attention, and a verdict when
/// there's no lead or something went wrong. Returns the lines and whether everything is in sync.
pub(super) fn report(
    ctx: &Ctx,
    lead: &[String],
    results: &[(String, Vec<Applied>)],
    after: &super::Workspace,
    replanned: &[Planned],
    in_sync: bool,
) -> Vec<String> {
    let caps = &ctx.out.caps;
    let mut rows: Vec<(Vec<Cell>, String, Token)> = Vec::new();
    // Each profile's outcomes, for when they don't fit on one line.
    let mut stacked: Vec<Vec<String>> = Vec::new();
    let mut detail: Vec<(usize, Vec<String>)> = Vec::new();
    let mut changes = 0;
    let mut failed_profiles = 0;
    let mut failures = 0;
    for p in replanned {
        let name = p.harness.name.clone();
        if p.plan.root != Root::Ok {
            rows.push((
                vec![
                    Cell::styled(Token::Failed.symbol(), Token::Failed),
                    Cell::styled(name, Token::Name),
                    Cell::plain(super::root_str(p.plan.root)),
                ],
                super::root_hint(p.plan.root).into(),
                Token::Command,
            ));
            stacked.push(Vec::new());
            continue;
        }
        let applied: &[Applied] = results
            .iter()
            .find(|(n, _)| n == &p.harness.name)
            .map(|(_, r)| r.as_slice())
            .unwrap_or_default();
        let (parts, n) = phrases(
            applied
                .iter()
                .filter(|r| r.outcome.is_done())
                .map(|r| &r.action),
            0,
        );
        changes += n;
        let failed: Vec<&Applied> = applied.iter().filter(|r| !r.outcome.is_done()).collect();
        let notes: Vec<String> = p
            .plan
            .items
            .iter()
            .filter(|i| i.state == ItemState::Unsupported)
            .map(|i| {
                format!(
                    "{}: {}",
                    escape(&i.name),
                    escape(i.hint.as_deref().unwrap_or("not supported here"))
                )
            })
            .collect();
        let mark = if !failed.is_empty() {
            failed_profiles += 1;
            failures += failed.len();
            Token::Failed
        } else if !parts.is_empty() {
            Token::Ok
        } else {
            Token::Info
        };
        let mut items: Vec<String> = parts;
        if !failed.is_empty() {
            items.push(format!("{} failed", plural(failed.len(), "step")));
        }
        items.extend(notes.iter().map(|n| format!("· {n}")));
        if items.is_empty() {
            items.push("· nothing to change".into());
        }
        rows.push((
            vec![
                Cell::styled(mark.symbol(), mark),
                Cell::styled(name, Token::Name),
            ],
            items.join(" · ").replace(" · · ", "  · "),
            Token::Command,
        ));
        stacked.push(items);
        let mut extra = Vec::new();
        for f in &failed {
            let (reason, rest) = match &f.outcome {
                Outcome::Failed(r) => (first_line(r), more_lines(r)),
                Outcome::Skipped(r) => (format!("skipped: {}", first_line(r)), more_lines(r)),
                Outcome::Done => (String::new(), Vec::new()),
            };
            let what = match subject(&f.action) {
                Some(("plugin", n)) => format!("{} {n}", verb(&f.action).2),
                Some((k, n)) => format!("{} {k} {n}", verb(&f.action).2),
                None => escape(&f.description),
            };
            extra.push(format!(
                "      {} {what} failed: {reason}",
                crate::ui::style::mark(Token::Failed, caps.color_out)
            ));
            // AC-30: a tool's error is shown in full.
            extra.extend(rest.into_iter().map(|l| format!("          {l}")));
        }
        if caps.verbose {
            for r in applied {
                let t = if r.outcome.is_done() {
                    Token::Ok
                } else {
                    Token::Failed
                };
                extra.push(format!(
                    "      {} {}  {}",
                    crate::ui::style::mark(t, caps.color_out),
                    escape(&r.description),
                    render::footer(caps, &format!("{:.1}s", r.took.as_secs_f64()))
                ));
            }
        }
        if !extra.is_empty() {
            detail.push((rows.len() - 1, extra));
        }
    }
    let usable = replanned.iter().filter(|p| p.plan.root == Root::Ok).count();
    let attention: Vec<Attention> = attention_rows(ctx, after, replanned, &[], &[]);
    let unusable = replanned.len() - usable;

    let verdict = if failures > 0 {
        Some(render::verdict(
            caps,
            Token::Failed,
            &format!("Synced {} of {} profiles", usable - failed_profiles, usable),
            &[format!("{failures} failed")],
        ))
    } else if !attention.is_empty() || unusable > 0 {
        Some(render::verdict(
            caps,
            Token::Attention,
            &format!("Synced {}", plural(usable, "profile")),
            &[
                plural(changes, "change"),
                format!(
                    "{} {} attention",
                    attention.len() + unusable,
                    if attention.len() + unusable == 1 {
                        "needs"
                    } else {
                        "need"
                    }
                ),
            ],
        ))
    } else if lead.is_empty() {
        Some(if changes == 0 {
            render::verdict(caps, Token::Ok, "Everything was already in sync", &[])
        } else {
            render::verdict(
                caps,
                Token::Ok,
                &format!("Synced {}", plural(usable, "profile")),
                &[plural(changes, "change")],
            )
        })
    } else {
        None
    };
    let _ = in_sync;

    let mut lines: Vec<String> = lead.to_vec();
    let problem = failures > 0 || !attention.is_empty() || unusable > 0;
    if caps.quiet {
        if lines.is_empty() || problem {
            lines.extend(verdict);
        }
        return lines
            .into_iter()
            .take(if problem { 2 } else { 1 })
            .collect();
    }
    // Without a lead, the verdict comes first; with one, a problem verdict closes the block.
    if lead.is_empty() {
        lines.extend(verdict.clone());
    }
    if !rows.is_empty() {
        lines.push(String::new());
        lines.extend(profile_block(caps, &rows, &stacked, &detail));
    }
    if !attention.is_empty() {
        lines.push(String::new());
        lines.push(render::heading(caps, "Needs attention"));
        let rows: Vec<(Vec<Cell>, String, Token)> =
            attention.iter().map(super::status_view::row).collect();
        lines.extend(rows_with_fixes(caps, &rows, 3));
    }
    if failures > 0 {
        lines.push(String::new());
        lines.push(render::hint(
            caps,
            "colctl sync retries · colctl sync -v shows each call",
        ));
    }
    if !lead.is_empty() && problem {
        lines.push(String::new());
        lines.extend(verdict);
    }
    lines
}

/// One line per profile: its mark, name and outcomes, on one line when they fit, else one outcome per
/// line under the name; then that profile's failures (and calls, with -v). Nothing is cut.
fn profile_block(
    caps: &crate::ui::term::Caps,
    rows: &[(Vec<Cell>, String, Token)],
    stacked: &[Vec<String>],
    detail: &[(usize, Vec<String>)],
) -> Vec<String> {
    let name_w = rows
        .iter()
        .map(|(c, _, _)| render::width(&c[1].text))
        .max()
        .unwrap_or(0);
    let indent = 2 + 1 + 2 + name_w + 2;
    let mut out = Vec::new();
    for (i, (cells, text, _)) in rows.iter().enumerate() {
        let mark = crate::ui::style::paint(
            &cells[0].text,
            cells[0].token.unwrap_or(Token::Info),
            caps.color_out,
        );
        let name = crate::ui::style::paint(&cells[1].text, Token::Name, caps.color_out);
        let pad = " ".repeat(name_w - render::width(&cells[1].text));
        let state = cells.get(2).map(|c| c.text.clone());
        let one = match &state {
            // An unusable profile: its state, and its fix.
            Some(st) => format!("{st}  → {text}"),
            None => text.clone(),
        };
        if indent + render::width(&one) <= caps.width || stacked[i].len() <= 1 {
            out.push(
                format!("  {mark}  {name}{pad}  {one}")
                    .trim_end()
                    .to_string(),
            );
        } else {
            out.push(format!("  {mark}  {name}"));
            for part in &stacked[i] {
                out.push(format!("{}{part}", " ".repeat(indent)));
            }
        }
        if let Some((_, extra)) = detail.iter().find(|(r, _)| *r == i) {
            out.extend(extra.iter().cloned());
        }
    }
    out
}

/// What `--dry-run` would do (AC6-17): a verdict, then each profile's changes in future tense.
pub(super) fn plan(ctx: &Ctx, planned: &[Planned]) -> Vec<String> {
    let caps = &ctx.out.caps;
    let mut rows = Vec::new();
    let mut stacked = Vec::new();
    let mut total = 0;
    let mut profiles = 0;
    for p in planned {
        let (parts, n) = phrases(p.plan.actions.iter(), 2);
        total += n;
        if n > 0 {
            profiles += 1;
        }
        let items = if parts.is_empty() {
            vec!["· nothing to change".to_string()]
        } else {
            parts
        };
        rows.push((
            vec![
                Cell::styled(if n > 0 { "·" } else { " " }, Token::Info),
                Cell::styled(p.harness.name.clone(), Token::Name),
            ],
            items.join(" · "),
            Token::Command,
        ));
        stacked.push(items);
    }
    let verdict = if total == 0 {
        render::verdict(caps, Token::Ok, "Nothing to change", &[])
    } else {
        render::verdict(
            caps,
            Token::Info,
            &format!(
                "Would make {total} change{} in {profiles} profile{}",
                if total == 1 { "" } else { "s" },
                if profiles == 1 { "" } else { "s" }
            ),
            &[],
        )
    };
    let mut lines = vec![verdict];
    if caps.quiet {
        return lines;
    }
    lines.push(String::new());
    lines.extend(profile_block(caps, &rows, &stacked, &[]));
    if caps.verbose {
        for p in planned {
            for a in &p.plan.actions {
                lines.push(format!(
                    "      {}  {}",
                    p.harness.name,
                    escape(&a.describe(crate::tool::Tool::for_kind(p.harness.kind)))
                ));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ac6_12_outcomes_grouped_and_counted() {
        let link = |n: &str| Action::Link {
            link: PathBuf::from(format!("/h/skills/{n}")),
            target: PathBuf::from("/s"),
        };
        let actions = [
            Action::InstallSource {
                source: "sp".into(),
                id: "superpowers@m".into(),
                marketplace: "m".into(),
                rev: "a".repeat(40),
            },
            link("a"),
            link("b"),
            Action::RecordGenerated {
                rel: "x".into(),
                hash: "h".into(),
            },
            Action::UpdateSource {
                source: "q".into(),
                id: "qdrant@q".into(),
                marketplace: "q".into(),
                rev: "b".repeat(40),
            },
            Action::Link {
                link: PathBuf::from("/h/agents/rev.md"),
                target: PathBuf::from("/s"),
            },
        ];
        let (past, n) = phrases(actions.iter(), 0);
        assert_eq!(
            past,
            vec![
                "installed superpowers",
                "linked skills a, b",
                "updated qdrant to bbbbbbb",
                "linked agent rev"
            ]
        );
        assert_eq!(n, 5, "housekeeping isn't counted");
        let (future, _) = phrases(actions[..1].iter(), 2);
        assert_eq!(future, vec!["would install superpowers"]);
        assert_eq!(doing(&actions[0]), "installing superpowers");
        assert_eq!(doing(&actions[1]), "linking skill a");
        assert_eq!(doing(&actions[3]), "tidying up");
        assert_eq!(first_line("\n  boom\nmore"), "boom");
    }
}
