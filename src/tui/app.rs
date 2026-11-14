//! The TUI's state and how it changes (components.md § tui::app): a `Model`, the `Msg`s that change
//! it, and the `Cmd`s it asks for. `update` is pure: no terminal, no files, no threads, so tests
//! drive it with keys and check what it asks to run.

use super::keys::{self, Action, Scope};
use crate::core::discover::Catalog;
use crate::core::quarantine::HeldSummary;
use crate::core::run::{Job, Report};
use crate::core::status::Snapshot;
use crate::error::CliError;
use ratatui::crossterm::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};

/// The five tabs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Overview,
    Profiles,
    Items,
    Review,
    Discover,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Profiles,
        Tab::Items,
        Tab::Review,
        Tab::Discover,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Profiles => "Profiles",
            Tab::Items => "Items",
            Tab::Review => "Review",
            Tab::Discover => "Discover",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn step(self, by: isize) -> Tab {
        let n = Tab::ALL.len() as isize;
        Tab::ALL[((self.index() as isize + by).rem_euclid(n)) as usize]
    }
}

/// How the TUI was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub color: bool,
    pub motion: bool,
}

/// Something loaded in the background.
#[derive(Debug)]
pub enum Loaded {
    Snapshot(Box<Result<Snapshot, CliError>>),
    Held(Result<Vec<HeldSummary>, CliError>),
    Catalog(Result<Catalog, CliError>),
    /// A detail overlay's lines (`review <name>`, `show <id>`).
    Detail {
        title: String,
        lines: Result<Vec<String>, CliError>,
    },
}

/// Progress from a running job.
#[derive(Debug)]
pub enum JobEvent {
    Step {
        profile: String,
        action: String,
        n: usize,
        total: usize,
    },
    Done {
        profile: String,
        summary: String,
        ok: bool,
    },
    Note(String),
    Warn(String),
    Finished(Result<Report, CliError>),
}

/// What happened.
#[derive(Debug)]
pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize(u16, u16),
    /// Milliseconds since the app started.
    Tick(u64),
    Loaded(Loaded),
    Job(JobEvent),
}

/// What to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    Snapshot,
    Held,
    Catalog,
    Review(String),
    Show(String),
}

/// What the app asks its runtime to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Load(Load),
    Run(Job),
    /// Ask a running job to stop before its next unit of work.
    Cancel,
    Quit,
}

/// A value that may still be loading.
#[derive(Debug, Default)]
pub enum Data<T> {
    #[default]
    Loading,
    Ready(T),
    Failed(CliError),
}

impl<T> Data<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Data::Ready(v) => Some(v),
            _ => None,
        }
    }
}

/// A position in a list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub selected: usize,
    pub offset: usize,
}

impl Cursor {
    /// Moves by `by` within `len` rows, keeping the selection on screen in `height` rows.
    pub fn move_by(&mut self, by: isize, len: usize, height: usize) {
        if len == 0 {
            *self = Cursor::default();
            return;
        }
        let to = (self.selected as isize + by).clamp(0, len as isize - 1) as usize;
        self.selected = to;
        self.fit(len, height);
    }

    /// Keeps the selection inside `len` and on screen.
    pub fn fit(&mut self, len: usize, height: usize) {
        if len == 0 {
            *self = Cursor::default();
            return;
        }
        self.selected = self.selected.min(len - 1);
        let height = height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
        self.offset = self.offset.min(len.saturating_sub(height));
    }
}

/// A one-line input (filter, search).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    pub text: String,
    /// Keys go to the input.
    pub focused: bool,
}

/// A window over the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Confirm {
        title: String,
        body: Vec<String>,
        job: Job,
    },
    /// Quit once the running job finishes?
    ConfirmQuit,
    Detail {
        title: String,
        lines: Vec<String>,
        scroll: usize,
        /// The held item it shows, for `a` and `x`.
        held: Option<String>,
    },
    Error {
        what: String,
        cause: Option<String>,
        hint: Option<String>,
    },
}

