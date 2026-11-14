//! The core API both front ends call (cycle 8, docs/tui/interfaces.md § Core API). Nothing here
//! prints, prompts, reads the terminal or exits: it returns what happened, and the CLI or the TUI
//! decides how it looks.

pub mod discover;
pub mod observer;
pub mod quarantine;
pub mod run;
pub mod status;

pub use observer::Observer;
