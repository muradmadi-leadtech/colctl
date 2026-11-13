//! Live progress while a run calls tools (AC6-15): one line per profile on stderr with a spinner and
//! the current step. Drawn only when stderr is a terminal (and not `-q`, `--json` or `CI`); cleared
//! when the run ends or is dropped, so the result block on stdout isn't repeated and an error never
//! lands on a half-drawn line (EC6-19).

use super::term::Caps;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::time::Duration;

/// A run over several profiles.
pub struct Run {
    multi: Option<MultiProgress>,
    bars: Vec<(String, ProgressBar)>,
    width: usize,
}

impl Run {
    /// One waiting line per profile name, when `caps` allows drawing; otherwise a no-op.
    pub fn new(caps: &Caps, profiles: &[String]) -> Self {
        if !caps.progress || profiles.is_empty() {
            return Run {
                multi: None,
                bars: Vec::new(),
                width: 0,
            };
        }
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
        let width = profiles
            .iter()
            .map(|p| p.chars().count())
            .max()
            .unwrap_or(0);
        let style = ProgressStyle::with_template("{spinner} {prefix}  {wide_msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ");
        let bars = profiles
            .iter()
            .map(|p| {
                let bar = multi.add(ProgressBar::new_spinner());
                bar.set_style(style.clone());
                bar.set_prefix(format!("{p:<width$}"));
                bar.set_message("waiting");
                (p.clone(), bar)
            })
            .collect();
        Run {
            multi: Some(multi),
            bars,
            width,
        }
    }

    fn bar(&self, profile: &str) -> Option<&ProgressBar> {
        self.bars.iter().find(|(p, _)| p == profile).map(|(_, b)| b)
    }

    /// The profile is working on step `n` of `total`.
    pub fn step(&self, profile: &str, what: &str, n: usize, total: usize) {
        if let Some(bar) = self.bar(profile) {
            bar.enable_steady_tick(Duration::from_millis(80));
            bar.set_message(format!("{what}  {n}/{total}"));
        }
    }

    /// The profile is done; its line shows `summary` until the run ends.
    pub fn done(&self, profile: &str, summary: &str) {
        if let Some(bar) = self.bar(profile) {
            bar.set_style(
                ProgressStyle::with_template("{prefix}  {wide_msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            bar.finish_with_message(summary.to_string());
        }
    }

    /// True when anything is being drawn.
    pub fn is_live(&self) -> bool {
        self.multi.is_some()
    }

    /// The width of the profile column.
    pub fn prefix_width(&self) -> usize {
        self.width
    }

    /// Removes every progress line.
    pub fn clear(&mut self) {
        for (_, bar) in &self.bars {
            bar.finish_and_clear();
        }
        if let Some(m) = &self.multi {
            let _ = m.clear();
        }
        self.bars.clear();
        self.multi = None;
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::term::{Facts, Settings};

    #[test]
    fn ac6_15_nothing_is_drawn_without_a_terminal() {
        let mut run = Run::new(&Caps::plain(), &["claude".into()]);
        assert!(!run.is_live());
        run.step("claude", "installing x", 1, 2);
        run.done("claude", "1 change");
        run.clear();
        // With a terminal, lines exist (drawn to stderr; hidden under the test harness).
        let tty = Facts {
            stderr_tty: true,
            ..Facts::default()
        };
        let caps = Caps::new(&Settings::default(), &tty);
        let mut run = Run::new(&caps, &["claude".into(), "kilo-claude".into()]);
        assert!(run.is_live());
        assert_eq!(run.prefix_width(), 11);
        run.step("claude", "installing x", 1, 2);
        run.step("nobody", "x", 1, 1);
        run.done("claude", "✓ 1 change");
        run.clear();
        assert!(!run.is_live());
        assert!(!Run::new(&caps, &[]).is_live());
    }

    #[test]
    fn ec6_19_progress_is_cleared_before_anything_else_prints() {
        let tty = Facts {
            stderr_tty: true,
            ..Facts::default()
        };
        let caps = Caps::new(&Settings::default(), &tty);
        let mut run = Run::new(&caps, &["claude".into()]);
        run.step("claude", "installing x", 1, 1);
        run.clear();
        assert!(!run.is_live());
        // Dropping a live run clears it too (an error unwinding past it).
        let live = Run::new(&caps, &["codex".into()]);
        drop(live);
    }
}
