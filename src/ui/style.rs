//! The palette as meaning (interfaces.md § Tokens). Colour adds to a symbol and a word; it never
//! carries meaning alone (AC6-04).

use console::Style;

/// A semantic token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Ok,
    Attention,
    Failed,
    Info,
    Name,
    Command,
    Dim,
    Heading,
    Added,
    Removed,
}

impl Token {
    /// The symbol for a state token; empty for the others.
    pub fn symbol(self) -> &'static str {
        match self {
            Token::Ok => "✓",
            Token::Attention => "!",
            Token::Failed => "✗",
            Token::Info => "·",
            _ => "",
        }
    }

    fn style(self) -> Style {
        let s = Style::new().force_styling(true);
        match self {
            Token::Ok | Token::Added => s.green(),
            Token::Attention => s.yellow(),
            Token::Failed | Token::Removed => s.red(),
            Token::Info | Token::Dim => s.dim(),
            Token::Name | Token::Heading => s.bold(),
            Token::Command => s.cyan(),
        }
    }
}

/// `text` in `token`'s style when `color` is on, else as is.
pub fn paint(text: &str, token: Token, color: bool) -> String {
    if color && !text.is_empty() {
        token.style().apply_to(text).to_string()
    } else {
        text.to_string()
    }
}

/// A state's symbol, coloured when `color` is on.
pub fn mark(token: Token, color: bool) -> String {
    paint(token.symbol(), token, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac6_04_symbols_carry_meaning_without_colour() {
        for (t, sym) in [
            (Token::Ok, "✓"),
            (Token::Attention, "!"),
            (Token::Failed, "✗"),
            (Token::Info, "·"),
        ] {
            assert_eq!(mark(t, false), sym);
            let coloured = mark(t, true);
            assert!(
                coloured.contains(sym) && coloured.contains('\u{1b}'),
                "{coloured:?}"
            );
        }
        assert_eq!(Token::Name.symbol(), "");
        assert_eq!(paint("x", Token::Command, false), "x");
        assert_eq!(paint("", Token::Command, true), "");
        for t in [
            Token::Dim,
            Token::Heading,
            Token::Added,
            Token::Removed,
            Token::Name,
        ] {
            assert!(paint("x", t, true).contains('\u{1b}'));
        }
    }
}
