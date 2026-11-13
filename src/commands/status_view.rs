//! `colctl status` as text (cycle 6, interfaces.md § status): a verdict, one line per profile with
//! what it has, then only what needs attention, each with its fix. `--all` lists every item.

use super::quarantine::View;
use super::{Planned, Workspace, root_hint};
use crate::adapter::Root;
use crate::config::Kind;
use crate::ctx::Ctx;
use crate::plan::{ItemKind, ItemState};
use crate::review::escape;
use crate::ui::render::{self, Cell};
use crate::ui::style::{Token, paint};

/// How a state reads in status.
fn tone(state: ItemState) -> Token {
    match state {
        ItemState::Ok => Token::Ok,
        ItemState::Unsupported | ItemState::Unreviewed => Token::Info,
        _ => Token::Attention,
    }
}

/// What a profile has, by kind, then its problems (AC6-34): `32 skills · 1 plugin · CLAUDE.md · 1
/// missing`. A Codex profile reads skills from `~/.agents`, which is said when that's registered.
fn contents(p: &Planned, agents: Option<&str>) -> Vec<Cell> {
    let items = &p.plan.items;
    let present = |kinds: &[ItemKind]| {
        items
            .iter()
            .filter(|i| kinds.contains(&i.kind) && i.state != ItemState::Unsupported)
            .count()
    };
    let mut parts = Vec::new();
    for (kinds, word) in [
        (&[ItemKind::Skill][..], "skill"),
        (&[ItemKind::Agent][..], "agent"),
        (&[ItemKind::Plugin, ItemKind::Source][..], "plugin"),
    ] {
        let n = present(kinds);
        if n > 0 {
            parts.push(Cell::plain(super::plural(n, word)));
        }
    }
    if let Some(i) = items
        .iter()
        .find(|i| i.kind == ItemKind::Instructions && i.state != ItemState::Unsupported)
    {
        parts.push(Cell::plain(i.name.clone()));
    }
    if p.harness.kind == Kind::Codex
        && let Some(agents) = agents
    {
        parts.push(Cell::styled(format!("skills via {agents}"), Token::Dim));
    }
    let order = [
        ItemState::Missing,
        ItemState::Broken,
        ItemState::Orphaned,
        ItemState::Foreign,
        ItemState::ForeignLink,
        ItemState::Edited,
        ItemState::Invalid,
        ItemState::PluginMissing,
        ItemState::MarketplaceMissing,
        ItemState::Outdated,
        ItemState::FromAggregator,
        ItemState::Unsupported,
        ItemState::Unreviewed,
    ];
    for state in order {
        let n = items.iter().filter(|i| i.state == state).count();
        if n == 0 {
            continue;
        }
        let words = match state {
            ItemState::Unsupported
                if items
                    .iter()
                    .all(|i| i.state != state || i.kind == ItemKind::Source) =>
            {
                "not packaged"
            }
            s => s.as_str(),
        };
        let token = tone(state);
        parts.push(Cell::styled(
            format!("{n} {words}"),
            if token == Token::Info {
                Token::Dim
            } else {
                token
            },
        ));
    }
    if parts.is_empty() {
        parts.push(Cell::styled("nothing yet", Token::Dim));
    }
    parts
}

