//! What each tab shows and does (T6–T9). The shell (T5) only needs `body`, `needs_you` and `act`.

use super::app::{Cmd, Model};
use super::keys::Action;
use super::theme::{Theme, Token};
use super::view::Body;
use ratatui::text::Line;

/// The current tab's body.
pub fn body(m: &Model, t: Theme) -> Body {
    Body {
        header: vec![Line::styled(
            m.tab.title().to_string(),
            t.style(Token::Name),
        )],
        rows: Vec::new(),
        empty: None,
    }
}

/// True when Overview has something that needs the person (its tab badge).
pub fn needs_you(_m: &Model) -> bool {
    false
}

/// A tab's own actions.
pub fn act(_m: &mut Model, _action: Action) -> Vec<Cmd> {
    Vec::new()
}
