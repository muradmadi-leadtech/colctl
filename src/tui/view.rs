//! Drawing a `Model` (design spec § Layout, § Components). Pure: the same model always draws the
//! same frame. The hit-testing helpers (`tab_at`, `key_at`, `row_at`) share the layout code with
//! drawing, so a click lands where the eye sees.

use super::app::{Model, Overlay, Tab};
use super::keys::{self, Action};
use super::tabs;
use super::theme::{Theme, Token};
use crate::ui::render::width as text_width;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

/// The spinner's frames, 80 ms each.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A tab's body: lines above the list, the list, and what to say when the list is empty.
#[derive(Debug, Default)]
pub struct Body {
    pub header: Vec<Line<'static>>,
    pub rows: Vec<Line<'static>>,
    pub empty: Option<Line<'static>>,
}

/// The rows the current tab lists.
pub fn rows(m: &Model) -> Vec<Line<'static>> {
    tabs::body(m, theme(m)).rows
}

pub fn theme(m: &Model) -> Theme {
    Theme {
        color: m.opts.color,
    }
}

fn area(m: &Model) -> Rect {
    Rect::new(0, 0, m.width, m.height)
}

/// The rows of the screen: tab bar, rule, body, status (one or more lines), key bar.
struct Regions {
    tabs: Rect,
    rule: Rect,
    body: Rect,
    status: Rect,
    keys: Rect,
}

fn regions(m: &Model) -> Regions {
    let a = area(m);
    let status_h = status_lines(m).len().max(1) as u16;
    let body_h = a.height.saturating_sub(3 + status_h);
    Regions {
        tabs: Rect::new(0, 0, a.width, 1),
        rule: Rect::new(0, 1, a.width, 1),
        body: Rect::new(1, 2, a.width.saturating_sub(2), body_h),
        status: Rect::new(1, 2 + body_h, a.width.saturating_sub(2), status_h),
        keys: Rect::new(1, a.height.saturating_sub(1), a.width.saturating_sub(2), 1),
    }
}

/// Each tab's label and where it sits on the tab bar.
fn tab_spans(m: &Model) -> Vec<(Tab, String, u16, u16)> {
    let mut x = 1u16;
    Tab::ALL
        .iter()
        .map(|t| {
            let label = tab_label(m, *t);
            let w = text_width(&label) as u16;
            let span = (*t, label, x, x + w);
            x += w + 2;
            span
        })
        .collect()
}

fn tab_label(m: &Model, t: Tab) -> String {
    match t {
        Tab::Review if m.held_count() > 0 => format!("Review !{}", m.held_count()),
        Tab::Overview if tabs::needs_you(m) => "Overview !".into(),
        t => t.title().into(),
    }
}

/// The tab under a click on the tab bar.
pub fn tab_at(m: &Model, x: u16, y: u16) -> Option<Tab> {
    (y == 0)
        .then(|| {
            tab_spans(m)
                .into_iter()
                .find(|(_, _, a, b)| x >= *a && x < *b)
                .map(|(t, ..)| t)
        })
        .flatten()
}

/// The key bar's entries and where they sit.
fn key_spans(m: &Model) -> Vec<(Action, String, u16, u16)> {
    let r = regions(m);
    let all = keys::bar(&m.scopes());
    let mut out = Vec::new();
    let mut x = r.keys.x;
    let max = r.keys.x + r.keys.width;
    // Shrinks from the left: drop the least useful (the end, before ? and q) until it fits.
    let mut chosen: Vec<&keys::Binding> = all.clone();
    let width = |list: &[&keys::Binding]| -> usize {
        list.iter()
            .map(|b| text_width(b.key) + 1 + b.label.len())
            .sum::<usize>()
            + 3 * list.len().saturating_sub(1)
    };
    while width(&chosen) > usize::from(r.keys.width) && chosen.len() > 2 {
        let drop = chosen
            .iter()
            .rposition(|b| !matches!(b.action, Action::Help | Action::Quit))
            .unwrap_or(0);
        chosen.remove(drop);
    }
    for b in chosen {
        let text = format!("{} {}", b.key, b.label);
        let w = text_width(&text) as u16;
        if x + w > max {
            break;
        }
        out.push((b.action, text, x, x + w));
        x += w + 3;
    }
    out
}

