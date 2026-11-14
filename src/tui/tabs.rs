//! What each tab shows and does (design spec § Screens). Every fact comes from `core`, every action
//! is a `core::run::Job` the CLI also runs.

use super::app::{self, Cmd, Data, Load, Model, Overlay, Tab};
use super::keys::Action;
use super::theme::{Theme, Token, symbol};
use super::view::{Body, pad};
use crate::core::run::Job;
use crate::core::status::{Attention, Profile, Snapshot, Tone};
use crate::error::CliError;
use crate::review::escape;
use crate::ui::render::{cut, cut_alike, width};
use ratatui::text::{Line, Span};

/// The current tab's body.
pub fn body(m: &Model, t: Theme) -> Body {
    match m.tab {
        Tab::Overview => overview(m, t),
        Tab::Profiles => profiles(m, t),
        Tab::Items => items(m, t),
        tab => Body {
            header: vec![Line::styled(tab.title().to_string(), t.style(Token::Name))],
            rows: Vec::new(),
            empty: None,
        },
    }
}

/// True when Overview has something that needs the person (its tab badge).
pub fn needs_you(m: &Model) -> bool {
    m.snapshot
        .ready()
        .is_some_and(|s| !s.attention.is_empty() || s.profiles.iter().any(|p| !p.usable))
}

/// A tab's own actions.
pub fn act(m: &mut Model, action: Action) -> Vec<Cmd> {
    match (m.tab, action) {
        (Tab::Overview, Action::Sync) => ask_sync(m, None),
        (Tab::Overview, Action::Open) => match focused_fix(m) {
            Some(fix) => run_fix(m, &fix),
            None => Vec::new(),
        },
        (Tab::Profiles, Action::Sync) => match focused_profile(m) {
            Some(p) => ask_sync(m, Some(p.name.clone())),
            None => Vec::new(),
        },
        (Tab::Profiles, Action::Open) => {
            if let Some(p) = focused_profile(m) {
                let lines = profile_detail(p);
                m.overlay = Some(Overlay::Detail {
                    title: p.name.clone(),
                    lines,
                    scroll: 0,
                    held: None,
                });
            }
            Vec::new()
        }
        (Tab::Items, Action::Open) => {
            if let Some(item) = focused_item(m) {
                let (title, lines) = item_detail(m, &item);
                m.overlay = Some(Overlay::Detail {
                    title,
                    lines,
                    scroll: 0,
                    held: None,
                });
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

// ── Shared ──────────────────────────────────────────────────────────────────────────────────

/// The verdict line, as `colctl status` prints it.
fn verdict(t: Theme, s: &Snapshot) -> Line<'static> {
    let v = &s.verdict;
    let mut spans = vec![Span::styled(
        format!("{} {}", symbol(v.tone), v.text),
        t.style(v.tone.into()),
    )];
    for f in &v.facts {
        spans.push(Span::styled(format!(" · {f}"), t.style(Token::Muted)));
    }
    Line::from(spans)
}

/// What to show when the snapshot isn't there (EC8-01, EC8-02).
fn not_ready(t: Theme, d: &Data<Snapshot>) -> Vec<Line<'static>> {
    match d {
        Data::Loading => vec![Line::styled("Loading…", t.style(Token::Muted))],
        Data::Failed(e) if e.message.starts_with("no colctl source") => vec![
            Line::styled("! colctl isn't set up yet", t.style(Token::Attention)),
            Line::default(),
            Line::from(vec![
                Span::raw("Quit and run "),
                Span::styled("colctl init", t.style(Token::Accent)),
                Span::raw("."),
            ]),
        ],
        Data::Failed(e) => error_lines(t, e),
        Data::Ready(_) => Vec::new(),
    }
}

fn error_lines(t: Theme, e: &CliError) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut parts = e.message.splitn(2, '\n');
    out.push(Line::styled(
        format!("✗ {}", escape(parts.next().unwrap_or_default())),
        t.style(Token::Failed),
    ));
    if let Some(cause) = parts.next() {
        out.push(Line::raw(format!("  {}", escape(cause.trim()))));
    }
    if let Some(h) = &e.hint {
        out.push(Line::styled(format!("hint: {h}"), t.style(Token::Muted)));
    }
    out.push(Line::default());
    out.push(Line::from(vec![
        Span::styled("r", t.style(Token::Accent)),
        Span::raw(" tries again"),
    ]));
    out
}

fn profile_paths(profiles: &[&Profile], room: usize) -> Vec<String> {
    let paths: Vec<String> = profiles.iter().map(|p| escape(&p.display_path)).collect();
    if paths.iter().map(|p| width(p)).max().unwrap_or(0) <= room {
        paths
    } else {
        cut_alike(&paths, room.max(12))
    }
}

/// One line per profile: mark, name, path, what it has (as `colctl status`).
fn profile_lines(t: Theme, profiles: &[&Profile], cols: usize) -> Vec<Line<'static>> {
    let name_w = profiles.iter().map(|p| width(&p.name)).max().unwrap_or(0);
    let path_room = (cols / 3).max(12);
    let paths = profile_paths(profiles, path_room);
    let path_w = paths.iter().map(|p| width(p)).max().unwrap_or(0);
    let room = cols.saturating_sub(2 + name_w + 2 + path_w + 2).max(10);
    profiles
        .iter()
        .zip(paths)
        .map(|(p, path)| {
            let mut spans = vec![
                Span::styled(symbol(p.mark).to_string(), t.style(p.mark.into())),
                Span::raw(" "),
                Span::styled(pad(&escape(&p.name), name_w), t.style(Token::Name)),
                Span::raw("  "),
                Span::styled(pad(&path, path_w), t.style(Token::Muted)),
                Span::raw("  "),
            ];
            let mut used = 0;
            let parts: Vec<(String, Token)> = match &p.problem {
                Some((state, hint)) => vec![
                    (state.clone(), Token::Failed),
                    (format!("→ {hint}"), Token::Accent),
                ],
                None => p
                    .contents
                    .iter()
                    .map(|(text, tone)| {
                        let tok = match tone {
                            None => Token::Plain,
                            Some(Tone::Info) => Token::Muted,
                            Some(tone) => (*tone).into(),
                        };
                        (escape(text), tok)
                    })
                    .collect(),
            };
            // When it doesn't all fit, plain counts go first; problems stay.
            let mut parts = parts;
            let total = |parts: &[(String, Token)]| {
                parts.iter().map(|(t, _)| width(t)).sum::<usize>()
                    + 3 * parts.len().saturating_sub(1)
            };
            while total(&parts) > room {
                match parts
                    .iter()
                    .rposition(|(_, tok)| matches!(tok, Token::Plain | Token::Muted))
                {
                    Some(i) => {
                        parts.remove(i);
                    }
                    None => break,
                }
            }
            for (i, (text, tok)) in parts.into_iter().enumerate() {
                let sep = if i == 0 { 0 } else { 3 };
                if used + sep + width(&text) > room {
                    if used + sep < room {
                        spans.push(Span::styled(" …", t.style(Token::Muted)));
                    }
                    break;
                }
                if i > 0 {
                    spans.push(Span::styled(" · ", t.style(Token::Muted)));
                }
                used += sep + width(&text);
                spans.push(Span::styled(text, t.style(tok)));
            }
            Line::from(spans)
        })
        .collect()
}

