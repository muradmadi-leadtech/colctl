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
use crate::ui::render::{self, Cell};
use crate::ui::style::Token;
use serde_json::{Value, json};

/// Said when no profile has any catalog (EC5-01).
fn no_catalogs_lines(ctx: &Ctx) -> Vec<String> {
    vec![
        render::verdict(&ctx.out.caps, Token::Attention, "No catalogs found", &[]),
        render::hint(
            &ctx.out.caps,
            "start Claude or Codex once, or add a marketplace to config.toml",
        ),
    ]
}

/// Reads every catalog and merges it into plugins; warnings go to stderr unless `quiet`.
fn plugins(ctx: &Ctx, quiet: bool) -> Result<(Catalog, Vec<Plugin>), CliError> {
    let ws = load(ctx)?;
    let catalog = catalog::build(&ws.config, &ctx.source);
    if !quiet {
        for w in &catalog.warnings {
            ctx.out.warn(&escape(w));
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

/// One aligned row per plugin (AC6-23): id, developer, description cut to fit, and whether it's
/// installed anywhere. Connectors without a description show their marketplace, and OpenAI's are
/// shown as by openai.
fn result_lines(ctx: &Ctx, results: &[Plugin]) -> Vec<String> {
    let rows: Vec<Vec<Cell>> = results
        .iter()
        .map(|p| {
            let connector = p.kind == "connector";
            let developer = if connector
                && p.developer == "unknown"
                && p.ids[0].ends_with("@openai-curated-remote")
            {
                "openai".to_string()
            } else {
                p.developer.clone()
            };
            let description = if p.description.is_empty() && connector {
                p.catalogs
                    .first()
                    .map(|c| c.name.clone())
                    .unwrap_or_default()
            } else {
                p.description
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            vec![
                if p.installed.is_empty() {
                    Cell::plain(" ")
                } else {
                    Cell::styled("✓", Token::Ok)
                },
                Cell::plain(escape(&p.ids[0])),
                Cell::styled(escape(&developer), Token::Dim),
                Cell::plain(escape(&description)),
            ]
        })
        .collect();
    render::table(&ctx.out.caps, 0, &rows, 3)
}

/// "✓ installed" in the footer when any listed plugin is installed.
fn legend(results: &[Plugin]) -> &'static str {
    if results.iter().any(|p| !p.installed.is_empty()) {
        "✓ installed"
    } else {
        ""
    }
}

/// The footer under a list: counts, and what to do next.
fn footer(ctx: &Ctx, shown: usize, matched: usize, noun: &str, next: &str, legend: &str) -> String {
    let mut text = format!("{shown} of {matched} {noun}");
    if !legend.is_empty() {
        text.push_str(&format!(" · {legend}"));
    }
    if shown < matched {
        text.push_str(" · --limit 0 shows all");
    }
    if !next.is_empty() {
        text.push_str(&format!(" · {next}"));
    }
    render::footer(&ctx.out.caps, &text)
}

fn results_json(matched: usize, results: &[Plugin], warnings: &[String]) -> Value {
    json!({
        "version": 1,
        "matched": matched,
        "results": results,
        "warnings": warnings,
    })
}

fn print_json(ctx: &Ctx, value: &Value) {
    ctx.out
        .line(serde_json::to_string_pretty(value).unwrap_or_default());
}

/// Prints search-shaped results, text or JSON, and returns the exit code (AC5-12, AC5-13, EC5-10).
fn report(
    ctx: &Ctx,
    catalog: &Catalog,
    shown: &[Plugin],
    matched: usize,
    what: &str,
    as_json: bool,
) -> i32 {
    if as_json {
        print_json(ctx, &results_json(matched, shown, &catalog.warnings));
    } else if no_catalogs(catalog) {
        ctx.out.lines(no_catalogs_lines(ctx));
    } else if matched == 0 {
        ctx.out.line(render::verdict(
            &ctx.out.caps,
            Token::Info,
            &format!("No plugins match {}", escape(what)),
            &[],
        ));
    } else {
        let mut lines = result_lines(ctx, shown);
        lines.push(String::new());
        lines.push(footer(
            ctx,
            shown.len(),
            matched,
            "results",
            "colctl show <id> for details",
            legend(shown),
        ));
        // AC6-20: a long list goes through the pager in a terminal.
        ctx.out.page(lines);
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
    Ok(report(ctx, &catalog, &shown, matched, &what, as_json))
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

/// `colctl show` (AC5-14).
pub fn show(ctx: &Ctx, what: Option<&str>, as_json: bool) -> Result<i32, CliError> {
    let (_, all) = plugins(ctx, as_json)?;
    // AC6-19: without an id, pick one in a terminal.
    let picked;
    let what = match what {
        Some(w) => w,
        None => {
            let labels: Vec<String> = all
                .iter()
                .map(|p| {
                    format!(
                        "{}  {}  {}",
                        escape(&p.ids[0]),
                        escape(&p.developer),
                        escape(&cut(&p.description, 50))
                    )
                })
                .collect();
            let Some(i) = (!as_json)
                .then(|| ctx.out.pick_one("Which plugin?", &labels))
                .flatten()
            else {
                return Err(CliError::usage("name a plugin id or name to show")
                    .hint("colctl search lists plugin ids"));
            };
            picked = all[i].ids[0].clone();
            picked.as_str()
        }
    };
    let p = find(&all, what)?;
    if as_json {
        print_json(ctx, &json!({"version": 1, "result": p}));
        return Ok(0);
    }
    // AC6-23: the name as a heading, then an aligned key–value block.
    let opt = |o: &Option<String>| escape(&o.clone().unwrap_or_default());
    let version = match (&p.version, &p.commit) {
        (Some(v), Some(c)) => format!("{v} (commit {})", crate::vault::short(c)),
        (Some(v), None) => v.clone(),
        (None, Some(c)) => format!("commit {}", crate::vault::short(c)),
        (None, None) => String::new(),
    };
    let catalogs = p
        .catalogs
        .iter()
        .map(|c| format!("{} ({})", c.name, c.tool))
        .collect::<Vec<_>>()
        .join(", ");
    let rows: Vec<(&str, String)> = vec![
        ("ids", escape(&p.ids.join(", "))),
        ("catalogs", escape(&catalogs)),
        ("developer", escape(&p.developer)),
        ("author", opt(&p.author)),
        ("description", escape(&p.description)),
        ("keywords", escape(&p.keywords.join(", "))),
        ("category", opt(&p.category)),
        ("homepage", opt(&p.homepage)),
        ("repo", opt(&p.repo)),
        ("subpath", opt(&p.subpath)),
        ("version", escape(&version)),
        (
            "installed",
            p.installed.iter().cloned().collect::<Vec<_>>().join(", "),
        ),
        ("add", escape(&p.add)),
    ];
    let mut lines = vec![render::heading(
        &ctx.out.caps,
        &escape(p.display_name.as_deref().unwrap_or(&p.name)),
    )];
    lines.extend(render::kv(&ctx.out.caps, 2, &rows));
    ctx.out.lines(lines);
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
            ctx,
            &catalog,
            &shown,
            matched,
            &format!("developer {name}"),
            as_json,
        ));
    }
    let list = search::developers(&all);
    if as_json {
        print_json(
            ctx,
            &json!({"version": 1, "developers": list, "warnings": catalog.warnings}),
        );
    } else if no_catalogs(&catalog) {
        ctx.out.lines(no_catalogs_lines(ctx));
    } else {
        let rows: Vec<Vec<Cell>> = list
            .iter()
            .map(|d| {
                vec![
                    Cell::styled(escape(&d.name), Token::Name),
                    Cell::plain(super::plural(d.plugins, "plugin")),
                    Cell::styled(escape(&d.catalogs.join(", ")), Token::Dim),
                ]
            })
            .collect();
        let mut lines = render::table(&ctx.out.caps, 0, &rows, 2);
        lines.push(String::new());
        lines.push(render::footer(
            &ctx.out.caps,
            &format!(
                "{} · colctl developers <name> lists one's plugins",
                super::plural(list.len(), "developer")
            ),
        ));
        ctx.out.page(lines);
    }
    Ok(if list.is_empty() { EXIT_DRIFT } else { 0 })
}

/// `colctl marketplace list` (AC5-19).
pub fn marketplace_list(ctx: &Ctx, as_json: bool) -> Result<i32, CliError> {
    let (catalog, _) = plugins(ctx, as_json)?;
    if as_json {
        print_json(
            ctx,
            &json!({
                "version": 1,
                "marketplaces": catalog.marketplaces,
                "warnings": catalog.warnings,
            }),
        );
    } else if catalog.marketplaces.is_empty() {
        ctx.out.lines(no_catalogs_lines(ctx));
    } else {
        // AC6-23: a table with a header row.
        let caps = &ctx.out.caps;
        let mut rows: Vec<Vec<Cell>> = vec![
            ["NAME", "TOOL", "SOURCE", "PLUGINS", "PROFILES", ""]
                .iter()
                .map(|h| Cell::styled(*h, Token::Dim))
                .collect(),
        ];
        for m in &catalog.marketplaces {
            rows.push(vec![
                Cell::styled(escape(&m.name), Token::Name),
                Cell::plain(m.tool.clone()),
                Cell::styled(
                    escape(&ctx.tilde(std::path::Path::new(&m.source))),
                    Token::Dim,
                ),
                Cell::plain(m.plugins.to_string()),
                Cell::plain(m.profiles.iter().cloned().collect::<Vec<_>>().join(", ")),
                if m.managed {
                    Cell::styled("✓ managed", Token::Ok)
                } else {
                    Cell::plain("")
                },
            ]);
        }
        ctx.out.lines(render::table(caps, 0, &rows, 2));
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
    let mut said: Vec<(Token, String)> = Vec::new();
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
                said.push((
                    Token::Attention,
                    format!("{}: skipped: {} isn't on PATH", h.name, tool.program()),
                ));
                continue;
            };
            let cmd = tool.command(ctx, program, h, &args);
            if dry_run {
                said.push((
                    Token::Info,
                    format!("{}: would run {}", h.name, cmd.display()),
                ));
                continue;
            }
            match cmd.run() {
                Ok(out) if out.success() => {
                    said.push((Token::Ok, format!("{}: {}", h.name, cmd.display())));
                }
                Ok(out) if out.timed_out => {
                    failed += 1;
                    said.push((
                        Token::Failed,
                        format!(
                            "{}: {} timed out after {}s",
                            h.name,
                            cmd.display(),
                            ctx.tool_timeout.as_secs()
                        ),
                    ));
                }
                Ok(out) => {
                    failed += 1;
                    said.push((
                        Token::Failed,
                        format!(
                            "{}: {}: {}",
                            h.name,
                            cmd.display(),
                            escape(out.stderr.trim())
                        ),
                    ));
                }
                Err(e) => {
                    failed += 1;
                    said.push((Token::Failed, format!("{}: {}: {e}", h.name, cmd.display())));
                }
            }
        }
    }
    let (tone, verdict) = if dry_run {
        (
            Token::Info,
            format!(
                "Would refresh the catalogs in {}",
                super::plural(said.len(), "profile")
            ),
        )
    } else if failed > 0 {
        (
            Token::Failed,
            format!("{} couldn't refresh", super::plural(failed, "profile")),
        )
    } else {
        (
            Token::Ok,
            format!(
                "Refreshed the catalogs in {}",
                super::plural(said.len(), "profile")
            ),
        )
    };
    ctx.out
        .lines(super::item_lines(ctx, tone, &verdict, &[], &said));
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
            ctx.out.warn(&escape(w));
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
        print_json(ctx, &results_json(matched, &shown, &warnings));
    } else if matched == 0 {
        ctx.out.line(render::verdict(
            &ctx.out.caps,
            Token::Info,
            &format!("No connectors match {}", escape(&terms.join(" "))),
            &[],
        ));
    } else {
        let mut lines = result_lines(ctx, &shown);
        lines.push(String::new());
        lines.push(footer(
            ctx,
            shown.len(),
            matched,
            "connectors",
            "installed through codex",
            legend(&shown),
        ));
        ctx.out.page(lines);
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