/// One profile's (or source's) row in the progress panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressRow {
    pub name: String,
    pub text: String,
    /// `None` while working, then whether it went well.
    pub ok: Option<bool>,
}

/// The write that's running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub job: Job,
    pub rows: Vec<ProgressRow>,
    pub started: u64,
}

/// A result in the status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    /// Errors stay until a key is pressed.
    pub until: Option<u64>,
}

/// The whole UI state.
#[derive(Debug)]
pub struct Model {
    pub opts: Options,
    pub width: u16,
    pub height: u16,
    pub now: u64,
    pub tab: Tab,
    pub snapshot: Data<Snapshot>,
    pub held: Data<Vec<HeldSummary>>,
    pub catalog: Data<Catalog>,
    pub cursors: [Cursor; 5],
    pub filters: [Input; 5],
    pub overlay: Option<Overlay>,
    pub job: Option<Running>,
    pub toast: Option<Toast>,
    pub quit: bool,
    pub quit_after_job: bool,
    /// When the current tab was entered (for its transition).
    pub tab_since: u64,
    /// Something changed what's on screen since the last draw.
    pub dirty: bool,
    /// When the last left click was (for double clicks).
    pub last_click: Option<u64>,
    /// How many times the snapshot has loaded (motion marks a reload).
    pub loads: usize,
    /// Loads asked for and not back yet.
    pub pending: usize,
}

/// The smallest screen the TUI draws on (AC8-11).
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 20;
/// How long a result stays in the status line.
pub const TOAST_MS: u64 = 4000;

impl Model {
    pub fn new(opts: Options, width: u16, height: u16) -> Self {
        Model {
            opts,
            width,
            height,
            now: 0,
            tab: Tab::Overview,
            snapshot: Data::Loading,
            held: Data::Loading,
            catalog: Data::Loading,
            cursors: [Cursor::default(); 5],
            filters: Default::default(),
            overlay: None,
            job: None,
            toast: None,
            quit: false,
            quit_after_job: false,
            tab_since: 0,
            dirty: true,
            last_click: None,
            loads: 0,
            pending: 0,
        }
    }

    /// What to load when the app opens: local state first; the catalog after (AC8-08).
    pub fn start(&mut self) -> Vec<Cmd> {
        self.pending += 3;
        vec![
            Cmd::Load(Load::Snapshot),
            Cmd::Load(Load::Held),
            Cmd::Load(Load::Catalog),
        ]
    }

    pub fn too_small(&self) -> bool {
        self.width < MIN_WIDTH || self.height < MIN_HEIGHT
    }

    /// Rows the current tab's list has on screen.
    pub fn body_rows(&self) -> usize {
        usize::from(super::view::list_area(self).height).max(1)
    }

    pub fn cursor(&self) -> Cursor {
        self.cursors[self.tab.index()]
    }

    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[self.tab.index()]
    }

    pub fn filter(&self) -> &Input {
        &self.filters[self.tab.index()]
    }

    fn filter_mut(&mut self) -> &mut Input {
        &mut self.filters[self.tab.index()]
    }

    /// How many held versions there are (the Review tab's badge).
    pub fn held_count(&self) -> usize {
        self.held.ready().map_or(0, Vec::len)
    }

    /// The scopes keys are looked up in, innermost first.
    pub fn scopes(&self) -> Vec<Scope> {
        match &self.overlay {
            Some(Overlay::Confirm { .. } | Overlay::ConfirmQuit) => vec![Scope::Confirm],
            Some(Overlay::Detail { .. }) => vec![Scope::Detail, Scope::Global],
            Some(Overlay::Help | Overlay::Error { .. }) => vec![Scope::Global],
            None => vec![Scope::Tab(self.tab), Scope::Global],
        }
    }

    /// The number of rows the current tab lists (for moving and clicking).
    pub fn list_len(&self) -> usize {
        super::view::rows(self).len()
    }

    fn toast(&mut self, text: impl Into<String>, sticky: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            until: (!sticky).then_some(self.now + TOAST_MS),
        });
    }

    fn go(&mut self, tab: Tab) {
        if tab != self.tab {
            self.tab = tab;
            self.tab_since = self.now;
        }
    }

    fn error(&mut self, e: &CliError) {
        let (what, cause) = match e.message.split_once('\n') {
            Some((w, c)) => (w.to_string(), Some(c.trim().to_string())),
            None => (e.message.clone(), None),
        };
        self.overlay = Some(Overlay::Error {
            what,
            cause,
            hint: e.hint.clone(),
        });
    }
}