fn ask_sync(m: &mut Model, profile: Option<String>) -> Vec<Cmd> {
    let names: Vec<String> = m.snapshot.ready().map_or_else(Vec::new, |s| {
        s.profiles
            .iter()
            .filter(|p| p.usable && profile.as_ref().is_none_or(|n| *n == p.name))
            .map(|p| p.name.clone())
            .collect()
    });
    let (title, what) = match &profile {
        Some(p) => (format!("Sync {p}"), format!("Make {p} match ~/.colctl.")),
        None => (
            "Sync".to_string(),
            "Make every profile match ~/.colctl.".to_string(),
        ),
    };
    let body = vec![
        what,
        String::new(),
        format!("Profiles: {}", names.join(", ")),
    ];
    app::ask(m, title, body, Job::Sync(profile))
}

/// Runs a fix shown on screen (AC8-14): reading ones open their view, writing ones ask first.
fn run_fix(m: &mut Model, fix: &str) -> Vec<Cmd> {
    let words: Vec<&str> = fix.split_whitespace().collect();
    match words.as_slice() {
        ["colctl", "review", name] => {
            m.tab = Tab::Review;
            m.tab_since = m.now;
            vec![Cmd::Load(Load::Review((*name).to_string()))]
        }
        ["colctl", "sync"] => ask_sync(m, None),
        ["colctl", "update"] => app::ask(
            m,
            "Check for updates",
            vec!["Fetch every source plugin and hold what's new for review.".into()],
            Job::Update(Vec::new()),
        ),
        _ => {
            m.toast = Some(app::Toast {
                text: format!("· Run {fix} in a shell"),
                until: Some(m.now + app::TOAST_MS),
            });
            Vec::new()
        }
    }
}

