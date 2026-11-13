//! Pure components (interfaces.md § Components): data and capabilities in, text out. Nothing here
//! prints, so every component is unit- and snapshot-tested.

use super::style::{Token, mark, paint};
use super::term::Caps;
use console::{measure_text_width, truncate_str};

/// Display width, ignoring escape codes; wide characters count 2 (EC6-08).
pub fn width(text: &str) -> usize {
    measure_text_width(text)
}

/// Cuts `text` to `max` columns, ending in "…" (AC6-05).
pub fn cut(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    truncate_str(text, max, "…").into_owned()
}

/// Cuts a path in the middle, keeping its start and its last parts (EC6-07):
/// `~/Library/Application Support/x/profiles/claude` → `~/Library/…/profiles/claude`.
pub fn cut_middle(path: &str, max: usize) -> String {
    if width(path) <= max {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    for keep in (1..parts.len()).rev() {
        for head in (1..parts.len().saturating_sub(keep)).rev() {
            let candidate = format!(
                "{}/…/{}",
                parts[..head].join("/"),
                parts[parts.len() - keep..].join("/")
            );
            if width(&candidate) <= max {
                return candidate;
            }
        }
    }
    cut(path, max)
}

/// A verdict: the first line of a result (`✓ All 5 profiles in sync · 3 sources`).
pub fn verdict(caps: &Caps, tone: Token, text: &str, facts: &[String]) -> String {
    let mut line = format!("{} {}", mark(tone, caps.color_out), text);
    for f in facts {
        line.push_str(&paint(&format!(" · {f}"), Token::Dim, caps.color_out));
    }
    line
}

/// A section heading.
pub fn heading(caps: &Caps, text: &str) -> String {
    paint(text, Token::Heading, caps.color_out)
}

/// A hint line: `→ <command>`.
pub fn hint(caps: &Caps, command: &str) -> String {
    format!(
        "{} {}",
        paint("→", Token::Dim, caps.color_out),
        paint(command, Token::Command, caps.color_out)
    )
}

/// A dim footer line.
pub fn footer(caps: &Caps, text: &str) -> String {
    paint(text, Token::Dim, caps.color_out)
}

/// One table cell: text and the token it's painted with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub token: Option<Token>,
}

impl Cell {
    pub fn plain(text: impl Into<String>) -> Self {
        Cell {
            text: text.into(),
            token: None,
        }
    }

    pub fn styled(text: impl Into<String>, token: Token) -> Self {
        Cell {
            text: text.into(),
            token: Some(token),
        }
    }
}

/// An aligned table (AC6-05): columns sized to their widest cell, two spaces apart, `indent` spaces in.
/// Column `flex` (the description, usually) is cut so a row never exceeds the width; earlier columns are
/// capped at a third of the width each. The last column is never padded.
pub fn table(caps: &Caps, indent: usize, rows: &[Vec<Cell>], flex: usize) -> Vec<String> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let cap = (caps.width / 3).max(8);
    let mut widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|cell| width(&cell.text))
                .max()
                .unwrap_or(0)
                .min(if c == flex { usize::MAX } else { cap })
        })
        .collect();
    let used: usize = indent
        + widths
            .iter()
            .enumerate()
            .filter(|(c, _)| *c != flex)
            .map(|(_, w)| w + 2)
            .sum::<usize>();
    if flex < cols {
        widths[flex] = widths[flex].min(caps.width.saturating_sub(used).max(4));
    }
    rows.iter()
        .map(|row| {
            let mut line = " ".repeat(indent);
            let last = row.len().saturating_sub(1);
            for (c, cell) in row.iter().enumerate() {
                let text = cut(&cell.text, widths[c]);
                let pad = if c == last {
                    0
                } else {
                    widths[c] - width(&text) + 2
                };
                let painted = match cell.token {
                    Some(t) => paint(&text, t, caps.color_out),
                    None => text,
                };
                line.push_str(&painted);
                line.push_str(&" ".repeat(pad));
            }
            line.trim_end().to_string()
        })
        .collect()
}

/// An aligned key–value block, keys dim.
pub fn kv(caps: &Caps, indent: usize, rows: &[(&str, String)]) -> Vec<String> {
    let key_width = rows
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| width(k))
        .max()
        .unwrap_or(0);
    let room = caps.width.saturating_sub(indent + key_width + 2).max(10);
    rows.iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| {
            format!(
                "{}{}{}  {}",
                " ".repeat(indent),
                paint(k, Token::Dim, caps.color_out),
                " ".repeat(key_width - width(k)),
                cut(v, room)
            )
        })
        .collect()
}

