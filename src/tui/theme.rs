//! Colour tokens (design spec § Tokens): the terminal's own 16 ANSI colours, so light and dark
//! themes both work, and the only place colours are named. Without colour, emphasis carries what
//! colour would: bold for problems, underline for the accent, reverse for the selection.

use crate::core::status::Tone;
use ratatui::style::{Color, Modifier, Style};

/// A semantic token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Ok,
    Attention,
    Failed,
    Info,
    Accent,
    Name,
    Muted,
    Added,
    Removed,
    Selection,
    Plain,
}

impl From<Tone> for Token {
    fn from(t: Tone) -> Self {
        match t {
            Tone::Ok => Token::Ok,
            Tone::Attention => Token::Attention,
            Tone::Failed => Token::Failed,
            Tone::Info => Token::Info,
        }
    }
}

/// A state's symbol, as in the CLI.
pub fn symbol(t: Tone) -> &'static str {
    match t {
        Tone::Ok => "✓",
        Tone::Attention => "!",
        Tone::Failed => "✗",
        Tone::Info => "·",
    }
}

/// Styles for one run of the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub color: bool,
}

impl Theme {
    pub fn style(self, t: Token) -> Style {
        let s = Style::new();
        if !self.color {
            return match t {
                Token::Attention | Token::Failed | Token::Name => s.add_modifier(Modifier::BOLD),
                Token::Accent => s.add_modifier(Modifier::UNDERLINED),
                Token::Selection => s.add_modifier(Modifier::REVERSED),
                _ => s,
            };
        }
        match t {
            Token::Ok | Token::Added => s.fg(Color::Green),
            Token::Attention => s.fg(Color::Yellow),
            Token::Failed | Token::Removed => s.fg(Color::Red),
            Token::Info | Token::Muted => s.add_modifier(Modifier::DIM),
            Token::Accent => s.fg(Color::Cyan),
            Token::Name => s.add_modifier(Modifier::BOLD),
            Token::Selection => s.add_modifier(Modifier::REVERSED),
            Token::Plain => s,
        }
    }
}
