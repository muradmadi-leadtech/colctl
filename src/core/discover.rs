#![allow(unused_imports, unused_variables)]
//! The plugin catalogs (AC8-04): what `colctl search` searches and `colctl show` shows, read from
//! the files the tools keep on disk (no network).

use super::observer::Quiet;
use crate::catalog;
use crate::commands;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::search::{self, Plugin};
use crate::ui::Out;

/// Every plugin every catalog knows, merged.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Catalog {
    pub plugins: Vec<Plugin>,
    /// How many catalogs (marketplaces, listings) were read.
    pub catalogs: usize,
    /// Catalogs that couldn't be read, as `search` warns.
    pub warnings: Vec<String>,
}

/// Reads every catalog.
pub fn catalog(ctx: &Ctx) -> Result<Catalog, CliError> {
    let _ = ctx;
    Err(CliError::usage("not built yet"))
}

/// `colctl show <id>` as plain lines at `width`.
pub fn show(ctx: &Ctx, id: &str, width: usize) -> Result<Vec<String>, CliError> {
    let _ = ctx;
    Err(CliError::usage("not built yet"))
}

/// The command that adds `plugin`, as words after `colctl`, or why there's none (the reason
/// `show` gives).
pub fn add_words(plugin: &Plugin) -> Result<Vec<String>, String> {
    match plugin.add.strip_prefix("colctl ") {
        Some(rest) if rest.starts_with("add ") => {
            Ok(rest.split_whitespace().map(String::from).collect())
        }
        _ => Err(plugin.add.clone()),
    }
}
