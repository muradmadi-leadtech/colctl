//! Asking, only when it helps and only in a terminal (interfaces.md § Interaction). Every decision is
//! made here from the capabilities; the actual prompt is a function passed in, so each path is tested
//! without a terminal.

use super::term::Caps;
use std::io;
use std::io::Write;
use std::process::{Command, Stdio};

/// What a confirmation decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    /// Ctrl-C or end of input (EC6-14).
    Interrupted,
}

/// Asks `question` (AC6-18). `--yes` answers yes; without a terminal nothing is asked and the command
/// proceeds as it did in 2.1 (EC6-12).
pub fn confirm(
    caps: &Caps,
    question: &str,
    prompt: &dyn Fn(&str) -> io::Result<Option<bool>>,
) -> Answer {
    if caps.yes || !caps.interactive {
        return Answer::Yes;
    }
    match prompt(question) {
        Ok(Some(true)) => Answer::Yes,
        Ok(Some(false)) => Answer::No,
        Ok(None) | Err(_) => Answer::Interrupted,
    }
}

/// A picker: (title, items) → the chosen indexes, or `None` when interrupted.
pub type Picker<'a> = dyn Fn(&str, &[String]) -> io::Result<Option<Vec<usize>>> + 'a;

/// Picks from `items` (AC6-19): `None` when it can't ask (no terminal) or was interrupted.
pub fn pick(caps: &Caps, title: &str, items: &[String], prompt: &Picker) -> Option<Vec<usize>> {
    if !caps.interactive || items.is_empty() {
        return None;
    }
    prompt(title, items).ok().flatten()
}

/// The real confirmation: `[y/N]`, default no.
pub fn terminal_confirm(question: &str) -> io::Result<Option<bool>> {
    dialoguer::Confirm::new()
        .with_prompt(question)
        .default(false)
        .interact_opt()
        .map_err(io::Error::other)
}

/// The real multi-select picker.
pub fn terminal_pick_many(title: &str, items: &[String]) -> io::Result<Option<Vec<usize>>> {
    dialoguer::MultiSelect::new()
        .with_prompt(format!("{title} (space selects, enter confirms)"))
        .items(items)
        .interact_opt()
        .map_err(io::Error::other)
}

/// The real single-select picker, with fuzzy search for long lists.
pub fn terminal_pick_one(title: &str, items: &[String]) -> io::Result<Option<Vec<usize>>> {
    dialoguer::FuzzySelect::new()
        .with_prompt(title)
        .items(items)
        .max_length(15)
        .interact_opt()
        .map(|o| o.map(|i| vec![i]))
        .map_err(io::Error::other)
}

/// The pager command: `$PAGER` split on spaces, else `less -R -F -X`.
pub fn pager_command(pager: Option<&str>) -> Vec<String> {
    match pager.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => p.split_whitespace().map(str::to_string).collect(),
        None => ["less", "-R", "-F", "-X"].map(String::from).to_vec(),
    }
}

/// Whether `lines` should be paged (AC6-20): a terminal, not `--no-pager`, and taller than it.
pub fn should_page(caps: &Caps, lines: usize) -> bool {
    caps.pager && lines >= caps.height
}

/// Sends `text` to the pager; `Err` when it couldn't be started (EC6-15), so the caller prints instead.
pub fn run_pager(command: &[String], text: &str) -> io::Result<()> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| io::Error::other("no pager"))?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("the pager failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::term::{Facts, Settings};

    fn interactive(yes: bool) -> Caps {
        let tty = Facts {
            stdin_tty: true,
            stdout_tty: true,
            stderr_tty: true,
            rows: Some(30),
            ..Facts::default()
        };
        Caps::new(
            &Settings {
                yes,
                ..Settings::default()
            },
            &tty,
        )
    }

    #[test]
    fn ac6_18_ec6_12_confirmation_rules() {
        let never = |_: &str| -> io::Result<Option<bool>> { panic!("must not ask") };
        assert_eq!(
            confirm(&interactive(true), "x?", &never),
            Answer::Yes,
            "--yes"
        );
        assert_eq!(
            confirm(&Caps::plain(), "x?", &never),
            Answer::Yes,
            "no terminal"
        );
        let yes = |q: &str| -> io::Result<Option<bool>> {
            assert_eq!(q, "Remove?");
            Ok(Some(true))
        };
        assert_eq!(confirm(&interactive(false), "Remove?", &yes), Answer::Yes);
        assert_eq!(
            confirm(&interactive(false), "Remove?", &|_| Ok(Some(false))),
            Answer::No
        );
    }

    #[test]
    fn ec6_14_interrupted_prompts_change_nothing() {
        assert_eq!(
            confirm(&interactive(false), "x?", &|_| Ok(None)),
            Answer::Interrupted
        );
        assert_eq!(
            confirm(&interactive(false), "x?", &|_| Err(io::Error::other("eof"))),
            Answer::Interrupted
        );
        assert_eq!(
            pick(&interactive(false), "t", &["a".into()], &|_, _| Ok(None)),
            None
        );
    }

    #[test]
    fn ac6_19_ec6_13_pickers() {
        let items = vec!["a".to_string(), "b".to_string()];
        let chose = |_: &str, i: &[String]| -> io::Result<Option<Vec<usize>>> {
            assert_eq!(i.len(), 2);
            Ok(Some(vec![1]))
        };
        assert_eq!(
            pick(&interactive(false), "t", &items, &chose),
            Some(vec![1])
        );
        let never =
            |_: &str, _: &[String]| -> io::Result<Option<Vec<usize>>> { panic!("must not ask") };
        assert_eq!(
            pick(&Caps::plain(), "t", &items, &never),
            None,
            "no terminal"
        );
        assert_eq!(
            pick(&interactive(false), "t", &[], &never),
            None,
            "EC6-13: nothing to pick"
        );
    }

    #[test]
    fn ac6_20_ec6_15_pager() {
        assert_eq!(pager_command(None), vec!["less", "-R", "-F", "-X"]);
        assert_eq!(pager_command(Some("  ")), vec!["less", "-R", "-F", "-X"]);
        assert_eq!(pager_command(Some("bat --plain")), vec!["bat", "--plain"]);
        let caps = interactive(false);
        assert!(!should_page(&caps, 10));
        assert!(should_page(&caps, 40));
        assert!(!should_page(&Caps::plain(), 400), "not a terminal");
        assert!(run_pager(&["/nonexistent/pager".into()], "x").is_err());
        assert!(run_pager(&[], "x").is_err());
        assert!(run_pager(&["false".into()], "x").is_err());
        assert!(run_pager(&["cat".into()], "").is_ok());
    }
}
