//! The command palette (AC9-23): every action from the keymap, fuzzy filtered, the ones that work
//! here first. ⏎ runs one as if its key had been pressed, switching tab first.

use super::app::{Model, Overlay, Tab};
use super::keys::{Action, KEYMAP, Scope};
use ratatui::crossterm::event::KeyCode;

/// One action the palette offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The tab it belongs to; `None` for one that works everywhere.
    pub tab: Option<Tab>,
    pub key: &'static str,
    pub label: &'static str,
    pub action: Action,
    /// Why it can't run now (EC9-15).
    pub why_not: Option<String>,
}

/// Moving, confirming and the palette itself aren't things to run from it.
fn offered(a: Action) -> bool {
    !matches!(
        a,
        Action::Up
            | Action::Down
            | Action::PageUp
            | Action::PageDown
            | Action::Top
            | Action::Bottom
            | Action::Back
            | Action::Yes
            | Action::No
            | Action::Toggle
            | Action::NextFile
            | Action::PrevFile
            | Action::FullReview
            | Action::NextTab
            | Action::PrevTab
            | Action::Palette
    )
}

/// Why `action` in `tab` can't run now, if it can't.
fn why_not(m: &Model, tab: Option<Tab>, action: Action) -> Option<String> {
    let profiles = m.snapshot.ready().map_or(0, |s| s.profiles.len());
    match (tab, action) {
        (Some(Tab::Review), Action::Approve | Action::Reject | Action::ApproveAll)
            if m.held_count() == 0 =>
        {
            Some("Nothing held for review".into())
        }
        (Some(Tab::Items), Action::Remove | Action::Convert | Action::Refresh)
            if super::tabs::items_across(m).is_empty() =>
        {
            Some("No items yet".into())
        }
        (_, Action::Sync | Action::Adopt) if profiles == 0 => Some("No profiles registered".into()),
        _ => None,
    }
}

fn query(m: &Model) -> &str {
    match &m.overlay {
        Some(Overlay::Palette { query, .. }) => query,
        _ => "",
    }
}

/// What the palette lists now: matching the typed words, usable ones first (this tab's, then
/// everywhere's, then other tabs'), unusable ones last.
pub fn entries(m: &Model) -> Vec<Entry> {
    if m.width > 0 {
        return Vec::new();
    }
    let mut all: Vec<Entry> = Vec::new();
    for b in KEYMAP {
        let tab = match b.scope {
            Scope::Tab(t) => Some(t),
            Scope::Global => None,
            _ => continue,
        };
        if b.code == KeyCode::Null || !offered(b.action) {
            continue;
        }
        if all.iter().any(|e| e.tab == tab && e.action == b.action) {
            continue;
        }
        all.push(Entry {
            tab,
            key: b.key,
            label: b.label,
            action: b.action,
            why_not: why_not(m, tab, b.action),
        });
    }
    let q = query(m).trim().to_string();
    let mut fuzzy = crate::search::Fuzzy::default();
    let mut scored: Vec<(Entry, u32)> = all
        .into_iter()
        .filter_map(|e| {
            if q.is_empty() {
                return Some((e, 0));
            }
            let text = format!("{} {}", e.label, e.tab.map_or("", Tab::title));
            q.split_whitespace()
                .map(|w| fuzzy.score(w, &text).map(u32::from))
                .sum::<Option<u32>>()
                .map(|s| (e, s))
        })
        .collect();
    let here = m.tab;
    let group = |e: &Entry| match (&e.why_not, e.tab) {
        (Some(_), _) => 3,
        (None, Some(t)) if t == here => 0,
        (None, None) => 1,
        (None, Some(_)) => 2,
    };
    // Stable: keymap order within a group and score.
    scored.sort_by(|(a, sa), (b, sb)| group(a).cmp(&group(b)).then(sb.cmp(sa)));
    scored.into_iter().map(|(e, _)| e).collect()
}