// ── Overview (AC8-13, AC8-14) ───────────────────────────────────────────────────────────────

fn overview(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let Some(s) = m.snapshot.ready() else {
        return Body {
            header: not_ready(t, &m.snapshot),
            ..Body::default()
        };
    };
    let mut header = vec![verdict(t, s)];
    if s.profiles.is_empty() {
        header.push(Line::default());
        header.push(Line::from(vec![
            Span::raw("Quit and run "),
            Span::styled("colctl init", t.style(Token::Accent)),
            Span::raw(" to find your profiles."),
        ]));
        return Body {
            header,
            ..Body::default()
        };
    }
    header.push(Line::default());
    header.push(Line::styled("Profiles", t.style(Token::Name)));
    let all: Vec<&Profile> = s.profiles.iter().collect();
    for l in profile_lines(t, &all, cols.saturating_sub(2)) {
        let mut spans = vec![Span::raw("  ")];
        spans.extend(l.spans);
        header.push(Line::from(spans));
    }
    if s.attention.is_empty() {
        return Body {
            header,
            ..Body::default()
        };
    }
    header.push(Line::default());
    header.push(Line::styled("Needs you", t.style(Token::Name)));
    Body {
        header,
        rows: attention_rows(t, &s.attention, cols.saturating_sub(2)),
        empty: None,
    }
}

fn attention_rows(t: Theme, rows: &[Attention], cols: usize) -> Vec<Line<'static>> {
    let place_w = rows
        .iter()
        .map(|a| width(&a.place))
        .max()
        .unwrap_or(0)
        .min(16);
    let kind_w = rows.iter().map(|a| width(&a.kind)).max().unwrap_or(0);
    let name_w = rows
        .iter()
        .map(|a| width(&a.name))
        .max()
        .unwrap_or(0)
        .min(24);
    let fix_w = rows
        .iter()
        .map(|a| width(&a.fix) + 2)
        .max()
        .unwrap_or(0)
        .min(cols / 2);
    let what_room = cols
        .saturating_sub(2 + place_w + 2 + kind_w + 2 + name_w + 2 + 2 + fix_w)
        .max(8)
        .min(
            rows.iter()
                .map(|a| width(&escape(&a.what)))
                .max()
                .unwrap_or(0),
        );
    rows.iter()
        .map(|a| {
            let what = cut(&escape(&a.what), what_room);
            let mut spans = vec![
                Span::styled("!", t.style(Token::Attention)),
                Span::raw(" "),
                Span::styled(
                    pad(&cut(&escape(&a.place), place_w), place_w),
                    t.style(Token::Name),
                ),
                Span::raw("  "),
                Span::raw(pad(&a.kind, kind_w)),
                Span::raw("  "),
                Span::raw(pad(&cut(&escape(&a.name), name_w), name_w)),
                Span::raw("  "),
                Span::raw(pad(&what, what_room)),
            ];
            if !a.fix.is_empty() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    cut(&format!("→ {}", escape(&a.fix)), fix_w),
                    t.style(Token::Accent),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn focused_fix(m: &Model) -> Option<String> {
    let s = m.snapshot.ready()?;
    let a = s.attention.get(m.cursor().selected)?;
    (!a.fix.is_empty() && a.fix.starts_with("colctl ")).then(|| a.fix.clone())
}

// ── Profiles (AC8-16) ───────────────────────────────────────────────────────────────────────

fn filtered_profiles(m: &Model) -> Vec<&Profile> {
    let f = m.filters[1].text.to_lowercase();
    m.snapshot.ready().map_or_else(Vec::new, |s| {
        s.profiles
            .iter()
            .filter(|p| f.is_empty() || p.name.to_lowercase().contains(&f))
            .collect()
    })
}

fn focused_profile(m: &Model) -> Option<&Profile> {
    filtered_profiles(m).get(m.cursor().selected).copied()
}

fn filter_line(t: Theme, m: &Model, what: &str) -> Option<Line<'static>> {
    let f = m.filter();
    (f.focused || !f.text.is_empty()).then(|| {
        let mut spans = vec![
            Span::styled("› ", t.style(Token::Accent)),
            Span::raw(f.text.clone()),
        ];
        if f.focused {
            spans.push(Span::styled("▏", t.style(Token::Accent)));
        } else {
            spans.push(Span::styled(
                format!("   filtering {what}"),
                t.style(Token::Muted),
            ));
        }
        Line::from(spans)
    })
}

fn profiles(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let Some(s) = m.snapshot.ready() else {
        return Body {
            header: not_ready(t, &m.snapshot),
            ..Body::default()
        };
    };
    let mut header = vec![verdict(t, s), Line::default()];
    if let Some(l) = filter_line(t, m, "profiles") {
        header.push(l);
        header.push(Line::default());
    }
    let shown = filtered_profiles(m);
    let rows = profile_lines(t, &shown, cols.saturating_sub(2));
    let empty = Some(if s.profiles.is_empty() {
        Line::from(vec![
            Span::raw("No profiles registered. Quit and run "),
            Span::styled("colctl init", t.style(Token::Accent)),
        ])
    } else {
        Line::styled(
            format!("No profiles match \"{}\"", escape(&m.filter().text)),
            t.style(Token::Muted),
        )
    });
    Body {
        header,
        rows,
        empty,
    }
}

/// A profile in full, as `colctl status --harness <name> --all` lists it.
fn profile_detail(p: &Profile) -> Vec<String> {
    let mut out = vec![
        format!("{}  {}", p.kind.as_str(), escape(&p.display_path)),
        String::new(),
    ];
    if let Some((state, hint)) = &p.problem {
        out.push(format!("✗ {state}  → {hint}"));
        return out;
    }
    if p.items.is_empty() {
        out.push("· 0 items".into());
        return out;
    }
    let kind_w = p.items.iter().map(|i| width(&i.kind)).max().unwrap_or(0);
    let state_w = p.items.iter().map(|i| width(&i.state)).max().unwrap_or(0);
    let name_w = p
        .items
        .iter()
        .map(|i| width(&escape(&i.name)))
        .max()
        .unwrap_or(0);
    for i in &p.items {
        let mut line = format!(
            "{} {}  {}  {}",
            symbol(i.tone),
            pad(&i.state, state_w),
            pad(&i.kind, kind_w),
            pad(&escape(&i.name), name_w)
        );
        if let Some(h) = &i.hint {
            line.push_str(&format!("  → {}", escape(h)));
        }
        out.push(line.trim_end().to_string());
    }
    out
}

// ── Items (AC8-17) ──────────────────────────────────────────────────────────────────────────

/// Kinds in the order Items lists them.
const KINDS: [&str; 5] = ["instructions", "skill", "agent", "plugin", "source"];

/// One item across every profile that has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAcross {
    pub kind: String,
    pub name: String,
    /// Per profile, in profile order: its state there, or `None` when that profile doesn't have it.
    pub states: Vec<Option<Tone>>,
}

