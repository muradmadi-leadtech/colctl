//! colctl keeps every AI coding harness on a machine in sync with one source of truth in `~/.colctl`.
//!
//! Modules follow the components in `docs/claude-profile-sync/components.md`.

pub mod config;
pub mod ctx;
pub mod error;
pub mod fsx;
pub mod git;
pub mod runner;
pub mod source;
pub mod state;
