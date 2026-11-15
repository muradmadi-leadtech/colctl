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
        Tab::Review => review(m, t),
        Tab::Discover => discover(m, t),
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
    if matches!(m.overlay, Some(Overlay::Pick { .. })) {
        return pick(m, action);
    }
    match (m.tab, action) {
        (Tab::Items, Action::Remove) => match focused_item(m) {
            Some(item) => ask_remove(m, &item),
            None => Vec::new(),
        },
        (Tab::Items, Action::Convert) => match focused_item(m) {
            Some(item) => convert(m, &item),
            None => Vec::new(),
        },
        (Tab::Items, Action::Refresh) => match focused_item(m) {
            Some(item) => refresh(m, &item),
            None => Vec::new(),
        },
        (Tab::Profiles, Action::Adopt) => match focused_profile(m) {
            Some(p) => vec![Cmd::Load(Load::Adopt(p.name.clone()))],
            None => Vec::new(),
        },
        (Tab::Discover, Action::NextView) => next_view(m),
        (Tab::Discover, Action::Open) if m.discover_view == app::DiscoverView::Developers => {
            if let Some(d) = focused_developer(m) {
                m.discover_view = app::DiscoverView::Plugins;
                m.filters[4] = app::Input {
                    text: d.name,
                    focused: false,
                };
                *m.cursor_mut() = app::Cursor::default();
            }
            Vec::new()
        }
        // A connector isn't in any catalog `show` reads: its details are what `connectors` found.
        (Tab::Discover, Action::Open) if m.discover_view == app::DiscoverView::Connectors => {
            if let Some(p) = focused_plugin(m) {
                let mut lines = vec![format!("{}  connector", escape(&p.ids[0]))];
                if let Some(v) = &p.version {
                    lines.push(format!("version  {}", escape(v)));
                }
                if !p.description.is_empty() {
                    lines.push(String::new());
                    lines.push(escape(&p.description));
                }
                if !p.installed.is_empty() {
                    let into: Vec<String> = p.installed.iter().map(|h| escape(h)).collect();
                    lines.push(String::new());
                    lines.push(format!("✓ installed in {}", into.join(", ")));
                }
                lines.push(String::new());
                lines.push(escape(&p.add));
                m.overlay = Some(Overlay::Detail {
                    title: p.name.clone(),
                    lines,
                    scroll: 0,
                    held: None,
                });
            }
            Vec::new()
        }
        (Tab::Discover, Action::Open) if m.discover_view == app::DiscoverView::Marketplaces => {
            if focused_marketplace(m).is_none() {
                return Vec::new();
            }
            app::ask(
                m,
                "Refresh marketplaces",
                vec![
                    "Ask claude and codex to update every marketplace".into(),
                    "your profiles know (uses the network).".into(),
                ],
                Job::MarketplaceRefresh,
            )
        }
        (Tab::Overview, Action::Sync) => ask_sync(m, None),
        (Tab::Overview, Action::Update) => ask_update(m),
        (Tab::Overview, Action::Open) => {
            let entry = overview_entries(m)
                .into_iter()
                .nth(m.cursor().selected)
                .flatten();
            match entry {
                Some(Entry::Menu(item)) => run_menu(m, item),
                Some(Entry::Fix(i)) => match focused_fix(m, i) {
                    Some(fix) => run_fix(m, &fix),
                    None => Vec::new(),
                },
                None => Vec::new(),
            }
        }
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
        (Tab::Review, Action::Open) => {
            if let Some(h) = focused_held(m) {
                let (name, lines) = (h.name.clone(), held_summary(h));
                m.overlay = Some(Overlay::Detail {
                    title: name.clone(),
                    lines,
                    scroll: 0,
                    held: Some(name),
                });
            }
            Vec::new()
        }
        (_, Action::FullReview) => match &m.overlay {
            Some(Overlay::Detail {
                held: Some(name), ..
            }) => vec![Cmd::Load(Load::Review(name.clone()))],
            _ => Vec::new(),
        },
        (Tab::Review, Action::Approve | Action::Reject) => match held_target(m) {
            Some(name) => ask_decision(m, &name, action == Action::Approve),
            None => Vec::new(),
        },
        (Tab::Review, Action::ApproveAll) => {
            let names: Vec<String> = m
                .held
                .ready()
                .map(|h| h.iter().map(|h| h.name.clone()).collect())
                .unwrap_or_default();
            if names.is_empty() {
                return Vec::new();
            }
            let body = vec![
                "Install every held version exactly as reviewed:".to_string(),
                String::new(),
                names.join(", "),
            ];
            app::ask(m, "Approve all", body, Job::Approve(Vec::new()))
        }
        (Tab::Review, Action::Update) => app::ask(
            m,
            "Check for updates",
            vec![
                "Fetch every source plugin and hold what's new for review.".into(),
                "Esc stops after the source it's on.".into(),
            ],
            Job::Update(Vec::new()),
        ),
        (Tab::Discover, Action::Open) => match focused_plugin(m) {
            Some(p) => vec![Cmd::Load(Load::Show(p.ids[0].clone()))],
            None => Vec::new(),
        },
        (Tab::Discover, Action::Approve | Action::Add) => match focused_plugin(m) {
            Some(p) => match crate::core::discover::add_words(&p) {
                Ok(words) => {
                    let body = vec![
                        format!("Runs colctl {}", words.join(" ")),
                        "and syncs every profile.".into(),
                    ];
                    app::ask(m, format!("Add {}", p.name), body, Job::Add(words))
                }
                Err(why) => {
                    m.toast = Some(app::Toast {
                        text: format!("· {}: {}", p.name, why),
                        until: Some(m.now + app::TOAST_MS),
                    });
                    Vec::new()
                }
            },
            None => Vec::new(),
        },
        (_, Action::NextFile | Action::PrevFile) => {
            if let Some(Overlay::Detail { lines, scroll, .. }) = &mut m.overlay {
                let is_file = |l: &String| l.trim_start().starts_with("diff --git ");
                let to = if action == Action::NextFile {
                    lines
                        .iter()
                        .enumerate()
                        .skip(*scroll + 1)
                        .find(|(_, l)| is_file(l))
                        .map(|(i, _)| i)
                } else {
                    lines
                        .iter()
                        .enumerate()
                        .take(*scroll)
                        .rev()
                        .find(|(_, l)| is_file(l))
                        .map(|(i, _)| i)
                };
                if let Some(to) = to {
                    *scroll = to;
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Keys in adopt's checklist: move, tick, adopt what's ticked.
fn pick(m: &mut Model, action: Action) -> Vec<Cmd> {
    let Some(Overlay::Pick {
        profile,
        items,
        selected,
        ..
    }) = &mut m.overlay
    else {
        return Vec::new();
    };
    match action {
        Action::Toggle => {
            if let Some((_, on)) = items.get_mut(*selected) {
                *on = !*on;
            }
            Vec::new()
        }
        Action::Open => {
            let ticked: Vec<String> = items
                .iter()
                .filter(|(_, on)| *on)
                .map(|(c, _)| c.address.clone())
                .collect();
            if ticked.is_empty() {
                m.toast("· Nothing ticked", false);
                return Vec::new();
            }
            let all = ticked.len() == items.len();
            let profile = profile.clone();
            let mut body = vec![format!("Move into ~/.colctl from {profile}, then sync:")];
            body.push(String::new());
            body.extend(ticked.iter().map(|t| format!("  {}", escape(t))));
            let job = Job::Adopt {
                profile: profile.clone(),
                items: if all { Vec::new() } else { ticked },
            };
            m.overlay = None;
            app::ask(m, format!("Adopt from {profile}"), body, job)
        }
        _ => Vec::new(),
    }
}

/// `x` on Items: what `colctl rm` would remove, and from where (AC9-17).
fn ask_remove(m: &mut Model, item: &ItemAcross) -> Vec<Cmd> {
    let Some(s) = m.snapshot.ready() else {
        return Vec::new();
    };
    if item.kind == "instructions" {
        m.toast(
            "· Instructions aren't removed: edit ~/.colctl/AGENTS.md instead",
            false,
        );
        return Vec::new();
    }
    let having: Vec<&Profile> = s
        .profiles
        .iter()
        .zip(&item.states)
        .filter(|(_, st)| st.is_some())
        .map(|(p, _)| p)
        .collect();
    let codex = item.kind == "plugin"
        && !having.is_empty()
        && having.iter().all(|p| p.kind == crate::config::Kind::Codex);
    let names: Vec<String> = having.iter().map(|p| escape(&p.name)).collect();
    let body = vec![
        format!(
            "Remove {} {} from ~/.colctl and every profile:",
            item.kind,
            escape(&item.name)
        ),
        if names.is_empty() {
            "no profile has it yet.".into()
        } else {
            names.join(", ")
        },
    ];
    let job = Job::Rm {
        kind: item.kind.clone(),
        name: item.name.clone(),
        codex,
    };
    app::ask(
        m,
        format!("Remove {} {}", item.kind, escape(&item.name)),
        body,
        job,
    )
}

/// `c` on Items: convert a source for Codex after showing its plan, or undo it (AC9-18).
fn convert(m: &mut Model, item: &ItemAcross) -> Vec<Cmd> {
    if item.kind != "source" {
        m.toast("· Only source plugins convert for Codex", false);
        return Vec::new();
    }
    let converted = m
        .snapshot
        .ready()
        .is_some_and(|s| s.converted.contains(&item.name));
    if converted {
        let name = item.name.clone();
        return app::ask(
            m,
            format!("Stop converting {}", escape(&name)),
            vec![
                format!("Uninstall {} from Codex profiles", escape(&name)),
                "and stop converting it.".into(),
            ],
            Job::ConvertUndo(name),
        );
    }
    vec![Cmd::Load(Load::ConvertPlan(item.name.clone()))]
}

/// `f` on Items: hold what changed where a taken-over item came from (AC9-19).
fn refresh(m: &mut Model, item: &ItemAcross) -> Vec<Cmd> {
    let address = match item.kind.as_str() {
        "skill" => format!("skills/{}", item.name),
        "agent" => format!("agents/{}.md", item.name),
        _ => String::new(),
    };
    let taken = m
        .snapshot
        .ready()
        .is_some_and(|s| s.taken_over.contains(&address));
    if !taken {
        m.toast(
            format!(
                "· {} isn't taken over, so there's nowhere to refresh it from",
                escape(&item.name)
            ),
            false,
        );
        return Vec::new();
    }
    app::begin(m, Job::Refresh(vec![address]))
}

/// Ctrl-V in Discover: the next view, loading its list when it needs one (AC9-21).
fn next_view(m: &mut Model) -> Vec<Cmd> {
    m.discover_view = m.discover_view.next();
    m.filters[4] = app::Input::default();
    *m.cursor_mut() = app::Cursor::default();
    match m.discover_view {
        app::DiscoverView::Marketplaces => vec![Cmd::Load(Load::Marketplaces)],
        app::DiscoverView::Connectors if !matches!(m.connectors, Data::Ready(_)) => {
            vec![Cmd::Load(Load::Connectors)]
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
            if let Some(i) = m
                .held
                .ready()
                .and_then(|h| h.iter().position(|h| h.name == *name))
            {
                m.cursors[3].selected = i;
            }
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

/// The actions Overview offers above everything else (AC8-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    Sync,
    Update,
    Review,
    Find,
    Items,
    Palette,
}

fn menu(m: &Model) -> Vec<(MenuItem, String, &'static str)> {
    let held = m.held_count();
    let review = match held {
        0 => "Review held versions (none now)".to_string(),
        1 => "Review 1 held version".to_string(),
        n => format!("Review {n} held versions"),
    };
    vec![
        (MenuItem::Sync, "Sync every profile".into(), "s"),
        (MenuItem::Update, "Check for updates".into(), "u"),
        (MenuItem::Review, review, "4"),
        (MenuItem::Find, "Find a plugin".into(), "5"),
        (MenuItem::Items, "Look through every item".into(), "3"),
        (MenuItem::Palette, "All actions".into(), ":"),
    ]
}

/// What an Overview row is.
enum Entry {
    Menu(MenuItem),
    Fix(usize),
}

/// Overview's rows: the menu, then the profiles to read, then what needs you.
fn overview_entries(m: &Model) -> Vec<Option<Entry>> {
    let mut out: Vec<Option<Entry>> = menu(m)
        .into_iter()
        .map(|(i, ..)| Some(Entry::Menu(i)))
        .collect();
    if let Some(s) = m.snapshot.ready() {
        if !s.profiles.is_empty() {
            out.push(None);
            out.push(None);
            out.extend(s.profiles.iter().map(|_| None));
        }
        if !s.attention.is_empty() {
            out.push(None);
            out.push(None);
            out.extend((0..s.attention.len()).map(|i| Some(Entry::Fix(i))));
        }
    }
    out
}

fn run_menu(m: &mut Model, item: MenuItem) -> Vec<Cmd> {
    let go = |m: &mut Model, tab: Tab| {
        m.tab = tab;
        m.tab_since = m.now;
        Vec::new()
    };
    match item {
        MenuItem::Sync => ask_sync(m, None),
        MenuItem::Update => ask_update(m),
        MenuItem::Review => go(m, Tab::Review),
        MenuItem::Find => go(m, Tab::Discover),
        MenuItem::Items => go(m, Tab::Items),
        MenuItem::Palette => app::act(m, Action::Palette),
    }
}

fn ask_update(m: &mut Model) -> Vec<Cmd> {
    app::ask(
        m,
        "Check for updates",
        vec![
            "Fetch every source plugin and hold what's new for review.".into(),
            "Esc stops after the source it's on.".into(),
        ],
        Job::Update(Vec::new()),
    )
}

fn overview(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let Some(s) = m.snapshot.ready() else {
        return Body {
            header: not_ready(t, &m.snapshot),
            ..Body::default()
        };
    };
    let header = vec![verdict(t, s), Line::default()];
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut skip = Vec::new();
    let label_w = menu(m).iter().map(|(_, l, _)| width(l)).max().unwrap_or(0) + 4;
    for (_, label, key) in menu(m) {
        rows.push(Line::from(vec![
            Span::styled(pad(&label, label_w), t.style(Token::Name)),
            Span::styled(key.to_string(), t.style(Token::Accent)),
        ]));
    }
    let mut readonly = |rows: &mut Vec<Line<'static>>, line: Line<'static>| {
        skip.push(rows.len());
        rows.push(line);
    };
    if s.profiles.is_empty() {
        readonly(&mut rows, Line::default());
        readonly(
            &mut rows,
            Line::from(vec![
                Span::raw("No profiles yet. Quit and run "),
                Span::styled("colctl init", t.style(Token::Accent)),
                Span::raw(" to find them."),
            ]),
        );
    } else {
        readonly(&mut rows, Line::default());
        readonly(&mut rows, Line::styled("Profiles", t.style(Token::Muted)));
        let all: Vec<&Profile> = s.profiles.iter().collect();
        for l in profile_lines(t, &all, cols.saturating_sub(2)) {
            readonly(&mut rows, l);
        }
    }
    if !s.attention.is_empty() {
        readonly(&mut rows, Line::default());
        readonly(&mut rows, Line::styled("Needs you", t.style(Token::Muted)));
        rows.extend(attention_rows(t, &s.attention, cols.saturating_sub(2)));
    }
    Body {
        header,
        rows,
        empty: None,
        skip,
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

fn focused_fix(m: &Model, i: usize) -> Option<String> {
    let s = m.snapshot.ready()?;
    let a = s.attention.get(i)?;
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
        skip: Vec::new(),
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
        skip: Vec::new(),
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

// ── Review (AC8-18 – 21) ────────────────────────────────────────────────────────────────────

fn focused_held(m: &Model) -> Option<&crate::core::quarantine::HeldSummary> {
    m.held.ready()?.get(m.cursor().selected)
}

/// The held version `a` and `x` act on: the open detail's, else the focused row's.
fn held_target(m: &Model) -> Option<String> {
    match &m.overlay {
        Some(Overlay::Detail {
            held: Some(name), ..
        }) => Some(name.clone()),
        Some(_) => None,
        None => focused_held(m).map(|h| h.name.clone()),
    }
}

fn ask_decision(m: &mut Model, name: &str, approve: bool) -> Vec<Cmd> {
    let change = m
        .held
        .ready()
        .and_then(|h| h.iter().find(|h| h.name == name))
        .map(|h| h.change.clone())
        .unwrap_or_default();
    let (title, body, job) = if approve {
        (
            format!("Approve {name}"),
            vec![
                format!("Install {name} {change} exactly as reviewed,"),
                "in every profile that has it.".into(),
            ],
            Job::Approve(vec![name.to_string()]),
        )
    } else {
        (
            format!("Reject {name}"),
            vec![
                format!("Drop {name} {change} for good."),
                "colctl update won't hold this version again.".into(),
            ],
            Job::Reject(vec![name.to_string()]),
        )
    };
    m.overlay = None;
    app::ask(m, title, body, job)
}

fn review(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let held = match &m.held {
        Data::Ready(h) => h,
        Data::Loading => {
            return Body {
                header: vec![Line::styled("Loading…", t.style(Token::Muted))],
                ..Body::default()
            };
        }
        Data::Failed(e) => {
            return Body {
                header: error_lines(t, e),
                ..Body::default()
            };
        }
    };
    let empty = Some(Line::from(vec![
        Span::raw("Nothing held for review. "),
        Span::styled("u", t.style(Token::Accent)),
        Span::raw(" checks for updates"),
    ]));
    if held.is_empty() {
        return Body {
            header: vec![Line::styled(
                "✓ Nothing held for review",
                t.style(Token::Ok),
            )],
            rows: Vec::new(),
            empty,
            skip: Vec::new(),
        };
    }
    let n = held.len();
    let header = vec![
        Line::styled(format!("! {n} held for review",), t.style(Token::Attention)),
        Line::default(),
    ];
    let name_w = held
        .iter()
        .map(|h| width(&escape(&h.name)))
        .max()
        .unwrap_or(0)
        .min(28);
    let kind_w = held.iter().map(|h| width(&h.kind)).max().unwrap_or(0);
    let size_w = held.iter().map(|h| width(&h.size)).max().unwrap_or(0);
    let change_room = cols
        .saturating_sub(2 + name_w + 2 + kind_w + 2 + 2 + size_w + 2 + 10)
        .max(10);
    let change_w = held
        .iter()
        .map(|h| width(&escape(&h.change)))
        .max()
        .unwrap_or(0)
        .min(change_room);
    let rows = held
        .iter()
        .map(|h| {
            let mut spans = vec![
                Span::styled(
                    pad(&cut(&escape(&h.name), name_w), name_w),
                    t.style(Token::Name),
                ),
                Span::raw("  "),
                Span::styled(pad(&h.kind, kind_w), t.style(Token::Muted)),
                Span::raw("  "),
                Span::raw(pad(&cut(&escape(&h.change), change_w), change_w)),
                Span::raw("  "),
                Span::styled(pad(&h.size, size_w), t.style(Token::Muted)),
            ];
            if let Some(e) = &h.error {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    cut(&format!("✗ {}", escape(e)), cols / 3),
                    t.style(Token::Failed),
                ));
            } else if h.flags > 0 {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("! {} flag{}", h.flags, if h.flags == 1 { "" } else { "s" }),
                    t.style(Token::Attention),
                ));
            }
            Line::from(spans)
        })
        .collect();
    Body {
        header,
        rows,
        empty,
        skip: Vec::new(),
    }
}

// ── Discover (AC8-22, AC8-23) ───────────────────────────────────────────────────────────────

/// The plugins matching the search line, ranked as `colctl search` ranks them, and how many in all.
pub fn results(m: &Model) -> (std::rc::Rc<Vec<crate::search::Plugin>>, usize) {
    let Some(c) = m.catalog.ready() else {
        return (std::rc::Rc::new(Vec::new()), 0);
    };
    let query = m.filters[4].text.clone();
    if let Some(hit) = m.discover_cache.borrow().as_ref()
        && hit.query == query
        && hit.catalog == m.catalog_loads
    {
        let n = hit.results.len();
        return (hit.results.clone(), n);
    }
    let q = crate::search::Query {
        terms: m.filters[4]
            .text
            .split_whitespace()
            .map(String::from)
            .collect(),
        limit: 0,
        ..crate::search::Query::default()
    };
    let (hits, n) = crate::core::discover::rank(&c.plugins, &q);
    let results = std::rc::Rc::new(hits);
    *m.discover_cache.borrow_mut() = Some(app::DiscoverCache {
        query,
        catalog: m.catalog_loads,
        results: results.clone(),
    });
    (results, n)
}

fn focused_plugin(m: &Model) -> Option<crate::search::Plugin> {
    match m.discover_view {
        app::DiscoverView::Plugins => results(m).0.get(m.cursor().selected).cloned(),
        app::DiscoverView::Connectors => connector_results(m).get(m.cursor().selected).cloned(),
        _ => None,
    }
}

/// The developers matching the search line (AC9-21), as `colctl developers` lists them.
fn developer_results(m: &Model) -> Vec<crate::search::Developer> {
    let Some(c) = m.catalog.ready() else {
        return Vec::new();
    };
    let q = m.filters[4].text.trim().to_lowercase();
    crate::core::discover::developers(c)
        .into_iter()
        .filter(|d| q.is_empty() || d.name.to_lowercase().contains(&q))
        .collect()
}

fn focused_developer(m: &Model) -> Option<crate::search::Developer> {
    developer_results(m).get(m.cursor().selected).cloned()
}

fn marketplace_results(m: &Model) -> Vec<crate::catalog::Marketplace> {
    let Some(list) = m.marketplaces.ready() else {
        return Vec::new();
    };
    let q = m.filters[4].text.trim().to_lowercase();
    list.iter()
        .filter(|mk| {
            q.is_empty()
                || mk.name.to_lowercase().contains(&q)
                || mk.tool.contains(&q)
                || mk.source.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

fn focused_marketplace(m: &Model) -> Option<crate::catalog::Marketplace> {
    marketplace_results(m).get(m.cursor().selected).cloned()
}

fn connector_results(m: &Model) -> Vec<crate::search::Plugin> {
    let Some(list) = m.connectors.ready() else {
        return Vec::new();
    };
    let q = crate::search::Query {
        terms: m.filters[4]
            .text
            .split_whitespace()
            .map(String::from)
            .collect(),
        limit: 0,
        ..crate::search::Query::default()
    };
    crate::core::discover::rank(list, &q).0
}

/// Discover's search line and which view it's on.
fn discover_head(m: &Model, t: Theme) -> Vec<Line<'static>> {
    let f = m.filter();
    let mut search = vec![
        Span::styled("› ", t.style(Token::Accent)),
        Span::raw(f.text.clone()),
    ];
    if f.focused {
        search.push(Span::styled("▏", t.style(Token::Accent)));
    } else if f.text.is_empty() {
        search.push(Span::styled("/ to search", t.style(Token::Muted)));
    }
    let mut views = Vec::new();
    for v in [
        app::DiscoverView::Plugins,
        app::DiscoverView::Developers,
        app::DiscoverView::Marketplaces,
        app::DiscoverView::Connectors,
    ] {
        if !views.is_empty() {
            views.push(Span::styled(" · ", t.style(Token::Muted)));
        }
        let tok = if v == m.discover_view {
            Token::Accent
        } else {
            Token::Muted
        };
        views.push(Span::styled(v.title(), t.style(tok)));
    }
    views.push(Span::styled("   ctrl-v next view", t.style(Token::Muted)));
    vec![Line::from(views), Line::from(search)]
}

/// What an empty Discover view says: nothing found, or nothing matching the search.
fn none_match(t: Theme, noun: &str, q: &str) -> Line<'static> {
    let text = if q.is_empty() {
        format!("No {noun} found")
    } else {
        format!("No {noun} match \"{q}\"")
    };
    Line::styled(text, t.style(Token::Muted))
}

fn data_lines<T>(t: Theme, d: &Data<T>, what: &str) -> Option<Vec<Line<'static>>> {
    match d {
        Data::Ready(_) => None,
        Data::Loading => Some(vec![Line::styled(
            format!("Loading {what}…"),
            t.style(Token::Muted),
        )]),
        Data::Failed(e) => Some(error_lines(t, e)),
    }
}

fn discover(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let mut header = discover_head(m, t);
    let q = escape(m.filter().text.trim());
    match m.discover_view {
        app::DiscoverView::Plugins => {
            let mut b = discover_plugins(m, t);
            header.truncate(1);
            header.append(&mut b.header);
            b.header = header;
            b
        }
        app::DiscoverView::Developers => {
            if let Some(mut l) = data_lines(t, &m.catalog, "catalogs") {
                header.append(&mut l);
                return Body {
                    header,
                    ..Body::default()
                };
            }
            let list = developer_results(m);
            header.push(Line::styled(
                format!("{} developers · ⏎ lists one's plugins", list.len()),
                t.style(Token::Muted),
            ));
            header.push(Line::default());
            let name_w = list
                .iter()
                .map(|d| width(&escape(&d.name)))
                .max()
                .unwrap_or(0)
                .min(30);
            let rows = list
                .iter()
                .map(|d| {
                    Line::from(vec![
                        Span::styled(
                            pad(&cut(&escape(&d.name), name_w), name_w),
                            t.style(Token::Name),
                        ),
                        Span::raw("  "),
                        Span::raw(pad(
                            &format!(
                                "{} plugin{}",
                                d.plugins,
                                if d.plugins == 1 { "" } else { "s" }
                            ),
                            12,
                        )),
                        Span::styled(
                            cut(
                                &escape(&d.catalogs.join(", ")),
                                cols.saturating_sub(name_w + 18),
                            ),
                            t.style(Token::Muted),
                        ),
                    ])
                })
                .collect();
            Body {
                header,
                rows,
                empty: Some(none_match(t, "developers", &q)),
                skip: Vec::new(),
            }
        }
        app::DiscoverView::Marketplaces => {
            if let Some(mut l) = data_lines(t, &m.marketplaces, "marketplaces") {
                header.append(&mut l);
                return Body {
                    header,
                    ..Body::default()
                };
            }
            let list = marketplace_results(m);
            header.push(Line::styled(
                format!("{} marketplaces · ⏎ refreshes them", list.len()),
                t.style(Token::Muted),
            ));
            header.push(Line::default());
            let name_w = list
                .iter()
                .map(|mk| width(&escape(&mk.name)))
                .max()
                .unwrap_or(0)
                .min(30);
            let rows = list
                .iter()
                .map(|mk| {
                    let mut spans = vec![
                        Span::styled(
                            pad(&cut(&escape(&mk.name), name_w), name_w),
                            t.style(Token::Name),
                        ),
                        Span::raw("  "),
                        Span::raw(pad(&mk.tool, 6)),
                        Span::raw("  "),
                        Span::raw(pad(&mk.plugins.to_string(), 6)),
                        Span::raw("  "),
                        Span::styled(
                            mk.profiles.iter().cloned().collect::<Vec<_>>().join(", "),
                            t.style(Token::Muted),
                        ),
                    ];
                    if mk.managed {
                        spans.push(Span::styled("  ✓ managed", t.style(Token::Ok)));
                    }
                    Line::from(spans)
                })
                .collect();
            Body {
                header,
                rows,
                empty: Some(none_match(t, "marketplaces", &q)),
                skip: Vec::new(),
            }
        }
        app::DiscoverView::Connectors => {
            if let Some(mut l) = data_lines(t, &m.connectors, "connectors") {
                header.append(&mut l);
                return Body {
                    header,
                    ..Body::default()
                };
            }
            let list = connector_results(m);
            header.push(Line::styled(
                format!("{} connectors · installed through codex", list.len()),
                t.style(Token::Muted),
            ));
            header.push(Line::default());
            let rows = plugin_rows(t, &list, cols);
            Body {
                header,
                rows,
                empty: Some(none_match(t, "connectors", &q)),
                skip: Vec::new(),
            }
        }
    }
}

fn discover_plugins(m: &Model, t: Theme) -> Body {
    let cols = usize::from(m.width.saturating_sub(4));
    let c = match &m.catalog {
        Data::Ready(c) => c,
        Data::Loading => {
            return Body {
                header: vec![Line::styled("Loading catalogs…", t.style(Token::Muted))],
                ..Body::default()
            };
        }
        Data::Failed(e) => {
            return Body {
                header: error_lines(t, e),
                ..Body::default()
            };
        }
    };
    let f = m.filter();
    let mut search = vec![
        Span::styled("› ", t.style(Token::Accent)),
        Span::raw(f.text.clone()),
    ];
    if f.focused {
        search.push(Span::styled("▏", t.style(Token::Accent)));
    } else if f.text.is_empty() {
        search.push(Span::styled("/ to search", t.style(Token::Muted)));
    }
    let mut header = vec![Line::from(search)];
    if c.plugins.is_empty() {
        header.push(Line::default());
        header.push(Line::styled(
            "! No catalogs found",
            t.style(Token::Attention),
        ));
        header.push(Line::styled(
            "hint: start Claude or Codex once, or add a marketplace to config.toml",
            t.style(Token::Muted),
        ));
        return Body {
            header,
            ..Body::default()
        };
    }
    let (shown, _) = results(m);
    header.push(Line::styled(
        format!(
            "{} of {} plugins · {} catalog{}",
            shown.len(),
            c.plugins.len(),
            c.catalogs,
            if c.catalogs == 1 { "" } else { "s" }
        ),
        t.style(Token::Muted),
    ));
    for w in &c.warnings {
        header.push(Line::styled(
            cut(&format!("! {}", escape(w)), cols),
            t.style(Token::Attention),
        ));
    }
    header.push(Line::default());
    let rows = plugin_rows(t, &shown, cols);
    let empty = Some(Line::styled(
        format!("No plugins match \"{}\"", escape(f.text.trim())),
        t.style(Token::Muted),
    ));
    Body {
        header,
        rows,
        empty,
        skip: Vec::new(),
    }
}

/// Plugin (or connector) rows: name, developer, description, ✓ when installed.
fn plugin_rows(t: Theme, shown: &[crate::search::Plugin], cols: usize) -> Vec<Line<'static>> {
    let name_w = shown
        .iter()
        .map(|p| width(&escape(&p.name)))
        .max()
        .unwrap_or(0)
        .min(28);
    let dev_w = shown
        .iter()
        .map(|p| width(&escape(&p.developer)))
        .max()
        .unwrap_or(0)
        .min(18);
    let desc_w = cols.saturating_sub(2 + name_w + 2 + dev_w + 2 + 2).max(10);
    shown
        .iter()
        .map(|p| {
            let installed = !p.installed.is_empty();
            Line::from(vec![
                Span::styled(
                    pad(&cut(&escape(&p.name), name_w), name_w),
                    t.style(Token::Name),
                ),
                Span::raw("  "),
                Span::styled(
                    pad(&cut(&escape(&p.developer), dev_w), dev_w),
                    t.style(Token::Muted),
                ),
                Span::raw("  "),
                Span::raw(pad(&cut(&escape(&p.description), desc_w), desc_w)),
                Span::styled(
                    if installed { " ✓" } else { "  " }.to_string(),
                    t.style(Token::Ok),
                ),
            ])
        })
        .collect()
}

/// A held version at a glance (AC8-36): what changed, the recommendation, anything suspicious, and
/// what runs code. `d` opens the full review.
pub fn held_summary(h: &crate::core::quarantine::HeldSummary) -> Vec<String> {
    let mut out = vec![format!(
        "{}  {} · {} · {} file{}",
        escape(&h.name),
        escape(&h.change),
        h.size,
        h.files,
        if h.files == 1 { "" } else { "s" }
    )];
    out.push(String::new());
    if let Some(e) = &h.error {
        out.push(format!("✗ Can't be approved: {}", escape(e)));
        return out;
    }
    match &h.recommendation {
        Some(r) if r.approve => {
            out.push("✓ Recommendation: approve".into());
            out.push(format!("  {}", r.reason));
        }
        Some(r) => {
            out.push("! Look before approving".into());
            out.push(format!("  {}", escape(&r.reason)));
            out.push(String::new());
            out.push("Suspicious".into());
            for f in &r.suspicious {
                out.push(format!("  {}", escape(&f.path)));
                out.push(format!("      {}", escape(&f.reason)));
            }
        }
        None => out.push("· No review available".into()),
    }
    // One line per file, its reasons joined; files that only declare allowed-tools together (AC9-25).
    let grouped = crate::risk::allowed_tools_only(&h.flag_list);
    let mut code: Vec<(String, Vec<String>)> = Vec::new();
    for f in h
        .flag_list
        .iter()
        .filter(|f| f.kind != crate::risk::FlagKind::Suspicious && !grouped.contains(&f.path))
    {
        match code.iter_mut().find(|(p, _)| *p == f.path) {
            Some((_, reasons)) => reasons.push(f.reason.clone()),
            None => code.push((f.path.clone(), vec![f.reason.clone()])),
        }
    }
    let files = code.len() + grouped.len();
    if !grouped.is_empty() {
        let (what, why) = crate::risk::allowed_tools_line(&h.flag_list, &grouped);
        code.push((what, vec![why.replace("-v lists them", "d lists them")]));
    }
    if !code.is_empty() {
        out.push(String::new());
        out.push(format!(
            "Flagged ({files} file{})",
            if files == 1 { "" } else { "s" }
        ));
        for (path, reasons) in code.iter().take(8) {
            out.push(format!(
                "  {}  {}",
                escape(path),
                escape(&reasons.join("; "))
            ));
        }
        if code.len() > 8 {
            out.push(format!(
                "  … and {} more (d shows them all)",
                code.len() - 8
            ));
        }
    }
    out
}
