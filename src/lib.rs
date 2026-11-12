//! colctl keeps every AI coding harness on a machine in sync with one source of truth in `~/.colctl`.
//!
//! The binary is a thin wrapper around [`cli::run`]. Modules follow the components in
//! `docs/claude-profile-sync/components.md`.

pub mod adapter;
pub mod apply;
pub mod cli;
pub mod commands;
pub mod config;
pub mod convert;
pub mod ctx;
pub mod error;
pub mod fsx;
pub mod git;
pub mod packaging;
pub mod plan;
pub mod runner;
pub mod source;
pub mod state;
pub mod tool;
pub mod vault;