impl ItemAcross {
    fn needs_you(&self) -> bool {
        self.states
            .iter()
            .any(|s| matches!(s, Some(Tone::Attention | Tone::Failed)))
    }
}

/// Every item, by kind, problems first, then by name; filtered by the Items filter.
pub fn items_across(m: &Model) -> Vec<ItemAcross> {
    let Some(s) = m.snapshot.ready() else {
        return Vec::new();
    };
    let f = m.filters[2].text.to_lowercase();
    let mut out: Vec<ItemAcross> = Vec::new();
    for (pi, p) in s.profiles.iter().enumerate() {
        for i in p.items.iter().filter(|i| KINDS.contains(&i.kind.as_str())) {
            if !f.is_empty() && !i.name.to_lowercase().contains(&f) {
                continue;
            }
            let at = match out
                .iter()
                .position(|a| a.kind == i.kind && a.name == i.name)
            {
                Some(at) => at,
                None => {
                    out.push(ItemAcross {
                        kind: i.kind.clone(),
                        name: i.name.clone(),
                        states: vec![None; s.profiles.len()],
                    });
                    out.len() - 1
                }
            };
            out[at].states[pi] = Some(i.tone);
        }
    }
    let rank = |k: &str| KINDS.iter().position(|x| *x == k).unwrap_or(KINDS.len());
    out.sort_by(|a, b| {
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then(b.needs_you().cmp(&a.needs_you()))
            .then(a.name.cmp(&b.name))
    });
    out
}

fn focused_item(m: &Model) -> Option<ItemAcross> {
    items_across(m).get(m.cursor().selected).cloned()
}

