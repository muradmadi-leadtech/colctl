//! What every profile has and what needs the person (AC8-01): the data behind `colctl status` and
//! the TUI's Overview, Profiles and Items tabs.

use crate::adapter::Root;
use crate::commands::quarantine::{self, View};
use crate::commands::{self, Planned, Workspace};
use crate::config::Kind;
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_DRIFT};
use crate::git::Git;
use crate::plan::{ItemKind, ItemState};
use crate::state::Candidate;
use crate::vault;
use std::path::PathBuf;

/// How a line reads: the state tokens of the output system (`ui::style::Token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Attention,
    Failed,
    Info,
}

/// The first line: what state things are in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Verdict {
    pub tone: Tone,
    pub text: String,
    pub facts: Vec<String>,
}

/// One item in one profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ItemRow {
    pub kind: String,
    pub name: String,
    pub state: String,
    pub tone: Tone,
    pub hint: Option<String>,
}

/// One registered profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Profile {
    pub name: String,
    pub kind: Kind,
    pub path: PathBuf,
    /// The path with the home folder written as `~`.
    pub display_path: String,
    /// False when the profile's folder is missing or isn't a folder.
    pub usable: bool,
    /// Why it isn't usable, and what to do: (`harness-missing`, hint).
    pub problem: Option<(String, String)>,
    pub mark: Tone,
    /// What it has, then its problems: ("32 skills", None), ("1 missing", Some(Attention))…
    /// `None` is plain text.
    pub contents: Vec<(String, Option<Tone>)>,
    pub items: Vec<ItemRow>,
}

/// Something that needs the person, with its fix.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Attention {
    /// A profile name, the source folder, or `held`.
    pub place: String,
    pub kind: String,
    pub name: String,
    pub what: String,
    /// The command that resolves it (may be empty).
    pub fix: String,
}

/// A version held for review.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Held {
    pub name: String,
    pub kind: String,
    /// `v1 → v2`, an origin path, or the conversion it is.
    pub change: String,
}