/// Applies `msg`, returning what to run, and keeps count of the loads in flight (the status line
/// says so while there are any).
pub fn update(m: &mut Model, msg: Msg) -> Vec<Cmd> {
    let loaded = matches!(msg, Msg::Loaded(_));
    let cmds = apply(m, msg);
    if loaded {
        m.pending = m.pending.saturating_sub(1);
    }
    m.pending += cmds.iter().filter(|c| matches!(c, Cmd::Load(_))).count();
    cmds
}

fn apply(m: &mut Model, msg: Msg) -> Vec<Cmd> {
    // A tick or a stray mouse move changes nothing by itself; it must never hide a pending redraw.
    let quiet = match &msg {
        Msg::Tick(_) => true,
        Msg::Mouse(e) => matches!(
            e.kind,
            MouseEventKind::Moved | MouseEventKind::Drag(_) | MouseEventKind::Up(_)
        ),
        _ => false,
    };
    if !quiet {
        m.dirty = true;
    }
    match msg {
        Msg::Tick(now) => {
            m.now = now;
            if m.toast
                .as_ref()
                .and_then(|t| t.until)
                .is_some_and(|u| now >= u)
            {
                m.toast = None;
                m.dirty = true;
            }
            if m.job.is_some() || m.pending > 0 {
                m.dirty = true;
            }
            Vec::new()
        }
        Msg::Resize(w, h) => {
            m.width = w;
            m.height = h;
            let (len, rows) = (m.list_len(), m.body_rows());
            m.cursor_mut().fit(len, rows);
            Vec::new()
        }
        Msg::Loaded(l) => loaded(m, l),
        Msg::Job(e) => job_event(m, e),
        Msg::Paste(text) => {
            if m.filter().focused {
                m.filter_mut()
                    .text
                    .push_str(text.trim_end_matches(['\n', '\r']));
                *m.cursor_mut() = Cursor::default();
            }
            Vec::new()
        }
        Msg::Mouse(ev) => mouse(m, ev),
        Msg::Key(ev) => key(m, ev),
    }
}

fn loaded(m: &mut Model, l: Loaded) -> Vec<Cmd> {
    match l {
        Loaded::Snapshot(r) => {
            m.loads += 1;
            m.snapshot = match *r {
                Ok(s) => Data::Ready(s),
                Err(e) => Data::Failed(e),
            };
        }
        Loaded::Held(r) => {
            m.held = match r {
                Ok(h) => Data::Ready(h),
                Err(e) => Data::Failed(e),
            };
        }
        Loaded::Catalog(r) => {
            m.catalog = match r {
                Ok(c) => Data::Ready(c),
                Err(e) => Data::Failed(e),
            };
        }
        Loaded::Detail { title, lines } => match lines {
            Ok(lines) => {
                let held = (m.tab == Tab::Review).then(|| title.clone());
                m.overlay = Some(Overlay::Detail {
                    title,
                    lines,
                    scroll: 0,
                    held,
                });
            }
            Err(e) => m.error(&e),
        },
    }
    let (len, rows) = (m.list_len(), m.body_rows());
    m.cursor_mut().fit(len, rows);
    Vec::new()
}