/// The key-bar entry under a click.
pub fn key_at(m: &Model, x: u16, y: u16) -> Option<Action> {
    let r = regions(m);
    (y == r.keys.y)
        .then(|| {
            key_spans(m)
                .into_iter()
                .find(|(_, _, a, b)| x >= *a && x < *b)
                .map(|(a, ..)| a)
        })
        .flatten()
}

/// Where the current tab's list starts and how many rows it shows.
fn list_area(m: &Model) -> Rect {
    let r = regions(m).body;
    let header = tabs::body(m, theme(m)).header.len() as u16;
    let top = r.y + header.min(r.height);
    Rect::new(r.x, top, r.width, (r.y + r.height).saturating_sub(top))
}

/// The list row under a click.
pub fn row_at(m: &Model, x: u16, y: u16) -> Option<usize> {
    let a = list_area(m);
    if x < a.x || x >= a.x + a.width || y < a.y || y >= a.y + a.height {
        return None;
    }
    let row = m.cursor().offset + usize::from(y - a.y);
    (row < m.list_len()).then_some(row)
}

/// The overlay's rectangle (design spec: centred, 80% wide up to 100 columns).
fn overlay_rect(m: &Model, wanted_height: u16) -> Rect {
    let body = regions(m).body;
    let w = (m.width * 4 / 5).clamp(40, 100).min(m.width);
    let h = wanted_height.min(body.height + 2).max(5);
    let x = (m.width - w) / 2;
    let y = body.y.saturating_sub(1) + (body.height + 2).saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

/// True when (x, y) is inside the open overlay.
pub fn in_overlay(m: &Model, x: u16, y: u16) -> bool {
    let r = overlay_rect(m, overlay_height(m));
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn overlay_height(m: &Model) -> u16 {
    match &m.overlay {
        Some(Overlay::Help) => {
            let n = keys::help(&help_scopes(m)).len() as u16;
            let room = regions(m).body.height;
            (if n + 2 > room { n.div_ceil(2) } else { n }) + 2
        }
        Some(Overlay::Confirm { body, .. }) => body.len() as u16 + 4,
        Some(Overlay::ConfirmQuit) => 5,
        Some(Overlay::Detail { lines, .. }) => lines.len() as u16 + 2,
        Some(Overlay::Error { cause, hint, .. }) => {
            3 + u16::from(cause.is_some()) + u16::from(hint.is_some()) + 1
        }
        None => 0,
    }
}

fn help_scopes(m: &Model) -> Vec<keys::Scope> {
    vec![keys::Scope::Tab(m.tab), keys::Scope::Global]
}

/// The status line: a running job's rows, else the toast.
fn status_lines(m: &Model) -> Vec<Line<'static>> {
    let t = theme(m);
    if let Some(run) = &m.job {
        if run.rows.is_empty() {
            return vec![Line::from(vec![
                Span::styled(spinner(m), t.style(Token::Accent)),
                Span::raw(format!(" {}", run.job.command())),
            ])];
        }
        let w = run
            .rows
            .iter()
            .map(|r| text_width(&r.name))
            .max()
            .unwrap_or(0);
        return run
            .rows
            .iter()
            .map(|r| {
                let (mark, tok) = match r.ok {
                    None => (spinner(m), Token::Accent),
                    Some(true) => ("✓".to_string(), Token::Ok),
                    Some(false) => ("✗".to_string(), Token::Failed),
                };
                Line::from(vec![
                    Span::styled(mark, t.style(tok)),
                    Span::raw(" "),
                    Span::styled(pad(&r.name, w), t.style(Token::Name)),
                    Span::raw("  "),
                    Span::styled(r.text.clone(), t.style(Token::Muted)),
                ])
            })
            .collect();
    }
    match &m.toast {
        Some(toast) => vec![styled_verdict(t, &toast.text)],
        None => vec![Line::default()],
    }
}

fn spinner(m: &Model) -> String {
    if !m.opts.motion {
        return "…".into();
    }
    SPINNER[((m.now / 80) % SPINNER.len() as u64) as usize].to_string()
}

pub fn pad(text: &str, to: usize) -> String {
    format!("{text}{}", " ".repeat(to.saturating_sub(text_width(text))))
}

/// A CLI line that starts with a state symbol, coloured by it.
pub fn styled_verdict(t: Theme, text: &str) -> Line<'static> {
    let tok = match text.trim_start().chars().next() {
        Some('✓') => Token::Ok,
        Some('!') => Token::Attention,
        Some('✗') => Token::Failed,
        Some('·') => Token::Info,
        _ => Token::Plain,
    };
    Line::styled(text.to_string(), t.style(tok))
}

