//! `colctl status` as text (cycle 6, interfaces.md § status): a verdict, one line per profile with
//! counts, then only what needs attention, each with its fix. `--all` lists every item.

use super::quarantine::View;
use super::{Planned, Workspace, root_hint};
use crate::adapter::Root;
use crate::ctx::Ctx;
use crate::plan::{Item, ItemState};
use crate::review::escape;
use crate::ui::render::{self, Cell};
use crate::ui::style::Token;

/// How a state reads in status.
fn tone(state: ItemState) -> Token {
    match state {
        ItemState::Ok => Token::Ok,
        ItemState::Unsupported | ItemState::Unreviewed => Token::Info,
        _ => Token::Attention,
    }
}

/// Counts per state for one profile, ok first, then problems, then informational, in enum order.
fn counts(items: &[Item]) -> String {
    let order = [
        ItemState::Ok,
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
    let parts: Vec<String> = order
        .iter()
        .filter_map(|s| {
            let n = items.iter().filter(|i| i.state == *s).count();
            (n > 0).then(|| format!("{n} {}", s.as_str()))
        })
        .collect();
    if parts.is_empty() {
        "0 items".to_string()
    } else {
        parts.join(" · ")
    }
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
    let path_room = (caps.width * 2 / 5).max(16);
    let rows: Vec<(Vec<Cell>, String, Token)> = planned
        .iter()
        .map(|p| {
            let path = render::cut_middle(&ctx.tilde(&p.harness.path), path_room);
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
                    vec![
                        Cell::styled(mark.symbol(), mark),
                        Cell::styled(p.harness.name.clone(), Token::Name),
                        Cell::styled(path, Token::Dim),
                        Cell::plain(counts(&p.plan.items)),
                    ],
                    String::new(),
                    Token::Command,
                )
            } else {
                (
                    vec![
                        Cell::styled(Token::Failed.symbol(), Token::Failed),
                        Cell::styled(p.harness.name.clone(), Token::Name),
                        Cell::styled(path, Token::Dim),
                        Cell::plain(super::root_str(p.plan.root)),
                    ],
                    root_hint(p.plan.root).to_string(),
                    Token::Command,
                )
            }
        })
        .collect();
    lines.extend(rows_with_fixes(caps, &rows, 3));

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
