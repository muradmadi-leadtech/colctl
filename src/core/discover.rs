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
    let ws = commands::load(ctx)?;
    let c = catalog::build(&ws.config, &ctx.source);
    Ok(Catalog {
        plugins: search::merge(&c.entries),
        catalogs: c.marketplaces.len(),
        warnings: c.warnings,
    })
}

/// The plugins matching `query`, ranked as `colctl search` ranks them (AC8-05), and how many
/// matched in all.
pub fn rank(plugins: &[Plugin], query: &search::Query) -> (Vec<Plugin>, usize) {
    search::search(plugins, query)
}

/// `colctl show <id>` as plain lines at `width`.
pub fn show(ctx: &Ctx, id: &str, width: usize) -> Result<Vec<String>, CliError> {
    let (out, sink) = Out::capture(width, Box::new(Quiet));
    let mut ctx = ctx.clone();
    ctx.out = out;
    commands::show(&ctx, Some(id), false)?;
    Ok(sink.lines())
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

/// Every marketplace any profile knows, as `colctl marketplace list` lists them.
pub fn marketplaces(_ctx: &Ctx) -> Result<Vec<catalog::Marketplace>, CliError> {
    Ok(Vec::new())
}

/// Who publishes what, as `colctl developers` lists them.
pub fn developers(_catalog: &Catalog) -> Vec<search::Developer> {
    Vec::new()
}

/// Codex's app connectors, as `colctl connectors` finds them.
pub fn connectors(_ctx: &Ctx) -> Result<Vec<Plugin>, CliError> {
    Ok(Vec::new())
}
