//! `colctl convert` (cycle 7, docs/convert/interfaces.md § Command line): builds a Codex package
//! from a source that only packages for Claude and holds it for review. Approving it (in
//! `quarantine.rs`) records `convert = ["codex"]`; sync installs it.

use super::{load, plugins_dir};
use crate::config::{self, Kind};
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::git::Git;
use crate::packaging;
use crate::plan::SourceTarget;
use crate::repackage::{self, Package};
use crate::review::escape;
use crate::state::{self, Candidate, HeldConversion, Lock};
use crate::tool::Tool;
use crate::ui::render;
use crate::ui::style::Token;
use crate::vault;
use serde_json::{Value, json};

/// The quarantine key of a source's held conversion.
pub(super) fn conversion_key(name: &str) -> String {
    format!("conversions/{name}")
}

/// The Codex package for a source at its pin, built from its clone (never written to).
pub(super) fn package_for(ctx: &Ctx, s: &config::Source) -> Result<Package, CliError> {
    let clone = plugins_dir(ctx).join(&s.name);
    let folder =
        packaging::claude_folder(&clone, &s.plugin, s.path.as_deref()).ok_or_else(|| {
            CliError::refused(format!("{} has no Claude packaging to convert", s.name))
        })?;
    repackage::build(&folder, &s.plugin).map_err(CliError::refused)
}

fn hook_text(h: &repackage::KeptHook) -> String {
    match &h.matcher {
        Some(m) if !m.is_empty() => format!("{} on {m}", h.event),
        _ => h.event.clone(),
    }
}

/// What a converted source's held commit means for Codex (AC7-15, AC7-16).
pub(super) enum ConversionChange {
    /// The commit packages for Codex itself; the conversion will stop.
    Native,
    /// The package that commit gives, and what changes against the installed one.
    Package {
        review: crate::review::Review,
        package: Box<Package>,
    },
}

/// Builds the Codex package of `rev` for a converted source, from a throwaway shared clone (the
/// source's clone isn't touched), and compares it with the installed package.
pub(super) fn change_at(
    ctx: &Ctx,
    git: &Git,
    s: &config::Source,
    rev: &str,
    with_diff: bool,
) -> Result<ConversionChange, String> {
    let err = |e: anyhow::Error| format!("{e:#}");
    let clone = plugins_dir(ctx).join(&s.name);
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let at = tmp.path().join("at");
    let at_str = at.to_string_lossy().into_owned();
    let clone_str = clone.to_string_lossy().into_owned();
    git.run(
        tmp.path(),
        &[
            "clone",
            "-q",
            "--shared",
            "--no-checkout",
            &clone_str,
            &at_str,
        ],
    )
    .map_err(err)?;
    git.run(&at, &["checkout", "-q", "--detach", rev])
        .map_err(err)?;
    let p = packaging::inspect_with(
        false,
        &at,
        &tmp.path().join("manifests"),
        &s.name,
        &s.plugin,
        s.path.as_deref(),
    );
    if matches!(p.codex, packaging::ToolPackaging::Supported(_)) {
        return Ok(ConversionChange::Native);
    }
    let folder = packaging::claude_folder(&at, &s.plugin, s.path.as_deref()).ok_or_else(|| {
        format!(
            "{} has no Claude packaging to convert at {}",
            s.name,
            vault::short(rev)
        )
    })?;
    let package = repackage::build(&folder, &s.plugin)?;
    let built = tmp.path().join("package");
    repackage::write(&package, &built)?;
    let installed = plugins_dir(ctx).join(super::CONVERTED).join(&s.name);
    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).map_err(|e| e.to_string())?;
    let installed = if installed.is_dir() { installed } else { empty };
    let review = crate::review::copies(git, tmp.path(), &installed, &built, true, with_diff)?;
    // The package's files are read from the throwaway clone; keep only what's been read.
    Ok(ConversionChange::Package {
        review,
        package: Box::new(package.detached()),
    })
}