/// Draws the whole screen.
pub fn draw(m: &Model, f: &mut Frame) {
    if !m.quit {
        return;
    }
    let t = theme(m);
    let a = area(m);
    if m.too_small() {
        let text = format!(
            "Make the window at least {}×{} (now {}×{})",
            super::app::MIN_WIDTH,
            super::app::MIN_HEIGHT,
            m.width,
            m.height
        );
        let y = a.height / 2;
        f.render_widget(
            Paragraph::new(Line::styled(text, t.style(Token::Attention))).centered(),
            Rect::new(0, y, a.width, 1),
        );
        return;
    }
    let r = regions(m);

    // Tab bar.
    let mut spans = vec![Span::raw(" ")];
    for (tab, label, ..) in tab_spans(m) {
        let style = if tab == m.tab {
            t.style(Token::Accent)
                .add_modifier(ratatui::style::Modifier::UNDERLINED)
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            Style::new()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), r.tabs);
    let version = format!("colctl {} ", env!("CARGO_PKG_VERSION"));
    let vw = text_width(&version) as u16;
    if r.tabs.width > vw {
        f.render_widget(
            Paragraph::new(Span::styled(version, t.style(Token::Muted))),
            Rect::new(r.tabs.width - vw, 0, vw, 1),
        );
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(usize::from(r.rule.width)),
            t.style(Token::Muted),
        )),
        r.rule,
    );

    // Body.
    let body = tabs::body(m, t);
    let header_h = (body.header.len() as u16).min(r.body.height);
    f.render_widget(
        Paragraph::new(body.header.clone()),
        Rect::new(r.body.x, r.body.y, r.body.width, header_h),
    );
    let list = list_area(m);
    if body.rows.is_empty() {
        if let Some(empty) = body.empty {
            let y = list.y + list.height / 3;
            if list.height > 0 {
                f.render_widget(
                    Paragraph::new(empty).centered(),
                    Rect::new(list.x, y, list.width, 1),
                );
            }
        }
    } else {
        let c = m.cursor();
        let shown: Vec<Line> = body
            .rows
            .into_iter()
            .enumerate()
            .skip(c.offset)
            .take(usize::from(list.height))
            .map(|(i, line)| {
                let focused = i == c.selected && m.overlay.is_none();
                select(t, line, focused, usize::from(list.width))
            })
            .collect();
        let total = m.list_len();
        f.render_widget(Paragraph::new(shown), list);
        if total > usize::from(list.height) && list.height > 0 {
            let pos = format!("{}/{}", c.selected + 1, total);
            let w = text_width(&pos) as u16;
            f.render_widget(
                Paragraph::new(Span::styled(pos, t.style(Token::Muted))),
                Rect::new(
                    list.x + list.width.saturating_sub(w),
                    list.y + list.height - 1,
                    w,
                    1,
                ),
            );
        }
    }

    // Status and keys.
    f.render_widget(Paragraph::new(status_lines(m)), r.status);
    let mut keyline = Vec::new();
    for (i, (_, text, ..)) in key_spans(m).into_iter().enumerate() {
        if i > 0 {
            keyline.push(Span::styled(" · ", t.style(Token::Muted)));
        }
        let (k, label) = text.split_once(' ').unwrap_or((&text, ""));
        keyline.push(Span::styled(k.to_string(), t.style(Token::Accent)));
        keyline.push(Span::raw(format!(" {label}")));
    }
    f.render_widget(Paragraph::new(Line::from(keyline)), r.keys);

    if let Some(o) = &m.overlay {
        overlay(m, f, o);
    }
}

/// A row, reversed when focused (and marked with › so it shows without colour).
fn select(t: Theme, line: Line<'static>, focused: bool, width: usize) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(if focused { "› " } else { "  " }));
    spans.extend(line.spans);
    if !focused {
        return Line::from(spans);
    }
    let used: usize = spans.iter().map(|s| text_width(&s.content)).sum();
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    Line::from(spans).patch_style(t.style(Token::Selection))
}