fn job_event(m: &mut Model, e: JobEvent) -> Vec<Cmd> {
    let Some(run) = m.job.as_mut() else {
        return Vec::new();
    };
    let row = |run: &mut Running, name: &str| -> usize {
        match run.rows.iter().position(|r| r.name == name) {
            Some(i) => i,
            None => {
                run.rows.push(ProgressRow {
                    name: name.to_string(),
                    text: String::new(),
                    ok: None,
                });
                run.rows.len() - 1
            }
        }
    };
    match e {
        JobEvent::Step {
            profile,
            action,
            n,
            total,
        } => {
            let i = row(run, &profile);
            run.rows[i].text = format!("{action}  {n}/{total}");
        }
        JobEvent::Done {
            profile,
            summary,
            ok,
        } => {
            let i = row(run, &profile);
            run.rows[i].text = summary;
            run.rows[i].ok = Some(ok);
        }
        JobEvent::Note(_) | JobEvent::Warn(_) => {}
        JobEvent::Finished(result) => {
            m.job = None;
            match result {
                Ok(report) => {
                    // A run that didn't succeed stays until a key is pressed.
                    let text = report.verdict();
                    m.toast(text, report.exit != 0);
                }
                Err(e) => m.error(&e),
            }
            if m.quit_after_job {
                m.quit = true;
                return vec![Cmd::Quit];
            }
            return vec![
                Cmd::Load(Load::Snapshot),
                Cmd::Load(Load::Held),
                Cmd::Load(Load::Catalog),
            ];
        }
    }
    Vec::new()
}

/// Asks to run `job` after a confirmation, unless another is running (AC8-25, AC8-26).
pub fn ask(m: &mut Model, title: impl Into<String>, body: Vec<String>, job: Job) -> Vec<Cmd> {
    if let Some(run) = &m.job {
        let text = format!("Wait for {} to finish", run.job.command());
        m.toast(text, false);
        return Vec::new();
    }
    m.overlay = Some(Overlay::Confirm {
        title: title.into(),
        body,
        job,
    });
    Vec::new()
}

fn start(m: &mut Model, job: Job) -> Vec<Cmd> {
    m.job = Some(Running {
        job: job.clone(),
        rows: Vec::new(),
        started: m.now,
    });
    vec![Cmd::Run(job)]
}

fn key(m: &mut Model, ev: KeyEvent) -> Vec<Cmd> {
    if keys::is_interrupt(&ev) {
        return quit(m);
    }
    // A sticky toast (an error) goes on the next key.
    if m.toast.as_ref().is_some_and(|t| t.until.is_none()) {
        m.toast = None;
    }
    if m.overlay.is_none()
        && m.filter().focused
        && let Some(cmds) = input_key(m, ev)
    {
        return cmds;
    }
    let Some(action) = keys::lookup(&m.scopes(), &ev) else {
        return Vec::new();
    };
    act(m, action)
}