/// One line per profile (AC6-34, AC6-35): mark, name, path, then what it has. Paths are cut alike
/// when the column needs it, and what it has wraps under itself rather than being cut.
fn profile_lines(ctx: &Ctx, planned: &[Planned]) -> Vec<String> {
    let caps = &ctx.out.caps;
    let agents = planned
        .iter()
        .find(|p| p.harness.kind == Kind::Agents)
        .map(|p| ctx.tilde(&p.harness.path));
    let rows: Vec<(Token, &str, String, Vec<Cell>)> = planned
        .iter()
        .map(|p| {
            let path = ctx.tilde(&p.harness.path);
            if p.plan.root == Root::Ok {
                let mark = if p
                    .plan
                    .items
                    .iter()
                    .any(|i| tone(i.state) == Token::Attention)
                {
                    Token::Attention
                } else {
                    Token::Ok
                };
                (
                    mark,
                    p.harness.name.as_str(),
                    path,
                    contents(p, agents.as_deref()),
                )
            } else {
                (
                    Token::Failed,
                    p.harness.name.as_str(),
                    path,
                    vec![
                        Cell::plain(super::root_str(p.plan.root)),
                        Cell::styled(format!("→ {}", root_hint(p.plan.root)), Token::Command),
                    ],
                )
            }
        })
        .collect();
    let joined = |parts: &[Cell]| -> usize {
        parts.iter().map(|c| render::width(&c.text)).sum::<usize>()
            + 3 * parts.len().saturating_sub(1)
    };
    let name_w = rows.iter().map(|r| render::width(r.1)).max().unwrap_or(0);
    let fixed = 2 + 1 + 2 + name_w + 2 + 2;
    let path_w = rows.iter().map(|r| render::width(&r.2)).max().unwrap_or(0);
    let contents_w = rows.iter().map(|r| joined(&r.3)).max().unwrap_or(0);
    let widest_part = rows
        .iter()
        .flat_map(|r| r.3.iter().map(|c| render::width(&c.text)))
        .max()
        .unwrap_or(0);
    let paths: Vec<String> = rows.iter().map(|r| r.2.clone()).collect();
    let paths = if fixed + path_w + contents_w <= caps.width {
        paths
    } else {
        let keep = contents_w.min(widest_part.max(24));
        render::cut_alike(&paths, caps.width.saturating_sub(fixed + keep).max(20))
    };
    let path_w = paths.iter().map(|p| render::width(p)).max().unwrap_or(0);
    let room = caps
        .width
        .saturating_sub(fixed + path_w)
        .max(widest_part.min(24));
    let mut lines = Vec::new();
    for ((mark, name, _, parts), path) in rows.iter().zip(&paths) {
        // Parts wrap onto lines of at most `room` columns, under the contents column.
        let mut wrapped: Vec<Vec<&Cell>> = vec![Vec::new()];
        let mut used = 0;
        for part in parts {
            let w = render::width(&part.text);
            let line = wrapped.last_mut().unwrap();
            if !line.is_empty() && used + 3 + w > room {
                wrapped.push(vec![part]);
                used = w;
            } else {
                used += w + if line.is_empty() { 0 } else { 3 };
                line.push(part);
            }
        }
        for (n, line) in wrapped.iter().enumerate() {
            let text = line
                .iter()
                .map(|c| {
                    let t = render::cut(&c.text, room);
                    match c.token {
                        Some(token) => paint(&t, token, caps.color_out),
                        None => t,
                    }
                })
                .collect::<Vec<_>>()
                .join(&paint(" · ", Token::Dim, caps.color_out));
            if n == 0 {
                lines.push(format!(
                    "  {}  {}  {}  {text}",
                    crate::ui::style::mark(*mark, caps.color_out),
                    pad(&paint(name, Token::Name, caps.color_out), name_w),
                    pad(&paint(path, Token::Dim, caps.color_out), path_w),
                ));
            } else {
                lines.push(format!("{}{text}", " ".repeat(fixed + path_w)));
            }
        }
    }
    lines
}

fn pad(text: &str, to: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(to.saturating_sub(render::width(text)))
    )
}

/// One row of the "Needs attention" table.
pub(super) struct Attention {
    place: String,
    kind: String,
    name: String,
    what: String,
    fix: String,
}

pub(super) fn row(a: &Attention) -> (Vec<Cell>, String, Token) {
    (
        vec![
            Cell::styled(Token::Attention.symbol(), Token::Attention),
            Cell::styled(a.place.clone(), Token::Name),
            Cell::plain(a.kind.clone()),
            Cell::plain(a.name.clone()),
            Cell::plain(a.what.clone()),
        ],
        a.fix.clone(),
        Token::Command,
    )
}

/// Item rows, each followed by its fix: on the same line when it fits, else on its own line
/// underneath (fixes are never cut).
pub(super) fn rows_with_fixes(
    caps: &crate::ui::term::Caps,
    rows: &[(Vec<Cell>, String, Token)],
    flex: usize,
) -> Vec<String> {
    let cells: Vec<Vec<Cell>> = rows.iter().map(|(c, _, _)| c.clone()).collect();
    let lines = render::table(caps, 2, &cells, flex);
    // Fixes share one column: rows are padded to the widest row that has a fix.
    let fix_col = lines
        .iter()
        .zip(rows)
        .filter(|(_, (_, fix, _))| !fix.is_empty())
        .map(|(l, _)| render::width(l))
        .max()
        .unwrap_or(0);
    let mut out = Vec::new();
    for (line, (_, fix, token)) in lines.into_iter().zip(rows) {
        if fix.is_empty() {
            out.push(line);
            continue;
        }
        let arrow = format!("→ {fix}");
        if fix_col + 2 + render::width(&arrow) <= caps.width {
            let pad = fix_col - render::width(&line);
            out.push(format!(
                "{line}{}  {}",
                " ".repeat(pad),
                crate::ui::style::paint(&arrow, *token, caps.color_out)
            ));
        } else {
            out.push(line);
            out.push(format!(
                "      {}",
                crate::ui::style::paint(&arrow, *token, caps.color_out)
            ));
        }
    }
    out
}

