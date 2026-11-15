//! The keymap (design spec § Keymap): one table drives handling, the key bar and the `?` overlay,
//! so a key that isn't here doesn't exist.

use super::app::Tab;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where a binding applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Tab(Tab),
    /// Inside a detail overlay.
    Detail,
    /// Inside a confirmation.
    Confirm,
    /// Inside a pick list (adopt's checklist).
    Pick,
}

/// What a key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NextTab,
    PrevTab,
    GoTab(Tab),
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Open,
    Back,
    Filter,
    Reload,
    Sync,
    Update,
    Approve,
    Reject,
    ApproveAll,
    Add,
    NextFile,
    PrevFile,
    FullReview,
    Help,
    Quit,
    Yes,
    No,
    /// `colctl rm` the focused item (AC9-17).
    Remove,
    /// `colctl convert` the focused source, or undo it (AC9-18).
    Convert,
    /// `colctl refresh` the focused taken-over item (AC9-19).
    Refresh,
    /// Pick what to `colctl adopt` from the focused profile (AC9-20).
    Adopt,
    /// Tick or untick in a pick list.
    Toggle,
    /// Discover's next view: plugins, developers, marketplaces, connectors (AC9-21).
    NextView,
}

/// One key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// How the key is written in the key bar and help.
    pub key: &'static str,
    pub code: KeyCode,
    pub mods: KeyModifiers,
    pub scope: Scope,
    pub action: Action,
    pub label: &'static str,
    /// Shown in the key bar (most useful first, in table order).
    pub bar: bool,
}

const fn b(
    key: &'static str,
    code: KeyCode,
    scope: Scope,
    action: Action,
    label: &'static str,
    bar: bool,
) -> Binding {
    Binding {
        key,
        code,
        mods: KeyModifiers::NONE,
        scope,
        action,
        label,
        bar,
    }
}

/// A binding with modifiers (Ctrl-V in Discover, where letters search).
const fn bm(
    key: &'static str,
    code: KeyCode,
    mods: KeyModifiers,
    scope: Scope,
    action: Action,
    label: &'static str,
    bar: bool,
) -> Binding {
    Binding {
        key,
        code,
        mods,
        scope,
        action,
        label,
        bar,
    }
}

use Action as A;
use KeyCode as K;
use Scope as S;

