//! Errors that end a command with a specific exit code (see `docs/claude-profile-sync/interfaces.md`).

use std::fmt;

/// Exit code: out of sync, or an action failed or was refused.
pub const EXIT_DRIFT: i32 = 1;
/// Exit code: usage, config, state or environment error; nothing changed.
pub const EXIT_USAGE: i32 = 2;
/// Exit code: another colctl holds the lock.
pub const EXIT_LOCKED: i32 = 3;

/// An error that ends the command with `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub code: i32,
    pub message: String,
    /// What to do about it (AC6-06); a generic one is used when `None`.
    pub hint: Option<String>,
}

impl CliError {
    /// A usage, config, state or environment error (exit 2).
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_USAGE,
            message: message.into(),
            hint: None,
        }
    }

    /// The command can't do what was asked (exit 1).
    pub fn refused(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_DRIFT,
            message: message.into(),
            hint: None,
        }
    }

    /// The lock is held by another colctl (exit 3).
    pub fn locked(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_LOCKED,
            message: message.into(),
            hint: Some("wait for the other colctl to finish; a lock left by a crashed run is reclaimed automatically".into()),
        }
    }

    /// The same error, with what to do about it.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        Self::usage(format!("{err:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_set_exit_codes() {
        assert_eq!(CliError::usage("x").code, 2);
        assert_eq!(CliError::locked("x").code, 3);
        assert_eq!(CliError::usage("boom").to_string(), "boom");
        let from: CliError = anyhow::anyhow!("inner").context("outer").into();
        assert_eq!(from.code, 2);
        assert_eq!(from.message, "outer: inner");
        assert_eq!(
            CliError::usage("x").hint("do y").hint.as_deref(),
            Some("do y")
        );
        assert!(CliError::locked("x").hint.is_some());
    }
}