/// An error block (AC6-06): `✗ error: <what>`, the cause indented, then `hint: <what to do>`.
pub fn error(caps: &Caps, what: &str, cause: Option<&str>, hint: &str) -> Vec<String> {
    let c = caps.color_err;
    let mut lines = vec![format!(
        "{} {} {what}",
        mark(Token::Failed, c),
        paint("error:", Token::Failed, c)
    )];
    if let Some(cause) = cause.filter(|s| !s.is_empty()) {
        lines.extend(cause.lines().map(|l| format!("  {l}")));
    }
    lines.push(format!(
        "  {} {}",
        paint("hint:", Token::Dim, c),
        paint(hint, Token::Command, c)
    ));
    lines
}

/// A warning line for stderr.
pub fn warning(caps: &Caps, text: &str) -> String {
    format!(
        "{} {} {text}",
        mark(Token::Attention, caps.color_err),
        paint("warning:", Token::Attention, caps.color_err)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(width: usize) -> Caps {
        Caps {
            width,
            ..Caps::plain()
        }
    }

    #[test]
    fn ac6_05_tables_never_wrap() {
        let rows = vec![
            vec![
                Cell::plain("security-guidance@anthropic-plugin-directory"),
                Cell::plain("anthropics"),
                Cell::plain(
                    "Security reminders while editing files in a long description that goes on",
                ),
            ],
            vec![Cell::plain("x@m"), Cell::plain("o"), Cell::plain("short")],
        ];
        for w in [40, 60, 80, 120] {
            let lines = table(&at(w), 0, &rows, 2);
            for l in &lines {
                assert!(width(l) <= w, "{w}: {l:?} is {}", width(l));
            }
            // The second column starts at the same display column on every row.
            let starts: Vec<usize> = lines
                .iter()
                .map(|l| {
                    let i = l
                        .find("anthropics")
                        .or_else(|| l.find(" o ").map(|i| i + 1))
                        .unwrap();
                    width(&l[..i])
                })
                .collect();
            assert_eq!(starts[0], starts[1], "{lines:?}");
        }
        assert!(table(&at(80), 2, &rows, 2)[0].starts_with("  security"));
        assert!(table(&at(80), 0, &[], 0).is_empty());
    }

    #[test]
    fn ec6_07_long_names_and_paths() {
        assert_eq!(cut("abcdefghij", 5), "abcd…");
        assert_eq!(cut("abc", 5), "abc");
        assert_eq!(
            cut_middle(
                "~/Library/Application Support/kilo-proxy-headless/profiles/claude",
                30
            ),
            "~/Library/…/profiles/claude"
        );
        assert_eq!(cut_middle("~/.claude", 30), "~/.claude");
        assert!(width(&cut_middle("/averyveryverylongsinglesegmentname", 10)) <= 10);
    }

    #[test]
    fn ec6_08_wide_characters_stay_aligned() {
        let rows = vec![
            vec![Cell::plain("日本語プラグイン"), Cell::plain("dev")],
            vec![Cell::plain("ascii"), Cell::plain("dev")],
        ];
        let lines = table(&at(80), 0, &rows, 1);
        assert_eq!(width(&lines[0]), width(&lines[1]));
        let col = |l: &str| width(&l[..l.rfind("dev").unwrap()]);
        assert_eq!(col(&lines[0]), col(&lines[1]));
    }

    #[test]
    fn components_render() {
        let c = at(80);
        assert_eq!(
            verdict(
                &c,
                Token::Ok,
                "All 5 profiles in sync",
                &["3 sources".into()]
            ),
            "✓ All 5 profiles in sync · 3 sources"
        );
        assert_eq!(heading(&c, "Needs attention"), "Needs attention");
        assert_eq!(hint(&c, "colctl sync"), "→ colctl sync");
        assert_eq!(footer(&c, "2 of 2"), "2 of 2");
        assert_eq!(
            kv(
                &c,
                2,
                &[
                    ("repo", "https://x".into()),
                    ("author", String::new()),
                    ("id", "a".into())
                ]
            ),
            vec!["  repo  https://x", "  id    a"]
        );
        assert_eq!(
            error(
                &c,
                "no source named \"x\"",
                Some("because"),
                "colctl status lists your sources"
            ),
            vec![
                "✗ error: no source named \"x\"",
                "  because",
                "  hint: colctl status lists your sources"
            ]
        );
        assert_eq!(error(&c, "x", None, "h").len(), 2);
        assert_eq!(
            warning(&c, "a catalog is unreadable"),
            "! warning: a catalog is unreadable"
        );
        let coloured = Caps {
            color_out: true,
            color_err: true,
            ..c
        };
        assert!(verdict(&coloured, Token::Ok, "x", &["y".into()]).contains('\u{1b}'));
        let styled = table(
            &coloured,
            0,
            &[vec![Cell::styled("a", Token::Name), Cell::plain("b")]],
            1,
        );
        assert!(styled[0].contains('\u{1b}'));
    }
}