/// Everything that needs the person, each with its fix: problem items in usable profiles, uncommitted
/// files, invalid source items, changed origins and held versions.
pub(super) fn attention_rows(
    ctx: &Ctx,
    ws: &Workspace,
    planned: &[Planned],
    uncommitted: &[String],
    held: &[View],
) -> Vec<Attention> {
    let mut attention: Vec<Attention> = Vec::new();
    let source = ctx.tilde(&ctx.source);
    for p in planned {
        if p.plan.root != Root::Ok {
            continue;
        }
        for i in p
            .plan
            .items
            .iter()
            .filter(|i| tone(i.state) == Token::Attention)
        {
            attention.push(Attention {
                place: p.harness.name.clone(),
                kind: i.kind.as_str().to_string(),
                name: escape(&i.name),
                what: i.state.as_str().to_string(),
                fix: i.hint.clone().map(|h| escape(&h)).unwrap_or_default(),
            });
        }
    }
    for path in uncommitted {
        attention.push(Attention {
            place: source.clone(),
            kind: "file".into(),
            name: escape(path),
            what: "uncommitted".into(),
            fix: "colctl sync".into(),
        });
    }
    for i in &ws.source.invalid {
        attention.push(Attention {
            place: source.clone(),
            kind: i.kind.to_string(),
            name: escape(&i.name),
            what: "invalid".into(),
            fix: escape(&i.reason),
        });
    }
    for (item, _) in &ws.origin_changed {
        attention.push(Attention {
            place: source.clone(),
            kind: item
                .split('/')
                .next()
                .unwrap_or("item")
                .trim_end_matches('s')
                .into(),
            name: escape(item),
            what: "origin-changed".into(),
            fix: format!("colctl refresh {}", escape(item)),
        });
    }
    for v in held {
        let change = match &v.candidate {
            crate::state::Candidate::Source(c) => format!(
                "{} → {}",
                crate::vault::short(&c.from),
                crate::vault::short(&c.to)
            ),
            crate::state::Candidate::Skill(c) | crate::state::Candidate::Agent(c) => {
                escape(&ctx.tilde(&c.origin))
            }
            crate::state::Candidate::Conversion(c) => {
                format!("Codex package from {}", crate::vault::short(&c.rev))
            }
        };
        attention.push(Attention {
            place: "held".into(),
            kind: v.candidate.kind().into(),
            name: escape(&v.name),
            what: change,
            fix: format!("colctl review {}", escape(&v.name)),
        });
    }
    attention
}

/// The text of `colctl status` (AC6-08 – 11, EC6-10, EC6-11).
pub(super) fn render(
    ctx: &Ctx,
    ws: &Workspace,
    planned: &[Planned],
    uncommitted: &[String],
    held: &[View],
    all: bool,
) -> Vec<String> {
    let caps = &ctx.out.caps;
    if planned.is_empty() {
        return vec![
            render::verdict(caps, Token::Attention, "No profiles registered", &[]),
            String::new(),
            render::hint(caps, "colctl init"),
        ];
    }
    let attention = attention_rows(ctx, ws, planned, uncommitted, held);
    let unusable = planned.iter().filter(|p| p.plan.root != Root::Ok).count();
    let problems = attention.len() + unusable;

    let mut lines = Vec::new();
    if problems == 0 {
        let held_fact = "nothing held for review".to_string();
        let sources = ws.config.sources.len();
        let sources_fact = format!("{sources} source{}", if sources == 1 { "" } else { "s" });
        let n = planned.len();
        let what = if n == 1 {
            "The profile is in sync".to_string()
        } else {
            format!("All {n} profiles in sync")
        };
        lines.push(render::verdict(
            caps,
            Token::Ok,
            &what,
            &[sources_fact, held_fact],
        ));
    } else {
        let word = if problems == 1 {
            "thing needs"
        } else {
            "things need"
        };
        lines.push(render::verdict(
            caps,
            Token::Attention,
            &format!("{problems} {word} attention"),
            &[],
        ));
    }
    if caps.quiet {
        return lines;
    }

    // One line per profile.
    lines.push(String::new());
    lines.extend(profile_lines(ctx, planned));

    if !attention.is_empty() {
        lines.push(String::new());
        lines.push(render::heading(caps, "Needs attention"));
        let rows: Vec<(Vec<Cell>, String, Token)> = attention.iter().map(row).collect();
        lines.extend(rows_with_fixes(caps, &rows, 3));
    }

    if all {
        for p in planned.iter().filter(|p| p.plan.root == Root::Ok) {
            lines.push(String::new());
            let room = caps
                .width
                .saturating_sub(render::width(&p.harness.name) + 2)
                .max(16);
            lines.push(format!(
                "{}  {}",
                render::heading(caps, &p.harness.name),
                render::footer(caps, &render::cut_middle(&ctx.tilde(&p.harness.path), room))
            ));
            if p.plan.items.is_empty() {
                lines.push(format!("  {} 0 items", Token::Info.symbol()));
                continue;
            }
            let rows: Vec<(Vec<Cell>, String, Token)> = p
                .plan
                .items
                .iter()
                .map(|i| {
                    let t = tone(i.state);
                    (
                        vec![
                            Cell::styled(t.symbol(), t),
                            Cell::plain(i.state.as_str()),
                            Cell::plain(i.kind.as_str()),
                            Cell::plain(escape(&i.name)),
                        ],
                        i.hint.as_ref().map(|h| escape(h)).unwrap_or_default(),
                        if t == Token::Info {
                            Token::Dim
                        } else {
                            Token::Command
                        },
                    )
                })
                .collect();
            lines.extend(rows_with_fixes(caps, &rows, 3));
        }
    } else {
        lines.push(String::new());
        lines.push(render::hint(caps, "colctl status --all lists every item"));
    }
    lines
}