/// The rows that sum a package up: what it carries, what Codex never runs, what isn't carried over.
pub(super) fn summary(pkg: &Package) -> Vec<(&'static str, String)> {
    let mut carries = super::plural(pkg.skills.len(), "skill");
    if !pkg.hooks.is_empty() {
        let list: Vec<String> = pkg.hooks.iter().map(hook_text).collect();
        carries.push_str(&format!(
            " · {} ({})",
            super::plural(pkg.hooks.len(), "hook"),
            list.join(", ")
        ));
    }
    let not_carried: Vec<String> = pkg
        .report
        .not_carried
        .iter()
        .map(|n| format!("{} {}", n.kind, n.name))
        .collect();
    vec![
        ("Carries", carries),
        ("Codex never runs", pkg.report.never_runs.join(" · ")),
        ("Not carried over", not_carried.join(" · ")),
    ]
}

/// Key-value rows whose values wrap under themselves; empty rows are left out.
pub(super) fn summary_lines(ctx: &Ctx, rows: &[(&str, String)]) -> Vec<String> {
    let caps = &ctx.out.caps;
    let rows: Vec<&(&str, String)> = rows.iter().filter(|(_, v)| !v.is_empty()).collect();
    let key_w = rows
        .iter()
        .map(|(k, _)| render::width(k))
        .max()
        .unwrap_or(0);
    let indent = 2 + key_w + 3;
    let mut lines = Vec::new();
    for (k, v) in rows {
        for (i, text) in render::wrap(&escape(v), caps.width, indent, indent)
            .into_iter()
            .enumerate()
        {
            if i == 0 {
                lines.push(format!(
                    "  {}{}   {text}",
                    render::footer(caps, k),
                    " ".repeat(key_w - render::width(k))
                ));
            } else {
                lines.push(text);
            }
        }
    }
    lines
}

/// A package's contents as JSON (skills, hooks, the report).
pub(super) fn package_json(pkg: &Package) -> Value {
    json!({
        "skills": pkg.skills,
        "hooks": pkg.hooks.iter().map(|h| json!({
            "event": h.event, "matcher": h.matcher, "type": h.kind, "command": h.command,
        })).collect::<Vec<_>>(),
        "never_runs": pkg.report.never_runs,
        "not_carried": pkg.report.not_carried.iter()
            .map(|n| json!({"kind": n.kind, "name": n.name})).collect::<Vec<_>>(),
    })
}

/// A one-line answer (nothing to do), as text or JSON.
fn said(ctx: &Ctx, as_json: bool, name: &str, tone: Token, text: &str) -> i32 {
    if as_json {
        ctx.out.raw(&format!(
            "{:#}\n",
            json!({"source": name, "held": false, "dry_run": false, "note": text})
        ));
    } else {
        ctx.out
            .line(render::verdict(&ctx.out.caps, tone, text, &[]));
    }
    0
}

