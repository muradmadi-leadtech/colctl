//! `colctl status` as text (cycle 6, interfaces.md § status): a verdict, one line per profile with
//! what it has, then only what needs attention, each with its fix. `--all` lists every item.

use super::quarantine::View;
use super::{Planned, Workspace};
use crate::core::status::{Attention, Profile, Snapshot, Tone};
use crate::ctx::Ctx;
use crate::review::escape;
use crate::ui::render::{self, Cell};
use crate::ui::style::{Token, paint};

/// A core tone as an output token.
pub(super) fn token(t: Tone) -> Token {
    match t {
        Tone::Ok => Token::Ok,
        Tone::Attention => Token::Attention,
        Tone::Failed => Token::Failed,
        Tone::Info => Token::Info,
    }
}

/// A profile's contents as cells: plain counts, dim notes, coloured problems; an unusable profile
/// says why and what to do.
fn cells(p: &Profile) -> Vec<Cell> {
    if let Some((state, hint)) = &p.problem {
        return vec![
            Cell::plain(state.clone()),
            Cell::styled(format!("→ {hint}"), Token::Command),
        ];
    }
    p.contents
        .iter()
        .map(|(text, tone)| match tone {
            None => Cell::plain(text.clone()),
            Some(Tone::Info) => Cell::styled(text.clone(), Token::Dim),
            Some(t) => Cell::styled(text.clone(), token(*t)),
        })
        .collect()
}

/// One line per profile (AC6-34, AC6-35): mark, name, path, then what it has. Paths are cut alike
/// when the column needs it, and what it has wraps under itself rather than being cut.
fn profile_lines(ctx: &Ctx, profiles: &[Profile]) -> Vec<String> {
    let caps = &ctx.out.caps;
    let rows: Vec<(Token, &str, String, Vec<Cell>)> = profiles
        .iter()
        .map(|p| {
            (
                token(p.mark),
                p.name.as_str(),
                p.display_path.clone(),
                cells(p),
            )
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

pub(super) fn row(a: &Attention) -> (Vec<Cell>, String, Token) {
    (
        vec![
            Cell::styled(Token::Attention.symbol(), Token::Attention),
            Cell::styled(escape(&a.place), Token::Name),
            Cell::plain(a.kind.clone()),
            Cell::plain(escape(&a.name)),
            Cell::plain(escape(&a.what)),
        ],
        escape(&a.fix),
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

/// Everything that needs the person, each with its fix (see [`crate::core::status::attention`]).
pub(super) fn attention_rows(
    ctx: &Ctx,
    ws: &Workspace,
    planned: &[Planned],
    uncommitted: &[String],
    held: &[View],
) -> Vec<Attention> {
    crate::core::status::attention(ctx, ws, planned, uncommitted, held)
}

/// The text of `colctl status` (AC6-08 – 11, EC6-10, EC6-11).
pub(super) fn render(ctx: &Ctx, snap: &Snapshot, all: bool) -> Vec<String> {
    let caps = &ctx.out.caps;
    let v = &snap.verdict;
    if snap.profiles.is_empty() {
        return vec![
            render::verdict(caps, token(v.tone), &v.text, &v.facts),
            String::new(),
            render::hint(caps, "colctl init"),
        ];
    }
    let mut lines = vec![render::verdict(caps, token(v.tone), &v.text, &v.facts)];
    if caps.quiet {
        return lines;
    }

    // One line per profile.
    lines.push(String::new());
    lines.extend(profile_lines(ctx, &snap.profiles));

    if !snap.attention.is_empty() {
        lines.push(String::new());
        lines.push(render::heading(caps, "Needs attention"));
        let rows: Vec<(Vec<Cell>, String, Token)> = snap.attention.iter().map(row).collect();
        lines.extend(rows_with_fixes(caps, &rows, 3));
    }

    if all {
        for p in snap.profiles.iter().filter(|p| p.usable) {
            lines.push(String::new());
            let room = caps
                .width
                .saturating_sub(render::width(&p.name) + 2)
                .max(16);
            lines.push(format!(
                "{}  {}",
                render::heading(caps, &p.name),
                render::footer(caps, &render::cut_middle(&p.display_path, room))
            ));
            if p.items.is_empty() {
                lines.push(format!("  {} 0 items", Token::Info.symbol()));
                continue;
            }
            let rows: Vec<(Vec<Cell>, String, Token)> = p
                .items
                .iter()
                .map(|i| {
                    let t = token(i.tone);
                    (
                        vec![
                            Cell::styled(t.symbol(), t),
                            Cell::plain(i.state.clone()),
                            Cell::plain(i.kind.clone()),
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