/// Every binding. Earlier entries win when two match in one scope.
#[rustfmt::skip]
pub const KEYMAP: &[Binding] = &[
    // Confirmations: y or n, nothing else (⏎ alone does nothing).
    b("y", K::Char('y'), S::Confirm, A::Yes, "yes", true),
    b("n", K::Char('n'), S::Confirm, A::No, "no", true),
    b("esc", K::Esc, S::Confirm, A::No, "no", false),
    // Detail overlays.
    b("a", K::Char('a'), S::Detail, A::Approve, "approve", true),
    b("x", K::Char('x'), S::Detail, A::Reject, "reject", true),
    b(
        "d",
        K::Char('d'),
        S::Detail,
        A::FullReview,
        "full review",
        true,
    ),
    b("]", K::Char(']'), S::Detail, A::NextFile, "next file", true),
    b(
        "[",
        K::Char('['),
        S::Detail,
        A::PrevFile,
        "previous file",
        false,
    ),
    b("esc", K::Esc, S::Detail, A::Back, "close", true),
    // Pick lists (adopt's checklist).
    b("space", K::Char(' '), S::Pick, A::Toggle, "tick", true),
    b("⏎", K::Enter, S::Pick, A::Open, "adopt ticked", true),
    b("esc", K::Esc, S::Pick, A::Back, "close", true),
    // Tabs.
    b("⏎", K::Enter, S::Tab(Tab::Overview), A::Open, "run", true),
    b("s", K::Char('s'), S::Tab(Tab::Overview), A::Sync, "sync", true),
    b("u", K::Char('u'), S::Tab(Tab::Overview), A::Update, "check updates", true),
    b("s", K::Char('s'), S::Tab(Tab::Profiles), A::Sync, "sync", true),
    b("⏎", K::Enter, S::Tab(Tab::Profiles), A::Open, "open", true),
    b("a", K::Char('a'), S::Tab(Tab::Profiles), A::Adopt, "adopt", true),
    b("/", K::Char('/'), S::Tab(Tab::Profiles), A::Filter, "filter", true),
    b("⏎", K::Enter, S::Tab(Tab::Items), A::Open, "open", true),
    b("x", K::Char('x'), S::Tab(Tab::Items), A::Remove, "remove", true),
    b("c", K::Char('c'), S::Tab(Tab::Items), A::Convert, "convert", true),
    b("f", K::Char('f'), S::Tab(Tab::Items), A::Refresh, "refresh", true),
    b("/", K::Char('/'), S::Tab(Tab::Items), A::Filter, "filter", true),
    b("a", K::Char('a'), S::Tab(Tab::Review), A::Approve, "approve", true),
    b("x", K::Char('x'), S::Tab(Tab::Review), A::Reject, "reject", true),
    b("u", K::Char('u'), S::Tab(Tab::Review), A::Update, "check updates", true),
    b("⏎", K::Enter, S::Tab(Tab::Review), A::Open, "open", true),
    b("A", K::Char('A'), S::Tab(Tab::Review), A::ApproveAll, "approve all", false),
    // Discover: typing searches (AC8-35); these only show in the key bar and help.
    b("type", K::Null, S::Tab(Tab::Discover), A::Filter, "to search", true),
    b("⏎", K::Enter, S::Tab(Tab::Discover), A::Open, "details", true),
    b("esc", K::Esc, S::Tab(Tab::Discover), A::Back, "clear", true),
    b("←→", K::Null, S::Tab(Tab::Discover), A::NextTab, "tabs", true),
    bm("ctrl-v", K::Char('v'), KeyModifiers::CONTROL, S::Tab(Tab::Discover), A::NextView, "view", true),
    b("ctrl-c", K::Null, S::Tab(Tab::Discover), A::Quit, "quit", true),
    // Everywhere.
    b("→", K::Right, S::Global, A::NextTab, "next tab", false),
    b("tab", K::Tab, S::Global, A::NextTab, "next tab", false),
    b("←", K::Left, S::Global, A::PrevTab, "previous tab", false),
    b("⇧tab", K::BackTab, S::Global, A::PrevTab, "previous tab", false),
    b(
        "1",
        K::Char('1'),
        S::Global,
        A::GoTab(Tab::Overview),
        "Overview",
        false,
    ),
    b(
        "2",
        K::Char('2'),
        S::Global,
        A::GoTab(Tab::Profiles),
        "Profiles",
        false,
    ),
    b(
        "3",
        K::Char('3'),
        S::Global,
        A::GoTab(Tab::Items),
        "Items",
        false,
    ),
    b(
        "4",
        K::Char('4'),
        S::Global,
        A::GoTab(Tab::Review),
        "Review",
        false,
    ),
    b(
        "5",
        K::Char('5'),
        S::Global,
        A::GoTab(Tab::Discover),
        "Discover",
        false,
    ),
    b("↑", K::Up, S::Global, A::Up, "up", false),
    b("k", K::Char('k'), S::Global, A::Up, "up", false),
    b("↓", K::Down, S::Global, A::Down, "down", false),
    b("j", K::Char('j'), S::Global, A::Down, "down", false),
    b("pgup", K::PageUp, S::Global, A::PageUp, "page up", false),
    b(
        "pgdn",
        K::PageDown,
        S::Global,
        A::PageDown,
        "page down",
        false,
    ),
    b("g", K::Char('g'), S::Global, A::Top, "top", false),
    b("G", K::Char('G'), S::Global, A::Bottom, "bottom", false),
    b("home", K::Home, S::Global, A::Top, "top", false),
    b("end", K::End, S::Global, A::Bottom, "bottom", false),
    b("esc", K::Esc, S::Global, A::Back, "back", false),
    b("r", K::Char('r'), S::Global, A::Reload, "reload", true),
    b("?", K::Char('?'), S::Global, A::Help, "keys", true),
    b("q", K::Char('q'), S::Global, A::Quit, "quit", true),
];

/// Ctrl-C quits from anywhere, overlays included.
pub fn is_interrupt(ev: &KeyEvent) -> bool {
    ev.code == K::Char('c') && ev.modifiers.contains(KeyModifiers::CONTROL)
}

fn same(b: &Binding, ev: &KeyEvent) -> bool {
    // Shifted letters arrive as uppercase chars, with or without SHIFT; ignore it.
    let mods = ev.modifiers - KeyModifiers::SHIFT;
    b.code == ev.code && b.mods == mods
}

/// The action for `ev`, looked up in `scopes` in order.
pub fn lookup(scopes: &[Scope], ev: &KeyEvent) -> Option<Action> {
    scopes.iter().find_map(|scope| {
        KEYMAP
            .iter()
            .find(|b| b.scope == *scope && b.code != K::Null && same(b, ev))
            .map(|b| b.action)
    })
}

/// The key bar for `scopes`: their bar bindings, one per action, `?` and `q` last.
pub fn bar(scopes: &[Scope]) -> Vec<&'static Binding> {
    let mut out: Vec<&'static Binding> = Vec::new();
    for scope in scopes {
        for b in KEYMAP.iter().filter(|b| b.scope == *scope && b.bar) {
            if !out.iter().any(|o| o.action == b.action) {
                out.push(b);
            }
        }
    }
    out.sort_by_key(|b| matches!(b.action, Action::Help | Action::Quit));
    out
}

/// Every binding for the `?` overlay: the scopes' own, then the global ones.
pub fn help(scopes: &[Scope]) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for scope in scopes {
        for b in KEYMAP.iter().filter(|b| b.scope == *scope) {
            if let Action::GoTab(_) = b.action {
                if !out.iter().any(|(k, _)| k == "1–5") {
                    out.push(("1–5".into(), "go to a tab"));
                }
                continue;
            }
            match out.iter_mut().find(|(_, l)| *l == b.label) {
                Some((keys, _)) if !keys.split(' ').any(|k| k == b.key) => {
                    keys.push(' ');
                    keys.push_str(b.key);
                }
                Some(_) => {}
                None => out.push((b.key.to_string(), b.label)),
            }
        }
    }
    out.push(("ctrl-c".into(), "quit"));
    out
}