/// `colctl convert --undo <source>` (AC7-11, AC7-12, EC7-14, EC7-15): drops a held conversion, or
/// stops converting and uninstalls the package from Codex profiles.
fn undo_convert(
    ctx: &Ctx,
    git: &Git,
    ws: &super::Workspace,
    s: &config::Source,
    dry_run: bool,
    as_json: bool,
) -> Result<i32, CliError> {
    let name = &s.name;
    let key = conversion_key(name);
    let held = ws.state.quarantine.contains_key(&key);
    let converted = s.convert.contains(&Tool::Codex);
    let json_out = |undone: bool| {
        ctx.out.raw(&format!(
            "{:#}\n",
            json!({"source": name, "undone": undone, "dry_run": dry_run})
        ));
    };
    if !held && !converted {
        if as_json {
            json_out(false);
        } else {
            ctx.out.line(render::verdict(
                &ctx.out.caps,
                Token::Info,
                &format!("{name} isn't converted"),
                &[],
            ));
        }
        return Ok(0);
    }
    if dry_run {
        if as_json {
            json_out(false);
        } else {
            let what = if converted {
                "uninstall it from Codex profiles and remove its package"
            } else {
                "drop its held package"
            };
            ctx.out.line(render::verdict(
                &ctx.out.caps,
                Token::Info,
                &format!("Would stop converting {name}"),
                &[what.to_string()],
            ));
        }
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    if held {
        let mut st = state::load(&ctx.source)?;
        st.quarantine.remove(&key);
        state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
        let _ = crate::fsx::remove_any(&state::snapshot_path(&ctx.source, &key));
    }
    let lead = vec![render::verdict(
        &ctx.out.caps,
        Token::Ok,
        &format!("Stopped converting {name}"),
        &[],
    )];
    if !converted {
        if as_json {
            json_out(true);
        } else {
            ctx.out.lines(lead);
        }
        return Ok(0);
    }
    super::stop_converting(ctx, git, name)?;
    if as_json {
        let quiet = Ctx {
            out: crate::ui::Out::new(&crate::ui::term::Settings {
                quiet: true,
                ..crate::ui::term::Settings::default()
            }),
            ..ctx.clone()
        };
        let code = super::run_sync_led(&quiet, git, None, &[])?;
        json_out(true);
        return Ok(code);
    }
    super::run_sync_led(ctx, git, None, &lead)
}

/// `colctl convert <source>` (AC7-07, AC7-12, EC7-01 – 05, 10, 11).
pub fn convert(
    ctx: &Ctx,
    name: &str,
    undo: bool,
    dry_run: bool,
    as_json: bool,
) -> Result<i32, CliError> {
    let _git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let Some(s) = ws.config.sources.iter().find(|s| s.name == name).cloned() else {
        return Err(CliError::usage(format!("no source named \"{name}\"")).hint("colctl status"));
    };
    if undo {
        return undo_convert(ctx, &_git, &ws, &s, dry_run, as_json);
    }
    if !ws.config.harnesses.iter().any(|h| h.kind == Kind::Codex) {
        return Err(CliError::refused("no Codex profile is registered").hint("colctl init"));
    }
    if s.convert.contains(&Tool::Codex) {
        return Ok(said(
            ctx,
            as_json,
            name,
            Token::Ok,
            &format!("{name} is already converted"),
        ));
    }
    match ws.sources.iter().find(|i| i.name == name).map(|i| &i.codex) {
        Some(SourceTarget::Ready(_)) => {
            return Ok(said(
                ctx,
                as_json,
                name,
                Token::Ok,
                &format!("{name} already packages for Codex"),
            ));
        }
        Some(SourceTarget::Pending(why)) => {
            return Err(CliError::refused(format!("{name}: {why}")).hint("colctl sync"));
        }
        _ => {}
    }
    let pkg = package_for(ctx, &s)?;
    if pkg.skills.is_empty() && pkg.hooks.is_empty() {
        return Err(CliError::refused(format!(
            "{name} has nothing Codex can use: no skills and no hook Codex runs"
        )));
    }
    let digest = pkg.digest();
    if !dry_run {
        let _lock = Lock::acquire(&ctx.source)?;
        let key = conversion_key(name);
        repackage::write(&pkg, &state::snapshot_path(&ctx.source, &key))
            .map_err(CliError::refused)?;
        let mut st = state::load(&ctx.source)?;
        st.quarantine.insert(
            key,
            Candidate::Conversion(HeldConversion {
                rev: s.rev.clone(),
                digest: digest.clone(),
                held_at: state::now(),
            }),
        );
        state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    }
    if as_json {
        let mut out = json!({
            "source": name, "held": !dry_run, "rev": s.rev, "digest": digest, "dry_run": dry_run,
        });
        if let (Value::Object(o), Value::Object(more)) = (&mut out, package_json(&pkg)) {
            o.extend(more);
        }
        ctx.out.raw(&format!("{out:#}\n"));
        return Ok(0);
    }
    let caps = &ctx.out.caps;
    let built = format!("built from {}", vault::short(&s.rev));
    let mut lines = vec![if dry_run {
        render::verdict(
            caps,
            Token::Info,
            &format!("Would hold {name}'s Codex package for review"),
            &[built],
        )
    } else {
        render::verdict(
            caps,
            Token::Attention,
            &format!("Held {name}'s Codex package for review"),
            &[built],
        )
    }];
    if !caps.quiet {
        lines.push(String::new());
        lines.extend(summary_lines(ctx, &summary(&pkg)));
        if !dry_run {
            lines.push(String::new());
            lines.push(render::hint(caps, &format!("colctl review {name}")));
        }
    }
    ctx.out.lines(lines);
    Ok(0)
}
