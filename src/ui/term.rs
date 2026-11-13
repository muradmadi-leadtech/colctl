//! What the terminal can do, worked out once per run (interfaces.md § Terminal capabilities).

/// `--color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

/// The global flags that shape output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    pub color: ColorChoice,
    pub quiet: bool,
    pub verbose: bool,
    pub yes: bool,
    pub no_pager: bool,
    pub json: bool,
}

/// Facts about the process, gathered by [`Facts::detect`] or set by tests.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    /// The terminal's columns, when it reports them.
    pub columns: Option<usize>,
    pub no_color: Option<String>,
    pub clicolor_force: Option<String>,
    pub term: Option<String>,
    pub columns_env: Option<String>,
    pub ci: Option<String>,
}

impl Facts {
    /// Reads the real terminal and environment.
    pub fn detect() -> Self {
        let out = console::Term::stdout();
        let var = |k: &str| std::env::var(k).ok();
        Facts {
            stdin_tty: std::io::IsTerminal::is_terminal(&std::io::stdin()),
            stdout_tty: out.is_term(),
            stderr_tty: console::Term::stderr().is_term(),
            columns: out
                .size_checked()
                .or_else(|| console::Term::stderr().size_checked())
                .map(|(_, c)| usize::from(c)),
            no_color: var("NO_COLOR"),
            clicolor_force: var("CLICOLOR_FORCE"),
            term: var("TERM"),
            columns_env: var("COLUMNS"),
            ci: var("CI"),
        }
    }
}

/// What colctl may do with the terminal this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub color_out: bool,
    pub color_err: bool,
    pub width: usize,
    /// Live progress on stderr.
    pub progress: bool,
    /// Prompts and pickers.
    pub interactive: bool,
    /// Paging long output.
    pub pager: bool,
    pub quiet: bool,
    pub verbose: bool,
    pub yes: bool,
}

pub const MIN_WIDTH: usize = 40;
pub const MAX_WIDTH: usize = 160;
pub const DEFAULT_WIDTH: usize = 80;

fn set(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

impl Caps {
    /// The rules of interfaces.md § Terminal capabilities (AC6-03, AC6-05, EC6-01 – 06, EC6-16).
    pub fn new(s: &Settings, f: &Facts) -> Self {
        let dumb = f.term.as_deref() == Some("dumb");
        let forced = set(&f.clicolor_force) && f.clicolor_force.as_deref() != Some("0");
        let color = |tty: bool| match s.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => !s.json && !set(&f.no_color) && !dumb && (tty || forced),
        };
        let width = f
            .columns_env
            .as_deref()
            .and_then(|c| c.trim().parse::<usize>().ok())
            .or(f.columns)
            .unwrap_or(DEFAULT_WIDTH)
            .clamp(MIN_WIDTH, MAX_WIDTH);
        let ci = set(&f.ci);
        Caps {
            color_out: color(f.stdout_tty),
            color_err: color(f.stderr_tty),
            width,
            progress: f.stderr_tty && !s.quiet && !s.json && !ci && !dumb,
            interactive: f.stdin_tty && f.stdout_tty && !s.json && !ci,
            pager: f.stdout_tty && !s.no_pager && !s.json,
            quiet: s.quiet,
            verbose: s.verbose,
            yes: s.yes,
        }
    }

    /// Plain, 80 columns, nothing interactive: what a pipe gets, and what tests render with.
    pub fn plain() -> Self {
        Caps::new(&Settings::default(), &Facts::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tty() -> Facts {
        Facts {
            stdin_tty: true,
            stdout_tty: true,
            stderr_tty: true,
            columns: Some(100),
            ..Facts::default()
        }
    }

    #[test]
    fn ac6_03_colour_policy() {
        let auto = Settings::default();
        assert!(Caps::new(&auto, &tty()).color_out, "a terminal gets colour");
        assert!(
            !Caps::new(&auto, &Facts::default()).color_out,
            "a pipe doesn't"
        );
        let with = |f: fn(&mut Facts)| {
            let mut facts = tty();
            f(&mut facts);
            Caps::new(&auto, &facts)
        };
        assert!(!with(|f| f.no_color = Some("1".into())).color_out);
        assert!(!with(|f| f.term = Some("dumb".into())).color_out);
        let never = Settings {
            color: ColorChoice::Never,
            ..Settings::default()
        };
        assert!(!Caps::new(&never, &tty()).color_out);
        let always = Settings {
            color: ColorChoice::Always,
            ..Settings::default()
        };
        assert!(Caps::new(&always, &Facts::default()).color_out);
        let forced = Facts {
            clicolor_force: Some("1".into()),
            ..Facts::default()
        };
        assert!(
            Caps::new(&auto, &forced).color_out,
            "CLICOLOR_FORCE in a pipe"
        );
        // stdout and stderr are decided separately.
        let split = Facts {
            stderr_tty: true,
            ..Facts::default()
        };
        let c = Caps::new(&auto, &split);
        assert!(!c.color_out && c.color_err);
    }

    #[test]
    fn ec6_02_ec6_03_empty_no_color_and_clicolor_force_zero() {
        let auto = Settings::default();
        let mut f = tty();
        f.no_color = Some(String::new());
        assert!(Caps::new(&auto, &f).color_out);
        let f = Facts {
            clicolor_force: Some("0".into()),
            ..Facts::default()
        };
        assert!(!Caps::new(&auto, &f).color_out);
    }

    #[test]
    fn ac6_05_ec6_01_width() {
        let s = Settings::default();
        assert_eq!(Caps::new(&s, &Facts::default()).width, 80);
        assert_eq!(Caps::new(&s, &tty()).width, 100);
        let with_cols = |c: Option<usize>, env: Option<&str>| {
            Caps::new(
                &s,
                &Facts {
                    columns: c,
                    columns_env: env.map(str::to_string),
                    ..Facts::default()
                },
            )
            .width
        };
        assert_eq!(with_cols(Some(20), None), 40);
        assert_eq!(with_cols(Some(400), None), 160);
        assert_eq!(with_cols(Some(100), Some("120")), 120);
        assert_eq!(
            with_cols(Some(100), Some("wide")),
            100,
            "a bad COLUMNS is ignored"
        );
    }

    #[test]
    fn ec6_04_ec6_05_ec6_06_ec6_16_who_may_ask_and_draw() {
        let s = Settings::default();
        let piped_out = Facts {
            stdout_tty: false,
            ..tty()
        };
        let c = Caps::new(&s, &piped_out);
        assert!(!c.interactive && c.progress, "EC6-04");
        let no_stdin = Facts {
            stdin_tty: false,
            ..tty()
        };
        assert!(!Caps::new(&s, &no_stdin).interactive, "EC6-05");
        let ci = Facts {
            ci: Some("true".into()),
            ..tty()
        };
        let c = Caps::new(&s, &ci);
        assert!(!c.interactive && !c.progress, "EC6-06");
        let json = Settings {
            json: true,
            ..Settings::default()
        };
        let c = Caps::new(&json, &tty());
        assert!(
            !c.interactive && !c.progress && !c.pager && !c.color_out,
            "EC6-16"
        );
        let quiet = Settings {
            quiet: true,
            ..Settings::default()
        };
        assert!(!Caps::new(&quiet, &tty()).progress);
        assert!(Caps::new(&s, &tty()).pager);
        let no_pager = Settings {
            no_pager: true,
            ..Settings::default()
        };
        assert!(!Caps::new(&no_pager, &tty()).pager);
        assert_eq!(Caps::plain().width, 80);
        let d = Facts::detect();
        assert!(d.columns.is_none_or(|c| c > 0));
    }
}
