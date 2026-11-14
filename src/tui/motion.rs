//! Motion (design spec § Motion): tachyonfx effects drawn over a finished frame. They only explain
//! a change (where the new tab came from, what just opened, what just finished), never block a key
//! (a key finishes them at once), stop when nothing moves (no redraws while idle), and are off with
//! `COLCTL_REDUCED_MOTION`. Without colour they use only characters, so no frame gains a colour.

use super::app::{Model, Tab};
use super::view;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{Effect, Interpolation, Motion, fx};

/// Tab switch.
pub const TAB_MS: u32 = 180;
/// An overlay opening.
pub const OVERLAY_MS: u32 = 150;
/// A profile or source finishing.
pub const ROW_MS: u32 = 250;
/// Lists after a reload.
pub const RELOAD_MS: u32 = 200;

/// What the last frame showed, to see what changed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    tab: Tab,
    overlay: bool,
    finished: usize,
    loads: usize,
}

fn seen(m: &Model) -> Seen {
    Seen {
        tab: m.tab,
        overlay: m.overlay.is_some(),
        finished: m
            .job
            .as_ref()
            .map_or(0, |j| j.rows.iter().filter(|r| r.ok.is_some()).count()),
        loads: m.loads,
    }
}

/// The running effects.
pub struct Fx {
    on: bool,
    color: bool,
    effects: Vec<Effect>,
    last: Option<Seen>,
    /// When effects were last drawn (ms since start); `None` when none run.
    clock: Option<u64>,
}

impl Fx {
    pub fn new(m: &Model) -> Self {
        Fx {
            on: m.opts.motion,
            color: m.opts.color,
            effects: Vec::new(),
            last: None,
            clock: None,
        }
    }

    /// True while an effect runs: the app keeps drawing until it ends.
    pub fn running(&self) -> bool {
        self.effects.iter().any(Effect::running)
    }

    /// How many effects are running (tests).
    pub fn len(&self) -> usize {
        self.effects.iter().filter(|e| e.running()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Ends every effect now (a key was pressed).
    pub fn finish(&mut self) {
        self.effects.clear();
        self.clock = None;
    }

    /// Starts the effects for what changed since the last frame.
    pub fn observe(&mut self, m: &Model) {
        let now = seen(m);
        let Some(before) = self.last.replace(now.clone()) else {
            return;
        };
        if !self.on || m.too_small() {
            return;
        }
        if now.tab != before.tab {
            let forward = now.tab as usize > before.tab as usize;
            self.tab_switch(view::body_area(m), forward);
        }
        if now.overlay && !before.overlay {
            self.add(
                fx::coalesce((OVERLAY_MS, Interpolation::QuadOut)),
                view::overlay_area(m),
            );
        }
        if now.finished > before.finished && self.color {
            self.add(
                fx::hsl_shift_fg([0.0, 0.0, 25.0], (ROW_MS, Interpolation::SineOut)).reversed(),
                view::status_area(m),
            );
        }
        if now.loads > before.loads && now.tab == before.tab && !now.overlay {
            self.add(
                fx::coalesce((RELOAD_MS, Interpolation::QuadOut)),
                view::list_area(m),
            );
        }
    }

    fn tab_switch(&mut self, area: Rect, forward: bool) {
        let timer = (TAB_MS, Interpolation::QuadOut);
        let e = if self.color {
            let dir = if forward {
                Motion::RightToLeft
            } else {
                Motion::LeftToRight
            };
            fx::sweep_in(dir, 4, 0, Color::Reset, timer)
        } else {
            fx::coalesce(timer)
        };
        self.add(e, area);
    }

    fn add(&mut self, e: Effect, area: Rect) {
        if area.area() > 0 {
            self.effects.push(e.with_area(area));
        }
    }

    /// Draws the effects over `buf` as of `now` (ms since start), dropping the ones that ended. A
    /// new effect starts from its first frame, however long the app was idle before it.
    pub fn apply(&mut self, now: u64, buf: &mut Buffer) {
        if self.effects.is_empty() {
            self.clock = None;
            return;
        }
        let elapsed = self.clock.map_or(0, |c| now.saturating_sub(c)).min(1000);
        self.clock = Some(now);
        let delta = tachyonfx::Duration::from_millis(elapsed as u32);
        let area = buf.area;
        for e in &mut self.effects {
            e.process(delta, buf, area);
        }
        self.effects.retain(Effect::running);
        if self.effects.is_empty() {
            self.clock = None;
        }
    }
}