fn items(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let Some(s) = m.snapshot.ready() else {
        return Body {
            header: not_ready(t, &m.snapshot),
            ..Body::default()
        };
    };
    let all = items_across(m);
    let mut header = Vec::new();
    let total = {
        let mut names: Vec<(&str, &str)> = s
            .profiles
            .iter()
            .flat_map(|p| p.items.iter())
            .filter(|i| KINDS.contains(&i.kind.as_str()))
            .map(|i| (i.kind.as_str(), i.name.as_str()))
            .collect();
        names.sort();
        names.dedup();
        names.len()
    };
    let problems = all.iter().filter(|a| a.needs_you()).count();
    let mut lead = vec![Span::raw(format!(
        "{total} item{} across {} profile{}",
        if total == 1 { "" } else { "s" },
        s.profiles.len(),
        if s.profiles.len() == 1 { "" } else { "s" }
    ))];
    if problems > 0 {
        lead.push(Span::styled(
            format!(
                " · {problems} need{} you",
                if problems == 1 { "s" } else { "" }
            ),
            t.style(Token::Attention),
        ));
    }
    header.push(Line::from(lead));
    // The legend: profile numbers.
    let legend: Vec<String> = s
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{} {}", i + 1, escape(&p.name)))
        .collect();
    header.push(Line::styled(
        cut(&legend.join(" · "), cols),
        t.style(Token::Muted),
    ));
    header.push(Line::default());
    if let Some(l) = filter_line(t, m, "items") {
        header.push(l);
        header.push(Line::default());
    }
    let n = s.profiles.len();
    let marks_w = 2 * n;
    let kind_w = all.iter().map(|a| width(&a.kind)).max().unwrap_or(4);
    let name_room = cols.saturating_sub(2 + kind_w + 2 + 2 + marks_w).max(8);
    let name_w = all
        .iter()
        .map(|a| width(&escape(&a.name)))
        .max()
        .unwrap_or(0)
        .min(name_room);
    if !all.is_empty() {
        let numbers: String = (1..=n).map(|i| format!("{:<2}", i % 10)).collect();
        header.push(Line::styled(
            format!(
                "  {}  {}  {}",
                pad("kind", kind_w),
                pad("name", name_w),
                numbers.trim_end()
            ),
            t.style(Token::Muted),
        ));
    }
    let rows = all
        .iter()
        .map(|a| {
            let mut spans = vec![
                Span::styled(pad(&a.kind, kind_w), t.style(Token::Muted)),
                Span::raw("  "),
                Span::styled(
                    pad(&cut(&escape(&a.name), name_w), name_w),
                    t.style(Token::Name),
                ),
                Span::raw("  "),
            ];
            for st in &a.states {
                match st {
                    Some(tone) => spans.push(Span::styled(
                        symbol(*tone).to_string(),
                        t.style((*tone).into()),
                    )),
                    None => spans.push(Span::styled("–", t.style(Token::Muted))),
                }
                spans.push(Span::raw(" "));
            }
            Line::from(spans)
        })
        .collect();
    let empty = Some(if m.filter().text.is_empty() {
        Line::styled("No items yet", t.style(Token::Muted))
    } else {
        Line::styled(
            format!("No items match \"{}\"", escape(&m.filter().text)),
            t.style(Token::Muted),
        )
    });
    Body {
        header,
        rows,
        empty,
    }
}

/// One item in full: each profile's state, where it lives there, and its fix.
fn item_detail(m: &Model, a: &ItemAcross) -> (String, Vec<String>) {
    let title = format!("{} {}", a.kind, escape(&a.name));
    let Some(s) = m.snapshot.ready() else {
        return (title, Vec::new());
    };
    let name_w = s.profiles.iter().map(|p| width(&p.name)).max().unwrap_or(0);
    let mut lines = Vec::new();
    let mut missing = Vec::new();
    for p in &s.profiles {
        match p
            .items
            .iter()
            .find(|i| i.kind == a.kind && i.name == a.name)
        {
            Some(i) => {
                let mut line = format!(
                    "{} {}  {}  {}",
                    symbol(i.tone),
                    pad(&escape(&p.name), name_w),
                    i.state,
                    escape(&i.address)
                );
                if let Some(h) = &i.hint {
                    line.push_str(&format!("  → {}", escape(h)));
                }
                lines.push(line);
            }
            None => missing.push(escape(&p.name)),
        }
    }
    if !missing.is_empty() {
        lines.push(String::new());
        lines.push(format!("· Not in {}", missing.join(", ")));
    }
    (title, lines)
}
