//! Discovery (cycle 5): `search`, `show`, `developers`, `marketplace list|refresh`, `connectors`.
//! The read commands work offline from the catalogs the harnesses already keep, write nothing and
//! take no lock.

use super::{EXIT_DRIFT, load};
use crate::catalog::{self, Catalog, Entry};
use crate::config::Harness;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::review::escape;
use crate::runner;
use crate::search::{self, Plugin, Query};
use crate::tool::Tool;
use serde_json::{Value, json};

const NO_CATALOGS: &str =
    "no catalogs found; start Claude or Codex once, or add a marketplace to config.toml";

/// Reads every catalog and merges it into plugins; warnings go to stderr unless `quiet`.
fn plugins(ctx: &Ctx, quiet: bool) -> Result<(Catalog, Vec<Plugin>), CliError> {
    let ws = load(ctx)?;
    let catalog = catalog::build(&ws.config, &ctx.source);
    if !quiet {
        for w in &catalog.warnings {
            eprintln!("warning: {}", escape(w));
        }
    }
    let merged = search::merge(&catalog.entries);
    Ok((catalog, merged))
}

fn no_catalogs(c: &Catalog) -> bool {
    c.entries.is_empty() && c.marketplaces.is_empty()
}

/// Cuts `text` to `max` characters, ending in "…" when cut (EC5-14).
fn cut(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let mut out: String = one_line.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// One line per plugin: id, developer, description, and where it's installed (AC5-12).
fn print_lines(results: &[Plugin]) {
    let id_width = results
        .iter()
        .map(|p| p.ids[0].chars().count())
        .max()
        .unwrap_or(0)
        .min(56);
    let dev_width = results
        .iter()
        .map(|p| p.developer.chars().count())
        .max()
        .unwrap_or(0)
        .min(20);
    for p in results {
        let installed = if p.installed.is_empty() {
            String::new()
        } else {
            format!(
                "  [installed: {}]",
                p.installed.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        };
        println!(
            "{:<id_width$}  {:<dev_width$}  {}{installed}",
            escape(&p.ids[0]),
            escape(&p.developer),
            escape(&cut(&p.description, 60))
        );
    }
}

fn results_json(matched: usize, results: &[Plugin], warnings: &[String]) -> Value {
    json!({
        "version": 1,
        "matched": matched,
        "results": results,
        "warnings": warnings,
    })
}

fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// Prints search-shaped results, text or JSON, and returns the exit code (AC5-12, AC5-13, EC5-10).
fn report(catalog: &Catalog, shown: &[Plugin], matched: usize, what: &str, as_json: bool) -> i32 {
    if as_json {
        print_json(&results_json(matched, shown, &catalog.warnings));
    } else if no_catalogs(catalog) {
        println!("{NO_CATALOGS}");
    } else if matched == 0 {
        println!("no plugins match {}", escape(what));
    } else {
        print_lines(shown);
        let hidden = if shown.len() < matched {
            "; --limit 0 shows all"
        } else {
            ""
        };
        println!("{} of {matched} results{hidden}", shown.len());
    }
    if matched == 0 { EXIT_DRIFT } else { 0 }
}

/// `colctl search` (AC5-12…16).
pub fn search(ctx: &Ctx, query: &Query, as_json: bool) -> Result<i32, CliError> {
    let (catalog, all) = plugins(ctx, as_json)?;
    let (shown, matched) = search::search(&all, query);
    let what = if query.terms.is_empty() {
        "the filters".to_string()
    } else {
        query.terms.join(" ")
    };
    Ok(report(&catalog, &shown, matched, &what, as_json))
}

/// Finds one plugin by any of its ids, or by a name only one plugin has (AC5-14, EC5-11).
pub(super) fn find<'a>(all: &'a [Plugin], what: &str) -> Result<&'a Plugin, CliError> {
    if let Some(p) = all.iter().find(|p| p.ids.iter().any(|i| i == what)) {
        return Ok(p);
    }
    let named: Vec<&Plugin> = all
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(what))
        .collect();
    match named.as_slice() {
        [one] => Ok(one),
        [] => Err(CliError::usage(format!("no plugin {what}"))),
        many => Err(CliError::usage(format!(
            "{what} is ambiguous: {}",
            many.iter()
                .map(|p| p.ids[0].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn row(label: &str, value: &str) {
    if !value.is_empty() {
        println!("  {label:<12} {}", escape(value));
    }
}

/// `colctl show` (AC5-14).
pub fn show(ctx: &Ctx, what: &str, as_json: bool) -> Result<i32, CliError> {
    let (_, all) = plugins(ctx, as_json)?;
    let p = find(&all, what)?;
    if as_json {
        print_json(&json!({"version": 1, "result": p}));
        return Ok(0);
    }
    let opt = |o: &Option<String>| o.clone().unwrap_or_default();
    println!("{}", escape(p.display_name.as_deref().unwrap_or(&p.name)));
    row("ids", &p.ids.join(", "));
    row(
        "catalogs",
        &p.catalogs
            .iter()
            .map(|c| format!("{} ({})", c.name, c.tool))
            .collect::<Vec<_>>()
            .join(", "),
    );
    row("developer", &p.developer);
    row("author", &opt(&p.author));
    row("description", &p.description);
    row("keywords", &p.keywords.join(", "));
    row("category", &opt(&p.category));
    row("homepage", &opt(&p.homepage));
    row("repo", &opt(&p.repo));
    row("subpath", &opt(&p.subpath));
    let version = match (&p.version, &p.commit) {
        (Some(v), Some(c)) => format!("{v} (commit {})", crate::vault::short(c)),
        (Some(v), None) => v.clone(),
        (None, Some(c)) => format!("commit {}", crate::vault::short(c)),
        (None, None) => String::new(),
    };
    row("version", &version);
    row(
        "installed",
        &p.installed.iter().cloned().collect::<Vec<_>>().join(", "),
    );
    row("add", &p.add);
    Ok(0)
}

/// `colctl developers [<developer>]` (AC5-17, AC5-18).
pub fn developers(ctx: &Ctx, name: Option<&str>, as_json: bool) -> Result<i32, CliError> {
    let (catalog, all) = plugins(ctx, as_json)?;
    if let Some(name) = name {
        let query = Query {
            developer: Some(name.to_string()),
            ..Query::default()
        };
        let (shown, matched) = search::search(&all, &query);
        return Ok(report(
            &catalog,
            &shown,
            matched,
            &format!("developer {name}"),
            as_json,
        ));
    }
    let list = search::developers(&all);
    if as_json {
        print_json(&json!({"version": 1, "developers": list, "warnings": catalog.warnings}));
    } else if no_catalogs(&catalog) {
        println!("{NO_CATALOGS}");
    } else {
        let width = list
            .iter()
            .map(|d| d.name.chars().count())
            .max()
            .unwrap_or(0)
            .min(24);
        for d in &list {
            let word = if d.plugins == 1 { "plugin" } else { "plugins" };
            println!(
                "{:<width$}  {:>4} {word:<7}  {}",
                escape(&d.name),
                d.plugins,
                escape(&d.catalogs.join(", "))
            );
        }
    }
    Ok(if list.is_empty() { EXIT_DRIFT } else { 0 })
}

/// `colctl marketplace list` (AC5-19).
pub fn marketplace_list(ctx: &Ctx, as_json: bool) -> Result<i32, CliError> {
    let (catalog, _) = plugins(ctx, as_json)?;
    if as_json {
        print_json(&json!({
            "version": 1,
            "marketplaces": catalog.marketplaces,
            "warnings": catalog.warnings,
        }));
    } else if catalog.marketplaces.is_empty() {
        println!("{NO_CATALOGS}");
    } else {
        let width = |f: &dyn Fn(&catalog::Marketplace) -> usize| {
            catalog.marketplaces.iter().map(f).max().unwrap_or(0)
        };
        let name_w = width(&|m| m.name.chars().count());
        let source_w =
            width(&|m| ctx.tilde(std::path::Path::new(&m.source)).chars().count()).min(48);
        for m in &catalog.marketplaces {
            let word = if m.plugins == 1 { "plugin" } else { "plugins" };
            let managed = if m.managed { "   managed" } else { "" };
            println!(
                "{:<name_w$}  {:<6}  {:<source_w$}  {:>5} {word:<7}  {}{managed}",
                escape(&m.name),
                m.tool,
                escape(&ctx.tilde(std::path::Path::new(&m.source))),
                m.plugins,
                m.profiles.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    }
    Ok(if catalog.marketplaces.is_empty() {
        EXIT_DRIFT
    } else {
        0
    })
}

/// The harnesses a tool serves, whose folders exist.
fn harnesses_for(harnesses: &[Harness], tool: Tool) -> Vec<&Harness> {
    harnesses
        .iter()
        .filter(|h| Tool::for_kind(h.kind) == Some(tool) && h.path.is_dir())
        .collect()
}

/// `colctl marketplace refresh` (AC5-20, EC5-17): asks each tool to update its own catalogs.
pub fn marketplace_refresh(ctx: &Ctx, dry_run: bool) -> Result<i32, CliError> {
    let ws = load(ctx)?;
    let mut failed = 0;
    for (tool, args) in [
        (Tool::Claude, ["plugin", "marketplace", "update"]),
        (Tool::Codex, ["plugin", "marketplace", "upgrade"]),
    ] {
        let targets = harnesses_for(&ws.config.harnesses, tool);
        if targets.is_empty() {
            continue;
        }
        let program = runner::find_program(tool.program(), &ctx.path_var);
        for h in targets {
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let Some(program) = program.clone() else {
                failed += 1;
                println!("{}: skipped: {} isn't on PATH", h.name, tool.program());
                continue;
            };
            let cmd = tool.command(ctx, program, h, &args);
            if dry_run {
                println!("{}: would run {}", h.name, cmd.display());
                continue;
            }
            match cmd.run() {
                Ok(out) if out.success() => println!("{}: ✓ {}", h.name, cmd.display()),
                Ok(out) if out.timed_out => {
                    failed += 1;
                    println!(
                        "{}: ✗ {} timed out after {}s",
                        h.name,
                        cmd.display(),
                        ctx.tool_timeout.as_secs()
                    );
                }
                Ok(out) => {
                    failed += 1;
                    println!("{}: ✗ {}: {}", h.name, cmd.display(), out.stderr.trim());
                }
                Err(e) => {
                    failed += 1;
                    println!("{}: ✗ {}: {e}", h.name, cmd.display());
                }
            }
        }
    }
    Ok(if failed > 0 { EXIT_DRIFT } else { 0 })
}

/// `colctl connectors` (AC5-21, EC5-15, EC5-16): Codex's remote catalog, one call per profile,
/// in parallel.
pub fn connectors(
    ctx: &Ctx,
    terms: &[String],
    limit: usize,
    as_json: bool,
) -> Result<i32, CliError> {
    let ws = load(ctx)?;
    let program = runner::find_program("codex", &ctx.path_var)
        .ok_or_else(|| CliError::usage("connectors come from codex, and it isn't on PATH"))?;
    let targets = harnesses_for(&ws.config.harnesses, Tool::Codex);
    if targets.is_empty() {
        return Err(CliError::usage("no Codex profiles registered"));
    }
    let args = ["plugin", "list", "--available", "--json"].map(String::from);
    let results: Vec<(String, Result<String, String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .iter()
            .map(|h| {
                let cmd = Tool::Codex.command(ctx, program.clone(), h, &args);
                let name = h.name.clone();
                scope.spawn(move || {
                    let out = match cmd.run() {
                        Ok(out) if out.success() => Ok(out.stdout),
                        Ok(out) if out.timed_out => Err(format!("{} timed out", cmd.display())),
                        Ok(out) => Err(format!("{}: {}", cmd.display(), out.stderr.trim())),
                        Err(e) => Err(format!("{}: {e}", cmd.display())),
                    };
                    (name, out)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut entries: Vec<Entry> = Vec::new();
    let mut warnings = Vec::new();
    for (harness, out) in results {
        let parsed = out.and_then(|text| {
            let installed: Vec<String> = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("installed").cloned())
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    p.get("pluginId")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect();
            catalog::connectors(&text).map(|list| (list, installed))
        });
        match parsed {
            Ok((list, installed)) => {
                for mut e in list {
                    if installed.contains(&e.id) {
                        e.installed.insert(harness.clone());
                    }
                    match entries.iter_mut().find(|x| x.id == e.id) {
                        Some(x) => x.installed.extend(e.installed),
                        None => entries.push(e),
                    }
                }
            }
            Err(e) => warnings.push(format!("{harness}: {e}")),
        }
    }
    if !as_json {
        for w in &warnings {
            eprintln!("warning: {}", escape(w));
        }
    }
    let all = search::merge(&entries);
    let query = Query {
        terms: terms.to_vec(),
        limit,
        ..Query::default()
    };
    let (shown, matched) = search::search(&all, &query);
    if as_json {
        print_json(&results_json(matched, &shown, &warnings));
    } else if matched == 0 {
        println!("no connectors match {}", escape(&terms.join(" ")));
    } else {
        print_lines(&shown);
        let hidden = if shown.len() < matched {
            "; --limit 0 shows all"
        } else {
            ""
        };
        println!("{} of {matched} connectors{hidden}", shown.len());
    }
    Ok(if matched == 0 || !warnings.is_empty() {
        EXIT_DRIFT
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec5_14_long_descriptions_are_cut_to_the_line() {
        assert_eq!(cut("short", 10), "short");
        assert_eq!(cut("a  b\nc", 10), "a b c");
        let long = "x".repeat(100);
        let got = cut(&long, 60);
        assert_eq!(got.chars().count(), 60);
        assert!(got.ends_with('…'));
    }
}