/// Everything `colctl status` knows, at one moment.
pub struct Snapshot {
    pub verdict: Verdict,
    pub profiles: Vec<Profile>,
    pub attention: Vec<Attention>,
    pub held: Vec<Held>,
    /// The number of declared source plugins.
    pub sources: usize,
    /// `colctl status`'s exit code for this state.
    pub exit: i32,
    /// What it was built from, for `status --json` (byte-identical to 2.3).
    pub(crate) inner: Inner,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("verdict", &self.verdict)
            .field("profiles", &self.profiles)
            .field("attention", &self.attention)
            .field("held", &self.held)
            .field("exit", &self.exit)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Inner {
    pub(crate) ws: Workspace,
    pub(crate) planned: Vec<Planned>,
    pub(crate) uncommitted: Vec<String>,
    pub(crate) held: Vec<View>,
}

/// Reads the source and every profile (or only `filter`). Never writes.
pub fn snapshot(ctx: &Ctx, filter: Option<&str>) -> Result<Snapshot, CliError> {
    let ws = commands::load(ctx)?;
    let planned = commands::plan_all(ctx, &ws, filter)?;
    let git = Git::find(&ctx.path_var).ok();
    let uncommitted = commands::uncommitted(ctx, git.as_ref());
    let held_views = quarantine::held_views(ctx, &ws, git.as_ref());
    let in_sync = commands::in_sync(&ws, &planned, &uncommitted);
    let attention = attention(ctx, &ws, &planned, &uncommitted, &held_views);
    let agents = planned
        .iter()
        .find(|p| p.harness.kind == Kind::Agents)
        .map(|p| ctx.tilde(&p.harness.path));
    let profiles: Vec<Profile> = planned
        .iter()
        .map(|p| profile(ctx, p, agents.as_deref()))
        .collect();
    let unusable = profiles.iter().filter(|p| !p.usable).count();
    let sources = ws.config.sources.len();
    let verdict = verdict(profiles.len(), attention.len() + unusable, sources);
    let held = held_views.iter().map(|v| held(ctx, v)).collect();
    Ok(Snapshot {
        verdict,
        profiles,
        attention,
        held,
        sources,
        exit: if in_sync && held_views.is_empty() {
            0
        } else {
            EXIT_DRIFT
        },
        inner: Inner {
            ws,
            planned,
            uncommitted,
            held: held_views,
        },
    })
}

fn verdict(profiles: usize, problems: usize, sources: usize) -> Verdict {
    if profiles == 0 {
        return Verdict {
            tone: Tone::Attention,
            text: "No profiles registered".into(),
            facts: Vec::new(),
        };
    }
    if problems == 0 {
        let text = if profiles == 1 {
            "The profile is in sync".to_string()
        } else {
            format!("All {profiles} profiles in sync")
        };
        let sources_fact = format!("{sources} source{}", if sources == 1 { "" } else { "s" });
        return Verdict {
            tone: Tone::Ok,
            text,
            facts: vec![sources_fact, "nothing held for review".into()],
        };
    }
    let word = if problems == 1 {
        "thing needs"
    } else {
        "things need"
    };
    Verdict {
        tone: Tone::Attention,
        text: format!("{problems} {word} attention"),
        facts: Vec::new(),
    }
}

/// How a state reads in status.
pub(crate) fn tone(state: ItemState) -> Tone {
    match state {
        ItemState::Ok => Tone::Ok,
        ItemState::Unsupported | ItemState::Unreviewed => Tone::Info,
        _ => Tone::Attention,
    }
}

fn profile(ctx: &Ctx, p: &Planned, agents: Option<&str>) -> Profile {
    let usable = p.plan.root == Root::Ok;
    let mark = if !usable {
        Tone::Failed
    } else if p
        .plan
        .items
        .iter()
        .any(|i| tone(i.state) == Tone::Attention)
    {
        Tone::Attention
    } else {
        Tone::Ok
    };
    Profile {
        name: p.harness.name.clone(),
        kind: p.harness.kind,
        path: p.harness.path.clone(),
        display_path: ctx.tilde(&p.harness.path),
        usable,
        problem: (!usable).then(|| {
            (
                commands::root_str(p.plan.root).to_string(),
                commands::root_hint(p.plan.root).to_string(),
            )
        }),
        mark,
        contents: if usable {
            contents(p, agents)
        } else {
            Vec::new()
        },
        items: p
            .plan
            .items
            .iter()
            .map(|i| ItemRow {
                kind: i.kind.as_str().to_string(),
                name: i.name.clone(),
                state: i.state.as_str().to_string(),
                tone: tone(i.state),
                hint: i.hint.clone(),
            })
            .collect(),
    }
}

/// What a profile has, by kind, then its problems (AC6-34): `32 skills · 1 plugin · CLAUDE.md · 1
/// missing`. A Codex profile reads skills from `~/.agents`, which is said when that's registered.
fn contents(p: &Planned, agents: Option<&str>) -> Vec<(String, Option<Tone>)> {
    let items = &p.plan.items;
    let present = |kinds: &[ItemKind]| {
        items
            .iter()
            .filter(|i| kinds.contains(&i.kind) && i.state != ItemState::Unsupported)
            .count()
    };
    let mut parts = Vec::new();
    for (kinds, word) in [
        (&[ItemKind::Skill][..], "skill"),
        (&[ItemKind::Agent][..], "agent"),
        (&[ItemKind::Plugin, ItemKind::Source][..], "plugin"),
    ] {
        let n = present(kinds);
        if n > 0 {
            parts.push((commands::plural(n, word), None));
        }
    }
    if let Some(i) = items
        .iter()
        .find(|i| i.kind == ItemKind::Instructions && i.state != ItemState::Unsupported)
    {
        parts.push((i.name.clone(), None));
    }
    if p.harness.kind == Kind::Codex
        && let Some(agents) = agents
    {
        parts.push((format!("skills via {agents}"), Some(Tone::Info)));
    }
    let order = [
        ItemState::Missing,
        ItemState::Broken,
        ItemState::Orphaned,
        ItemState::Foreign,
        ItemState::ForeignLink,
        ItemState::Edited,
        ItemState::Invalid,
        ItemState::PluginMissing,
        ItemState::MarketplaceMissing,
        ItemState::Outdated,
        ItemState::FromAggregator,
        ItemState::Unsupported,
        ItemState::Unreviewed,
    ];
    for state in order {
        let n = items.iter().filter(|i| i.state == state).count();
        if n == 0 {
            continue;
        }
        let words = match state {
            ItemState::Unsupported
                if items
                    .iter()
                    .all(|i| i.state != state || i.kind == ItemKind::Source) =>
            {
                "not packaged"
            }
            s => s.as_str(),
        };
        parts.push((format!("{n} {words}"), Some(tone(state))));
    }
    if parts.is_empty() {
        parts.push(("nothing yet".into(), Some(Tone::Info)));
    }
    parts
}

/// Everything that needs the person, each with its fix: problem items in usable profiles, uncommitted
/// files, invalid source items, changed origins and held versions. Names are as found: front ends
/// escape them.
pub(crate) fn attention(
    ctx: &Ctx,
    ws: &Workspace,
    planned: &[Planned],
    uncommitted: &[String],
    held: &[View],
) -> Vec<Attention> {
    let mut out: Vec<Attention> = Vec::new();
    let source = ctx.tilde(&ctx.source);
    for p in planned.iter().filter(|p| p.plan.root == Root::Ok) {
        for i in p
            .plan
            .items
            .iter()
            .filter(|i| tone(i.state) == Tone::Attention)
        {
            out.push(Attention {
                place: p.harness.name.clone(),
                kind: i.kind.as_str().to_string(),
                name: i.name.clone(),
                what: i.state.as_str().to_string(),
                fix: i.hint.clone().unwrap_or_default(),
            });
        }
    }
    for path in uncommitted {
        out.push(Attention {
            place: source.clone(),
            kind: "file".into(),
            name: path.clone(),
            what: "uncommitted".into(),
            fix: "colctl sync".into(),
        });
    }
    for i in &ws.source.invalid {
        out.push(Attention {
            place: source.clone(),
            kind: i.kind.to_string(),
            name: i.name.clone(),
            what: "invalid".into(),
            fix: i.reason.clone(),
        });
    }
    for (item, _) in &ws.origin_changed {
        out.push(Attention {
            place: source.clone(),
            kind: item
                .split('/')
                .next()
                .unwrap_or("item")
                .trim_end_matches('s')
                .into(),
            name: item.clone(),
            what: "origin-changed".into(),
            fix: format!("colctl refresh {item}"),
        });
    }
    for v in held {
        let h = self::held(ctx, v);
        out.push(Attention {
            place: "held".into(),
            kind: h.kind,
            fix: format!("colctl review {}", v.name),
            name: h.name,
            what: h.change,
        });
    }
    out
}

fn held(ctx: &Ctx, v: &View) -> Held {
    let change = match &v.candidate {
        Candidate::Source(c) => format!("{} → {}", vault::short(&c.from), vault::short(&c.to)),
        Candidate::Skill(c) | Candidate::Agent(c) => ctx.tilde(&c.origin),
        Candidate::Conversion(c) => format!("Codex package from {}", vault::short(&c.rev)),
    };
    Held {
        name: v.name.clone(),
        kind: v.candidate.kind().into(),
        change,
    }
}