fn overlay(m: &Model, f: &mut Frame, o: &Overlay) {
    let t = theme(m);
    let rect = overlay_rect(m, overlay_height(m));
    let (title, lines, footer): (String, Vec<Line>, String) = match o {
        Overlay::Help => {
            let all = keys::help(&help_scopes(m));
            let inner = usize::from(rect.height.saturating_sub(2));
            let cell = |(k, label): &(String, &str)| {
                vec![
                    Span::styled(pad(k, 10), t.style(Token::Accent)),
                    Span::raw(pad(label, 22)),
                ]
            };
            // Two columns when one doesn't fit.
            let rows = if all.len() > inner {
                all.len().div_ceil(2)
            } else {
                all.len()
            };
            let lines = (0..rows)
                .map(|i| {
                    let mut spans = cell(&all[i]);
                    if let Some(right) = all.get(i + rows).filter(|_| rows < all.len()) {
                        spans.push(Span::raw("  "));
                        spans.extend(cell(right));
                    }
                    Line::from(spans)
                })
                .collect();
            ("Keys".into(), lines, "esc close".into())
        }
        Overlay::Confirm { title, body, .. } => (
            title.clone(),
            body.iter().map(|l| Line::raw(l.clone())).collect(),
            "y yes · n no".into(),
        ),
        Overlay::ConfirmQuit => (
            "Quit".into(),
            vec![Line::raw(format!(
                "{} is running. Quit when it finishes?",
                m.job
                    .as_ref()
                    .map_or("A job".to_string(), |j| j.job.command())
            ))],
            "y yes · n no".into(),
        ),
        Overlay::Detail {
            title,
            lines,
            scroll,
            ..
        } => {
            let inner = usize::from(rect.height.saturating_sub(2));
            let shown: Vec<Line> = lines
                .iter()
                .skip(*scroll)
                .take(inner)
                .map(|l| detail_line(t, l))
                .collect();
            let pos = if lines.len() > inner {
                format!(" {}/{} ", scroll + 1, lines.len())
            } else {
                String::new()
            };
            let keys: Vec<String> = keys::bar(&m.scopes())
                .iter()
                .filter(|b| b.scope == keys::Scope::Detail)
                .filter(|b| {
                    m.tab == Tab::Review || !matches!(b.action, Action::Approve | Action::Reject)
                })
                .map(|b| format!("{} {}", b.key, b.label))
                .collect();
            (title.clone(), shown, format!("{}{pos}", keys.join(" · ")))
        }
        Overlay::Error { what, cause, hint } => {
            let mut l = vec![Line::styled(
                format!("error: {what}"),
                t.style(Token::Failed),
            )];
            if let Some(c) = cause {
                l.push(Line::raw(format!("  {c}")));
            }
            if let Some(h) = hint {
                l.push(Line::styled(format!("hint: {h}"), t.style(Token::Muted)));
            }
            ("Error".into(), l, "esc close".into())
        }
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.style(Token::Muted))
        .title(Line::from(Span::styled(
            format!(" {title} "),
            t.style(Token::Name),
        )))
        .title_bottom(Line::from(Span::styled(
            format!(" {footer} "),
            t.style(Token::Muted),
        )));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(Paragraph::new(lines), inner);
}

/// A line of CLI text in a detail view: diffs in added/removed, verdicts by their symbol.
fn detail_line(t: Theme, l: &str) -> Line<'static> {
    let s = l.trim_start();
    if s.starts_with('+') && !s.starts_with("+++") {
        Line::styled(l.to_string(), t.style(Token::Added))
    } else if s.starts_with('-') && !s.starts_with("---") {
        Line::styled(l.to_string(), t.style(Token::Removed))
    } else {
        styled_verdict(t, l)
    }
}

/// Every line of the screen as text (tests, and the too-small check).
pub fn text(buffer: &ratatui::buffer::Buffer) -> String {
    let a = buffer.area;
    let mut out = String::new();
    for y in 0..a.height {
        let mut line = String::new();
        let mut skip = 0;
        for x in 0..a.width {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let sym = buffer[(x, y)].symbol();
            skip = text_width(sym).saturating_sub(1);
            line.push_str(sym);
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}