/// Keys for a focused input: text edits it; Esc clears and closes it; ⏎ and arrows leave it.
fn input_key(m: &mut Model, ev: KeyEvent) -> Option<Vec<Cmd>> {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    if ev
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    match ev.code {
        KeyCode::Char(c) => {
            m.filter_mut().text.push(c);
            *m.cursor_mut() = Cursor::default();
            Some(Vec::new())
        }
        KeyCode::Backspace => {
            m.filter_mut().text.pop();
            *m.cursor_mut() = Cursor::default();
            Some(Vec::new())
        }
        KeyCode::Esc => {
            *m.filter_mut() = Input::default();
            *m.cursor_mut() = Cursor::default();
            Some(Vec::new())
        }
        KeyCode::Enter | KeyCode::Down | KeyCode::Up | KeyCode::Tab | KeyCode::BackTab => {
            m.filter_mut().focused = false;
            match ev.code {
                KeyCode::Enter => Some(Vec::new()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn quit(m: &mut Model) -> Vec<Cmd> {
    if m.job.is_some() {
        m.overlay = Some(Overlay::ConfirmQuit);
        return Vec::new();
    }
    m.quit = true;
    vec![Cmd::Quit]
}

/// Runs an action from a key, a click on the key bar, or a mouse gesture.
pub fn act(m: &mut Model, action: Action) -> Vec<Cmd> {
    let (len, rows) = (m.list_len(), m.body_rows());
    match action {
        Action::Quit => return quit(m),
        Action::Help => m.overlay = Some(Overlay::Help),
        Action::NextTab => m.go(m.tab.step(1)),
        Action::PrevTab => m.go(m.tab.step(-1)),
        Action::GoTab(t) => m.go(t),
        Action::Up
        | Action::Down
        | Action::PageUp
        | Action::PageDown
        | Action::Top
        | Action::Bottom => {
            let page = rows.max(1) as isize;
            let by = match action {
                Action::Up => -1,
                Action::Down => 1,
                Action::PageUp => -page,
                Action::PageDown => page,
                Action::Top => isize::MIN / 2,
                _ => isize::MAX / 2,
            };
            if let Some(Overlay::Detail { lines, scroll, .. }) = &mut m.overlay {
                let max = lines.len().saturating_sub(1);
                *scroll = (*scroll as isize + by).clamp(0, max as isize) as usize;
            } else {
                m.cursor_mut().move_by(by, len, rows);
            }
        }
        Action::Back => match &m.overlay {
            Some(_) => m.overlay = None,
            None if m.job.is_some() => {
                let cancellable = matches!(m.job.as_ref().map(|j| &j.job), Some(Job::Update(_)));
                if cancellable {
                    m.toast("Stopping after this source", false);
                    return vec![Cmd::Cancel];
                }
                m.toast("Sync can't be stopped half way", false);
            }
            None if !m.filter().text.is_empty() => {
                *m.filter_mut() = Input::default();
                *m.cursor_mut() = Cursor::default();
            }
            None => {}
        },
        Action::Filter => {
            m.filter_mut().focused = true;
        }
        Action::Reload => {
            return vec![
                Cmd::Load(Load::Snapshot),
                Cmd::Load(Load::Held),
                Cmd::Load(Load::Catalog),
            ];
        }
        Action::Yes => match m.overlay.take() {
            Some(Overlay::Confirm { job, .. }) => return start(m, job),
            Some(Overlay::ConfirmQuit) => m.quit_after_job = true,
            other => m.overlay = other,
        },
        Action::No => m.overlay = None,
        Action::Open
        | Action::Sync
        | Action::Update
        | Action::Approve
        | Action::Reject
        | Action::ApproveAll
        | Action::Add
        | Action::NextFile
        | Action::PrevFile
        | Action::FullReview => return super::tabs::act(m, action),
    }
    Vec::new()
}

fn mouse(m: &mut Model, ev: MouseEvent) -> Vec<Cmd> {
    match ev.kind {
        MouseEventKind::ScrollDown => act(m, Action::Down),
        MouseEventKind::ScrollUp => act(m, Action::Up),
        MouseEventKind::Down(MouseButton::Left) => {
            let (x, y) = (ev.column, ev.row);
            if let Some(action) = super::view::key_at(m, x, y) {
                return act(m, action);
            }
            if m.overlay.is_some() {
                if !matches!(
                    m.overlay,
                    Some(Overlay::Confirm { .. } | Overlay::ConfirmQuit)
                ) && !super::view::in_overlay(m, x, y)
                {
                    m.overlay = None;
                }
                return Vec::new();
            }
            if let Some(tab) = super::view::tab_at(m, x, y) {
                m.go(tab);
                return Vec::new();
            }
            if let Some(row) = super::view::row_at(m, x, y) {
                let again = m.cursor().selected == row
                    && m.last_click.is_some_and(|t| m.now.saturating_sub(t) < 400);
                m.last_click = Some(m.now);
                let (len, rows) = (m.list_len(), m.body_rows());
                m.cursor_mut().selected = row;
                m.cursor_mut().fit(len, rows);
                if again {
                    return act(m, Action::Open);
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}
