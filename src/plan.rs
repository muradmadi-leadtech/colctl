//! The planner: compares what a harness should have with what it has and returns each item's
//! state plus the ordered actions that would fix it. Pure: no I/O, so `--dry-run` is just printing
//! the plan.

use crate::adapter::codex::AGENTS_MD as CODEX_AGENTS_MD;
use crate::adapter::{CLAUDE_MD, Entry, Generated, Instructions, Root, Scan};
use crate::config::{Config, Harness, Kind, PluginList};
use crate::source::{AGENTS, AGENTS_MD, SKILLS, Source};
use crate::state::HarnessState;
use crate::tool::Tool;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Instructions,
    Skill,
    Agent,
    Marketplace,
    Plugin,
    /// A source plugin (cycle 3), named by its source.
    Source,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Instructions => "instructions",
            ItemKind::Skill => "skill",
            ItemKind::Agent => "agent",
            ItemKind::Marketplace => "marketplace",
            ItemKind::Plugin => "plugin",
            ItemKind::Source => "source",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    Ok,
    Missing,
    Broken,
    /// Owned by colctl but its source item is gone; sync removes it.
    Orphaned,
    Foreign,
    /// A symlink colctl doesn't own (EC-14, EC-31).
    ForeignLink,
    Edited,
    Invalid,
    PluginMissing,
    MarketplaceMissing,
    /// A source plugin installed at another commit than the pin.
    Outdated,
    /// The source's repo has no packaging for this harness's tool; informational.
    Unsupported,
    /// A plugin from a marketplace whose own source repo is known (AC3-23).
    FromAggregator,
}

impl ItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemState::Ok => "ok",
            ItemState::Missing => "missing",
            ItemState::Broken => "broken",
            ItemState::Orphaned => "orphaned",
            ItemState::Foreign => "foreign",
            ItemState::ForeignLink => "linked-elsewhere",
            ItemState::Edited => "edited",
            ItemState::Invalid => "invalid",
            ItemState::PluginMissing => "plugin-missing",
            ItemState::MarketplaceMissing => "marketplace-missing",
            ItemState::Outdated => "outdated",
            ItemState::Unsupported => "unsupported",
            ItemState::FromAggregator => "from-aggregator",
        }
    }
}

/// One item's state in one harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    /// Skill or agent name, plugin id, marketplace name, or the instructions file's name.
    pub name: String,
    pub state: ItemState,
    pub hint: Option<String>,
    /// How `adopt` and hints refer to it in its harness: `skills/foo`, `agents/foo.md` (Claude)
    /// or `agents/foo.toml` (Codex), `CLAUDE.md` / `AGENTS.md`, `plugin p@m`.
    pub address: String,
    /// For a `linked-elsewhere` item, where its link points.
    pub target: Option<PathBuf>,
}

impl Item {
    /// True for a link adopt can take over: a skill, or a Claude agent (AC2-25).
    pub fn can_take_over(&self) -> bool {
        self.kind == ItemKind::Skill
            || (self.kind == ItemKind::Agent && self.address.ends_with(".md"))
    }
}

impl Item {
    /// The item's address in a harness of `kind`.
    pub fn address_for(kind: ItemKind, harness: Kind, name: &str) -> String {
        match kind {
            ItemKind::Instructions => name.to_string(),
            ItemKind::Skill => format!("skills/{name}"),
            ItemKind::Agent if harness == Kind::Codex => format!("agents/{name}.toml"),
            ItemKind::Agent => format!("agents/{name}.md"),
            ItemKind::Plugin => format!("plugin {name}"),
            ItemKind::Marketplace => format!("marketplace {name}"),
            ItemKind::Source => format!("sources/{name}"),
        }
    }

    pub fn address(&self) -> String {
        self.address.clone()
    }
}

/// A change to one harness, executed by [`crate::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Remove a leftover temporary entry (EC-21).
    CleanupTemp(PathBuf),
    /// Create or replace the symlink `link` → `target`.
    Link {
        link: PathBuf,
        target: PathBuf,
    },
    /// Remove an owned symlink.
    Unlink(PathBuf),
    /// Write a file colctl generates (`CLAUDE.md`, a Codex agent); `rel` is its path in the harness.
    WriteGenerated {
        rel: String,
        path: PathBuf,
        content: Vec<u8>,
    },
    /// Remove a generated file.
    RemoveGenerated {
        rel: String,
        path: PathBuf,
    },
    /// Record the hash of a file that already has the generated content.
    RecordGenerated {
        rel: String,
        hash: String,
    },
    AddMarketplace {
        name: String,
        source: String,
    },
    RemoveMarketplace {
        name: String,
    },
    /// Drop a marketplace from the state; it's already gone from the harness.
    ForgetMarketplace {
        name: String,
    },
    Install {
        id: String,
    },
    Uninstall {
        id: String,
    },
    /// Drop a plugin from the state; it's already gone from the harness.
    ForgetPlugin {
        id: String,
    },
    /// Add a source plugin's local marketplace (not recorded as a list marketplace).
    AddSourceMarketplace {
        name: String,
        dir: PathBuf,
    },
    /// Install a source plugin; records it in the harness's `sources`.
    InstallSource {
        source: String,
        id: String,
        marketplace: String,
        rev: String,
    },
    /// Bring an installed source plugin to the pin.
    UpdateSource {
        source: String,
        id: String,
        marketplace: String,
        rev: String,
    },
    UninstallSource {
        source: String,
        id: String,
    },
    RemoveSourceMarketplace {
        name: String,
    },
    /// Drop a source from the state.
    ForgetSource {
        source: String,
    },
}

impl Action {
    /// A one-line description for output; plugin actions are shown as the `tool` command.
    pub fn describe(&self, tool: Option<Tool>) -> String {
        if let Some(t) = tool {
            let calls = t.argvs(self);
            if !calls.is_empty() {
                return calls
                    .iter()
                    .map(|a| format!("{} {}", t.program(), a.join(" ")))
                    .collect::<Vec<_>>()
                    .join(" && ");
            }
        }
        match self {
            Action::CleanupTemp(p) => format!("remove leftover {}", p.display()),
            Action::Link { link, target } => {
                format!("link {} -> {}", link.display(), target.display())
            }
            Action::Unlink(p) => format!("unlink {}", p.display()),
            Action::WriteGenerated { path, .. } => format!("write {}", path.display()),
            Action::RemoveGenerated { path, .. } => format!("remove {}", path.display()),
            Action::RecordGenerated { rel, .. } => format!("record {rel} as generated"),
            Action::AddMarketplace { source, .. } => format!("add marketplace {source}"),
            Action::RemoveMarketplace { name } => format!("remove marketplace {name}"),
            Action::ForgetMarketplace { name } => format!("forget marketplace {name}"),
            Action::Install { id } => format!("install plugin {id}"),
            Action::Uninstall { id } => format!("uninstall plugin {id}"),
            Action::ForgetPlugin { id } => format!("forget plugin {id}"),
            Action::AddSourceMarketplace { dir, .. } => {
                format!("add marketplace {}", dir.display())
            }
            Action::InstallSource { id, .. } => format!("install plugin {id}"),
            Action::UpdateSource { id, .. } => format!("update plugin {id}"),
            Action::UninstallSource { id, .. } => format!("uninstall plugin {id}"),
            Action::RemoveSourceMarketplace { name } => format!("remove marketplace {name}"),
            Action::ForgetSource { source } => format!("forget source {source}"),
        }
    }

    /// True for actions that run the harness's plugin CLI.
    pub fn needs_tool(&self) -> bool {
        matches!(
            self,
            Action::AddMarketplace { .. }
                | Action::RemoveMarketplace { .. }
                | Action::Install { .. }
                | Action::Uninstall { .. }
                | Action::AddSourceMarketplace { .. }
                | Action::InstallSource { .. }
                | Action::UpdateSource { .. }
                | Action::UninstallSource { .. }
                | Action::RemoveSourceMarketplace { .. }
        )
    }
}

/// The argument `<tool> plugin marketplace add` gets for a config source.
pub fn marketplace_arg(source: &str) -> &str {
    source.strip_prefix("github:").unwrap_or(source)
}

/// The plan for one harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessPlan {
    pub root: Root,
    pub items: Vec<Item>,
    pub actions: Vec<Action>,
}

impl HarnessPlan {
    /// True when every item is ok and the folder is usable.
    pub fn in_sync(&self) -> bool {
        self.root == Root::Ok
            && self
                .items
                .iter()
                .all(|i| matches!(i.state, ItemState::Ok | ItemState::Unsupported))
    }
}

/// Inputs that don't change between harnesses.
pub struct Inputs<'a> {
    pub config: &'a Config,
    pub source: &'a Source,
    pub source_dir: &'a std::path::Path,
    /// The source as written in hints and imports (`~/.colctl` or an absolute path).
    pub source_display: &'a str,
    /// The exact bytes of a generated `CLAUDE.md`.
    pub generated: &'a [u8],
    /// Each source agent rendered as Codex TOML, or why it can't be (AC2-08, AC2-11).
    pub codex_agents: &'a BTreeMap<String, Result<Vec<u8>, String>>,
    /// Source plugins and where each tool installs them from (cycle 3).
    pub sources: &'a [SourceInput],
}

/// Where one tool gets a source plugin from, as far as planning is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTarget {
    Ready(crate::packaging::Target),
    /// Not ready yet; sync will get it there (not cloned, clone not at the pin).
    Pending(String),
    Unsupported(String),
    Invalid(String),
}

/// A declared source plugin, prepared for planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInput {
    pub name: String,
    pub repo: String,
    pub rev: String,
    pub replaces: Vec<String>,
    pub claude: SourceTarget,
    pub codex: SourceTarget,
}

impl SourceInput {
    pub fn target(&self, kind: Kind) -> Option<&SourceTarget> {
        match kind {
            Kind::Claude => Some(&self.claude),
            Kind::Codex => Some(&self.codex),
            Kind::Agents => None,
        }
    }
}

fn hint(item: &Item, harness: &str, source_display: &str) -> Option<String> {
    let address = &item.address;
    Some(match item.state {
        ItemState::Ok => return None,
        ItemState::Missing
        | ItemState::Broken
        | ItemState::Orphaned
        | ItemState::PluginMissing
        | ItemState::MarketplaceMissing
        | ItemState::Outdated => "colctl sync".to_string(),
        ItemState::Unsupported => return None,
        ItemState::FromAggregator => format!(
            "source: {}; colctl adopt {harness} {address}",
            item.target
                .as_ref()
                .map(|t| t.display().to_string())
                .unwrap_or_default()
        ),
        ItemState::Foreign => format!("colctl adopt {harness} {address}"),
        ItemState::ForeignLink => match &item.target {
            Some(target) if item.can_take_over() => format!(
                "links to {}; colctl adopt {harness} {address}",
                target.display()
            ),
            _ => "remove or replace this link yourself".to_string(),
        },
        ItemState::Edited if item.kind == ItemKind::Agent => format!(
            "move your changes into {source_display}/agents/{}.md, delete {address}, run colctl sync",
            item.name
        ),
        ItemState::Edited => format!(
            "move your text into {source_display}/AGENTS.md, delete {address}, run colctl sync"
        ),
        ItemState::Invalid => return None,
    })
}

struct Builder<'a> {
    harness: &'a Harness,
    inputs: &'a Inputs<'a>,
    items: Vec<Item>,
    actions: Vec<Action>,
}

impl Builder<'_> {
    fn item(&mut self, kind: ItemKind, name: &str, state: ItemState) {
        let mut item = Item {
            kind,
            name: name.to_string(),
            state,
            hint: None,
            address: Item::address_for(kind, self.harness.kind, name),
            target: None,
        };
        item.hint = hint(&item, &self.harness.name, self.inputs.source_display);
        self.items.push(item);
    }

    fn linked_elsewhere(&mut self, kind: ItemKind, name: &str, target: &std::path::Path) {
        let mut item = Item {
            kind,
            name: name.to_string(),
            state: ItemState::ForeignLink,
            hint: None,
            address: Item::address_for(kind, self.harness.kind, name),
            target: Some(target.to_path_buf()),
        };
        item.hint = hint(&item, &self.harness.name, self.inputs.source_display);
        self.items.push(item);
    }

    fn invalid(&mut self, kind: ItemKind, name: &str, reason: &str) {
        self.items.push(Item {
            kind,
            name: name.to_string(),
            state: ItemState::Invalid,
            hint: Some(reason.to_string()),
            address: Item::address_for(kind, self.harness.kind, name),
            target: None,
        });
    }

    /// A Codex profile's `AGENTS.md`: a link to the source's (AC2-05, AC2-06).
    fn instructions_link(&mut self, scan: &Instructions) {
        let link = self.harness.path.join(CODEX_AGENTS_MD);
        let target = self.inputs.source_dir.join(AGENTS_MD);
        let kind = ItemKind::Instructions;
        let name = CODEX_AGENTS_MD;
        if self.harness.excludes(name) {
            if matches!(scan, Instructions::Linked(Entry::Owned { .. })) {
                self.actions.push(Action::Unlink(link));
            }
            return;
        }
        match (scan, self.inputs.source.agents_md) {
            (Instructions::Absent, true) => {
                self.item(kind, name, ItemState::Missing);
                self.actions.push(Action::Link { link, target });
            }
            (Instructions::Absent, false) => {}
            (Instructions::Linked(Entry::Owned { correct: true, .. }), true) => {
                self.item(kind, name, ItemState::Ok)
            }
            (Instructions::Linked(Entry::Owned { .. }), true) => {
                self.item(kind, name, ItemState::Broken);
                self.actions.push(Action::Link { link, target });
            }
            (Instructions::Linked(Entry::Owned { .. }), false) => {
                self.item(kind, name, ItemState::Orphaned);
                self.actions.push(Action::Unlink(link));
            }
            (Instructions::Linked(_) | Instructions::Link, _) => {
                self.item(kind, name, ItemState::ForeignLink)
            }
            (Instructions::File { .. }, _) => self.item(kind, name, ItemState::Foreign),
        }
    }

    /// A Codex profile's `agents/<name>.toml`, generated from the source agents (AC2-08…11).
    fn generated_agents(&mut self, scan: &BTreeMap<String, Generated>, state: &HarnessState) {
        let wanted = self.inputs.codex_agents;
        let names: BTreeSet<&String> = wanted.keys().chain(scan.keys()).collect();
        for name in names {
            let rel = format!("{AGENTS}/{name}.toml");
            let path = self.harness.path.join(&rel);
            let recorded = state.generated.get(&rel);
            let desired = wanted.get(name);
            let entry = scan.get(name);
            let owned = |hash: &String, content: &[u8]| {
                recorded == Some(hash) || matches!(desired, Some(Ok(b)) if b == content)
            };
            if self.harness.excludes(&rel) {
                if let Some(Generated::File { content, hash }) = entry
                    && owned(hash, content)
                {
                    self.actions.push(Action::RemoveGenerated { rel, path });
                }
                continue;
            }
            let kind = ItemKind::Agent;
            match (entry, desired) {
                (None, Some(Ok(content))) => {
                    self.item(kind, name, ItemState::Missing);
                    self.actions.push(Action::WriteGenerated {
                        rel,
                        path,
                        content: content.clone(),
                    });
                }
                (None, Some(Err(reason))) => self.invalid(kind, name, reason),
                (None, None) => {}
                (Some(Generated::Link(_)), _) => self.item(kind, name, ItemState::ForeignLink),
                (Some(Generated::File { content, hash }), desired) => {
                    if !owned(hash, content) {
                        let state = if recorded.is_some() {
                            ItemState::Edited
                        } else {
                            ItemState::Foreign
                        };
                        self.item(kind, name, state);
                        continue;
                    }
                    match desired {
                        Some(Ok(want)) if want == content => {
                            self.item(kind, name, ItemState::Ok);
                            if recorded != Some(hash) {
                                self.actions.push(Action::RecordGenerated {
                                    rel,
                                    hash: hash.clone(),
                                });
                            }
                        }
                        Some(Ok(want)) => {
                            self.item(kind, name, ItemState::Broken);
                            self.actions.push(Action::WriteGenerated {
                                rel,
                                path,
                                content: want.clone(),
                            });
                        }
                        Some(Err(reason)) => self.invalid(kind, name, reason),
                        None => {
                            self.item(kind, name, ItemState::Orphaned);
                            self.actions.push(Action::RemoveGenerated { rel, path });
                        }
                    }
                }
            }
        }
    }

    fn links(&mut self, kind: ItemKind, entries: &std::collections::BTreeMap<String, Entry>) {
        let (dir, suffix, wanted) = match kind {
            ItemKind::Skill => (SKILLS, "", &self.inputs.source.skills),
            _ => (AGENTS, ".md", &self.inputs.source.agents),
        };
        let names: BTreeSet<&String> = wanted.iter().chain(entries.keys()).collect();
        for name in names {
            let file = format!("{name}{suffix}");
            let link = self.harness.path.join(dir).join(&file);
            let target = self.inputs.source_dir.join(dir).join(&file);
            let entry = entries.get(name);
            if self.harness.excludes(&format!("{dir}/{file}")) {
                if matches!(entry, Some(Entry::Owned { .. })) {
                    self.actions.push(Action::Unlink(link));
                }
                continue;
            }
            let in_source = wanted.contains(name);
            match (entry, in_source) {
                (None, true) => {
                    self.item(kind, name, ItemState::Missing);
                    self.actions.push(Action::Link { link, target });
                }
                (None, false) => {}
                (Some(Entry::Owned { correct: true, .. }), _) => {
                    self.item(kind, name, ItemState::Ok)
                }
                (Some(Entry::Owned { .. }), true) => {
                    self.item(kind, name, ItemState::Broken);
                    self.actions.push(Action::Link { link, target });
                }
                (Some(Entry::Owned { .. }), false) => {
                    self.item(kind, name, ItemState::Orphaned);
                    self.actions.push(Action::Unlink(link));
                }
                (Some(Entry::ForeignLink(target)), _) => self.linked_elsewhere(kind, name, target),
                (Some(Entry::Real), _) => self.item(kind, name, ItemState::Foreign),
            }
        }
    }

    fn instructions(&mut self, scan: &Instructions, state: &HarnessState) {
        let path = self.harness.path.join(CLAUDE_MD);
        let recorded = state.generated.get(CLAUDE_MD);
        let generated = self.inputs.generated;
        if self.harness.excludes(CLAUDE_MD) {
            if let Instructions::File { hash, .. } = scan
                && recorded == Some(hash)
            {
                self.actions.push(Action::RemoveGenerated {
                    rel: CLAUDE_MD.into(),
                    path,
                });
            }
            return;
        }
        let kind = ItemKind::Instructions;
        match (scan, self.inputs.source.agents_md) {
            (Instructions::Absent, true) => {
                self.item(kind, CLAUDE_MD, ItemState::Missing);
                self.actions.push(Action::WriteGenerated {
                    rel: CLAUDE_MD.into(),
                    path,
                    content: generated.to_vec(),
                });
            }
            (Instructions::Absent, false) => {}
            (Instructions::Link | Instructions::Linked(_), _) => {
                self.item(kind, CLAUDE_MD, ItemState::ForeignLink)
            }
            (Instructions::File { content, hash }, true) => {
                if content == generated {
                    self.item(kind, CLAUDE_MD, ItemState::Ok);
                    if recorded != Some(hash) {
                        self.actions.push(Action::RecordGenerated {
                            rel: CLAUDE_MD.into(),
                            hash: hash.clone(),
                        });
                    }
                } else if recorded == Some(hash) {
                    self.item(kind, CLAUDE_MD, ItemState::Broken);
                    self.actions.push(Action::WriteGenerated {
                        rel: CLAUDE_MD.into(),
                        path,
                        content: generated.to_vec(),
                    });
                } else if recorded.is_some() {
                    self.item(kind, CLAUDE_MD, ItemState::Edited);
                } else {
                    self.item(kind, CLAUDE_MD, ItemState::Foreign);
                }
            }
            (Instructions::File { content, hash }, false) => {
                if recorded == Some(hash) || content == generated {
                    self.item(kind, CLAUDE_MD, ItemState::Orphaned);
                    self.actions.push(Action::RemoveGenerated {
                        rel: CLAUDE_MD.into(),
                        path,
                    });
                } else if recorded.is_some() {
                    self.item(kind, CLAUDE_MD, ItemState::Edited);
                } else {
                    self.item(kind, CLAUDE_MD, ItemState::Foreign);
                }
            }
        }
    }

    fn plugins(
        &mut self,
        scan: &Result<crate::adapter::Plugins, String>,
        state: &HarnessState,
        list: PluginList,
    ) {
        let plugins = match scan {
            Ok(p) => p,
            Err(reason) => {
                self.invalid(ItemKind::Plugin, "*", reason);
                return;
            }
        };
        let (config_markets, config_plugins) = self.inputs.config.plugin_list(list);
        let excluded = |id: &str| self.harness.excludes(&format!("plugins/{id}"));
        let wanted: Vec<&String> = config_plugins.iter().filter(|id| !excluded(id)).collect();
        let mut adds = Vec::new();
        for (name, source) in config_markets {
            if plugins.marketplaces.contains_key(name) {
                self.item(ItemKind::Marketplace, name, ItemState::Ok);
            } else {
                self.item(ItemKind::Marketplace, name, ItemState::MarketplaceMissing);
                adds.push(Action::AddMarketplace {
                    name: name.clone(),
                    source: source.clone(),
                });
            }
        }

        let mut installs = Vec::new();
        for id in &wanted {
            if plugins.installed.contains(*id) {
                self.item(ItemKind::Plugin, id, ItemState::Ok);
            } else {
                self.item(ItemKind::Plugin, id, ItemState::PluginMissing);
                installs.push(Action::Install { id: (*id).clone() });
            }
        }

        let mut removals = Vec::new();
        for id in &state.plugins {
            if wanted.contains(&id) {
                continue;
            }
            if plugins.installed.contains(id) {
                self.item(ItemKind::Plugin, id, ItemState::Orphaned);
                removals.push(Action::Uninstall { id: id.clone() });
            } else {
                removals.push(Action::ForgetPlugin { id: id.clone() });
            }
        }
        for name in &state.marketplaces {
            if config_markets.contains_key(name) {
                continue;
            }
            if plugins.marketplaces.contains_key(name) {
                self.item(ItemKind::Marketplace, name, ItemState::Orphaned);
                removals.push(Action::RemoveMarketplace { name: name.clone() });
            } else {
                removals.push(Action::ForgetMarketplace { name: name.clone() });
            }
        }

        let kind = self.harness.kind;
        let from_sources: BTreeSet<&String> = self
            .inputs
            .sources
            .iter()
            .flat_map(|s| {
                let target = match s.target(kind) {
                    Some(SourceTarget::Ready(t)) => Some(&t.id),
                    _ => None,
                };
                s.replaces.iter().chain(target)
            })
            .chain(state.sources.values().map(|s| &s.id))
            .collect();
        for id in &plugins.installed {
            let ours = config_plugins.contains(id) || state.plugins.contains(id);
            if ours || excluded(id) || from_sources.contains(id) {
                continue;
            }
            match plugins.origins.get(id) {
                Some(repo) => {
                    let mut item = Item {
                        kind: ItemKind::Plugin,
                        name: id.clone(),
                        state: ItemState::FromAggregator,
                        hint: None,
                        address: Item::address_for(ItemKind::Plugin, kind, id),
                        target: Some(PathBuf::from(repo)),
                    };
                    item.hint = hint(&item, &self.harness.name, self.inputs.source_display);
                    self.items.push(item);
                }
                None => self.item(ItemKind::Plugin, id, ItemState::Foreign),
            }
        }

        self.actions.extend(adds);
        self.actions.extend(installs);
        self.actions.extend(removals);
    }
}

impl Builder<'_> {
    /// Source plugins in a claude or codex harness (AC3-09…12).
    fn sources(
        &mut self,
        scan: &Result<crate::adapter::Plugins, String>,
        state: &HarnessState,
        list: PluginList,
    ) {
        let Ok(plugins) = scan else { return };
        let kind = self.harness.kind;
        let (list_markets, list_plugins) = self.inputs.config.plugin_list(list);
        let declared: BTreeSet<&str> = self
            .inputs
            .sources
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        for source in self.inputs.sources {
            if self.harness.excludes(&format!("sources/{}", source.name)) {
                continue;
            }
            let name = source.name.as_str();
            let target = match source.target(kind) {
                Some(SourceTarget::Ready(t)) => t,
                Some(SourceTarget::Unsupported(reason)) => {
                    let mut item = Item {
                        kind: ItemKind::Source,
                        name: name.to_string(),
                        state: ItemState::Unsupported,
                        hint: Some(reason.clone()),
                        address: Item::address_for(ItemKind::Source, kind, name),
                        target: None,
                    };
                    item.hint = Some(reason.clone());
                    self.items.push(item);
                    continue;
                }
                Some(SourceTarget::Invalid(reason)) => {
                    self.invalid(ItemKind::Source, name, reason);
                    continue;
                }
                Some(SourceTarget::Pending(reason)) => {
                    self.items.push(Item {
                        kind: ItemKind::Source,
                        name: name.to_string(),
                        state: ItemState::Missing,
                        hint: Some(reason.clone()),
                        address: Item::address_for(ItemKind::Source, kind, name),
                        target: None,
                    });
                    continue;
                }
                None => continue,
            };
            if list_plugins.contains(&target.id) {
                self.invalid(
                    ItemKind::Source,
                    name,
                    &format!("{} is both a source plugin and a list plugin", target.id),
                );
                continue;
            }
            // A replaced id equal to the source's own id is the aggregator's copy only until colctl has
            // installed its own; after that it's colctl's and stays.
            let own_id_taken_over = state.sources.get(name).is_some_and(|s| s.id == target.id);
            let own_id_replaced = source.replaces.contains(&target.id) && !own_id_taken_over;
            for id in &source.replaces {
                if plugins.installed.contains(id) && (id != &target.id || own_id_replaced) {
                    self.actions.push(Action::Uninstall { id: id.clone() });
                }
            }
            let market = &target.marketplace;
            if !plugins.marketplace_is(market, &target.marketplace_dir) {
                if plugins.marketplaces.contains_key(market) {
                    let ours_to_drop =
                        state.marketplaces.contains(market) && !list_markets.contains_key(market);
                    // The same marketplace served from its remote repo (e.g. adopting a plugin whose
                    // marketplace is the plugin's own repo): re-point it at the clone.
                    let same_source = plugins
                        .marketplace_repo(market, kind)
                        .is_some_and(|r| crate::config::same_repo(&r, &source.repo));
                    if same_source && !ours_to_drop {
                        self.actions.push(Action::RemoveSourceMarketplace {
                            name: market.clone(),
                        });
                    } else if !ours_to_drop {
                        self.invalid(
                            ItemKind::Source,
                            name,
                            &format!(
                                "marketplace {market} already comes from {}",
                                plugins.marketplace_origin(market)
                            ),
                        );
                        continue;
                    }
                }
                self.actions.push(Action::AddSourceMarketplace {
                    name: market.clone(),
                    dir: target.marketplace_dir.clone(),
                });
            }
            let recorded = state.sources.get(name).map(|s| s.rev.as_str());
            // The commit a tool records is the marketplace folder's. That's the clone's only when the
            // repo's own manifest is used; a colctl-written manifest sits in ~/.colctl's repo instead.
            let wrapped = target
                .marketplace_dir
                .components()
                .any(|c| c.as_os_str() == crate::commands::MANIFESTS);
            let tool_rev = if wrapped {
                None
            } else {
                plugins.revs.get(&target.id).map(String::as_str)
            };
            let installed_rev = tool_rev.or(recorded);
            let record = |rev: &str| {
                (
                    name.to_string(),
                    target.id.clone(),
                    market.clone(),
                    rev.to_string(),
                )
            };
            let (s_name, id, marketplace, rev) = record(&source.rev);
            // A replaced id equal to the source's own id is uninstalled above, so it needs installing.
            if !plugins.installed.contains(&target.id) || own_id_replaced {
                self.item(ItemKind::Source, name, ItemState::Missing);
                self.actions.push(Action::InstallSource {
                    source: s_name,
                    id,
                    marketplace,
                    rev,
                });
            } else if installed_rev == Some(source.rev.as_str()) {
                self.item(ItemKind::Source, name, ItemState::Ok);
                if recorded != Some(source.rev.as_str()) {
                    // Installed at the pin but not recorded: record it with a no-op install.
                    self.actions.push(Action::InstallSource {
                        source: s_name,
                        id,
                        marketplace,
                        rev,
                    });
                }
            } else {
                self.item(ItemKind::Source, name, ItemState::Outdated);
                self.actions.push(Action::UpdateSource {
                    source: s_name,
                    id,
                    marketplace,
                    rev,
                });
            }
        }
        for (name, installed) in &state.sources {
            if declared.contains(name.as_str()) {
                continue;
            }
            if plugins.installed.contains(&installed.id) {
                self.item(ItemKind::Source, name, ItemState::Orphaned);
                self.actions.push(Action::UninstallSource {
                    source: name.clone(),
                    id: installed.id.clone(),
                });
            }
            if plugins.marketplaces.contains_key(&installed.marketplace) {
                self.actions.push(Action::RemoveSourceMarketplace {
                    name: installed.marketplace.clone(),
                });
            }
            self.actions.push(Action::ForgetSource {
                source: name.clone(),
            });
        }
    }
}

/// Plans one harness.
pub fn plan(harness: &Harness, scan: &Scan, state: &HarnessState, inputs: &Inputs) -> HarnessPlan {
    let mut b = Builder {
        harness,
        inputs,
        items: Vec::new(),
        actions: Vec::new(),
    };
    if scan.root == Root::Ok {
        b.actions
            .extend(scan.temps.iter().cloned().map(Action::CleanupTemp));
        match harness.kind {
            Kind::Claude => {
                b.instructions(&scan.instructions, state);
                b.links(ItemKind::Skill, &scan.skills);
                b.links(ItemKind::Agent, &scan.agents);
                b.plugins(&scan.plugins, state, PluginList::Claude);
                b.sources(&scan.plugins, state, PluginList::Claude);
            }
            Kind::Codex => {
                b.instructions_link(&scan.instructions);
                b.generated_agents(&scan.generated, state);
                b.plugins(&scan.plugins, state, PluginList::Codex);
                b.sources(&scan.plugins, state, PluginList::Codex);
            }
            Kind::Agents => b.links(ItemKind::Skill, &scan.skills),
        }
    }
    HarnessPlan {
        root: scan.root,
        items: b.items,
        actions: b.actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Plugins;
    use crate::config::Kind;
    use crate::fsx::hash_bytes;
    use std::collections::BTreeMap;
    use std::path::Path;

    const GEN: &[u8] = b"<!-- generated -->\n@~/.colctl/AGENTS.md\n";

    fn harness(exclude: &[&str]) -> Harness {
        Harness {
            name: "h".into(),
            kind: Kind::Claude,
            path: PathBuf::from("/h"),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn scan() -> Scan {
        Scan {
            root: Root::Ok,
            instructions: Instructions::Absent,
            skills: BTreeMap::new(),
            agents: BTreeMap::new(),
            generated: BTreeMap::new(),
            plugins: Ok(Plugins::default()),
            temps: Vec::new(),
        }
    }

    fn source(skills: &[&str], agents: &[&str], agents_md: bool) -> Source {
        Source {
            agents_md,
            skills: skills.iter().map(|s| s.to_string()).collect(),
            agents: agents.iter().map(|s| s.to_string()).collect(),
            invalid: Vec::new(),
        }
    }

    fn run(h: &Harness, s: &Scan, st: &HarnessState, config: &Config, src: &Source) -> HarnessPlan {
        let inputs = Inputs {
            config,
            source: src,
            source_dir: Path::new("/src"),
            source_display: "~/.colctl",
            generated: GEN,
            codex_agents: &BTreeMap::new(),
            sources: &[],
        };
        plan(h, s, st, &inputs)
    }

    fn states(p: &HarnessPlan) -> Vec<String> {
        p.items
            .iter()
            .map(|i| format!("{} {} {}", i.kind.as_str(), i.name, i.state.as_str()))
            .collect()
    }

    fn owned(target: &str, correct: bool) -> Entry {
        Entry::Owned {
            target: PathBuf::from(target),
            correct,
        }
    }

    #[test]
    fn ac_09_missing_skill_is_linked() {
        let p = run(
            &harness(&[]),
            &scan(),
            &HarnessState::default(),
            &Config::default(),
            &source(&["foo"], &[], false),
        );
        assert_eq!(states(&p), vec!["skill foo missing"]);
        assert_eq!(
            p.actions,
            vec![Action::Link {
                link: PathBuf::from("/h/skills/foo"),
                target: PathBuf::from("/src/skills/foo")
            }]
        );
        assert_eq!(p.items[0].hint.as_deref(), Some("colctl sync"));
        assert!(!p.in_sync());
    }

    #[test]
    fn ac_10_missing_agent_is_linked() {
        let p = run(
            &harness(&[]),
            &scan(),
            &HarnessState::default(),
            &Config::default(),
            &source(&[], &["rev"], false),
        );
        assert_eq!(
            p.actions,
            vec![Action::Link {
                link: PathBuf::from("/h/agents/rev.md"),
                target: PathBuf::from("/src/agents/rev.md")
            }]
        );
        assert_eq!(p.items[0].address(), "agents/rev.md");
    }

    #[test]
    fn ac_11_claude_md_is_generated() {
        let p = run(
            &harness(&[]),
            &scan(),
            &HarnessState::default(),
            &Config::default(),
            &source(&[], &[], true),
        );
        assert_eq!(states(&p), vec!["instructions CLAUDE.md missing"]);
        assert_eq!(
            p.actions,
            vec![Action::WriteGenerated {
                rel: CLAUDE_MD.into(),
                path: PathBuf::from("/h/CLAUDE.md"),
                content: GEN.to_vec()
            }]
        );
    }

    #[test]
    fn generated_claude_md_states() {
        let src = source(&[], &[], true);
        let mut s = scan();
        let hash = hash_bytes(GEN);
        s.instructions = Instructions::File {
            content: GEN.to_vec(),
            hash: hash.clone(),
        };
        let mut st = HarnessState::default();
        // Content matches but isn't recorded yet: ok, and recorded.
        let p = run(&harness(&[]), &s, &st, &Config::default(), &src);
        assert_eq!(states(&p), vec!["instructions CLAUDE.md ok"]);
        assert_eq!(
            p.actions,
            vec![Action::RecordGenerated {
                rel: CLAUDE_MD.into(),
                hash: hash.clone()
            }]
        );
        st.generated.insert(CLAUDE_MD.into(), hash.clone());
        let p = run(&harness(&[]), &s, &st, &Config::default(), &src);
        assert!(p.actions.is_empty() && p.in_sync());

        // Owned but stale (e.g. the source moved): rewritten.
        let old = b"old generated".to_vec();
        st.generated.insert(CLAUDE_MD.into(), hash_bytes(&old));
        s.instructions = Instructions::File {
            hash: hash_bytes(&old),
            content: old,
        };
        let p = run(&harness(&[]), &s, &st, &Config::default(), &src);
        assert_eq!(states(&p), vec!["instructions CLAUDE.md broken"]);
    }

    #[test]
    fn ec_15_edited_claude_md_is_left_alone() {
        let mut s = scan();
        s.instructions = Instructions::File {
            content: b"mine".to_vec(),
            hash: hash_bytes(b"mine"),
        };
        let mut st = HarnessState::default();
        st.generated.insert(CLAUDE_MD.into(), hash_bytes(GEN));
        for agents_md in [true, false] {
            let p = run(
                &harness(&[]),
                &s,
                &st,
                &Config::default(),
                &source(&[], &[], agents_md),
            );
            assert_eq!(states(&p), vec!["instructions CLAUDE.md edited"]);
            assert!(p.actions.is_empty());
            assert!(
                p.items[0]
                    .hint
                    .as_ref()
                    .unwrap()
                    .contains("~/.colctl/AGENTS.md")
            );
        }
    }

    #[test]
    fn ec_04_foreign_and_orphaned_claude_md() {
        let mut s = scan();
        s.instructions = Instructions::File {
            content: b"mine".to_vec(),
            hash: hash_bytes(b"mine"),
        };
        for agents_md in [true, false] {
            let p = run(
                &harness(&[]),
                &s,
                &HarnessState::default(),
                &Config::default(),
                &source(&[], &[], agents_md),
            );
            assert_eq!(states(&p), vec!["instructions CLAUDE.md foreign"]);
            assert_eq!(p.items[0].hint.as_deref(), Some("colctl adopt h CLAUDE.md"));
        }
        // AC-14: generated CLAUDE.md and AGENTS.md removed from the source.
        s.instructions = Instructions::File {
            content: GEN.to_vec(),
            hash: hash_bytes(GEN),
        };
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&[], &[], false),
        );
        assert_eq!(states(&p), vec!["instructions CLAUDE.md orphaned"]);
        assert_eq!(
            p.actions,
            vec![Action::RemoveGenerated {
                rel: CLAUDE_MD.into(),
                path: PathBuf::from("/h/CLAUDE.md")
            }]
        );
        // A symlinked CLAUDE.md is foreign (EC-31).
        s.instructions = Instructions::Link;
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&[], &[], true),
        );
        assert_eq!(p.items[0].state, ItemState::ForeignLink);
        assert_eq!(
            p.items[0].hint.as_deref(),
            Some("remove or replace this link yourself")
        );
    }

    #[test]
    fn ac_13_ac_14_broken_and_orphaned_links() {
        let mut s = scan();
        s.skills
            .insert("foo".into(), owned("/src/skills/elsewhere", false));
        s.skills
            .insert("gone".into(), owned("/src/skills/gone", false));
        s.skills.insert("ok".into(), owned("/src/skills/ok", true));
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&["foo", "ok"], &[], false),
        );
        assert_eq!(
            states(&p),
            vec!["skill foo broken", "skill gone orphaned", "skill ok ok"]
        );
        assert_eq!(
            p.actions,
            vec![
                Action::Link {
                    link: "/h/skills/foo".into(),
                    target: "/src/skills/foo".into()
                },
                Action::Unlink("/h/skills/gone".into()),
            ]
        );
    }

    #[test]
    fn ec_13_dangling_owned_link_without_source_is_removed() {
        let mut s = scan();
        s.agents
            .insert("old".into(), owned("/src/agents/old.md", false));
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&[], &[], false),
        );
        assert_eq!(p.actions, vec![Action::Unlink("/h/agents/old.md".into())]);
    }

    #[test]
    fn ac_15_ec_14_foreign_items_get_no_actions() {
        let mut s = scan();
        s.skills.insert("foo".into(), Entry::Real);
        s.skills
            .insert("mine".into(), Entry::ForeignLink("/x".into()));
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&["foo", "mine"], &[], false),
        );
        // AC2-24: a link into another folder is "linked-elsewhere" (cycle 1 called it foreign).
        assert_eq!(
            states(&p),
            vec!["skill foo foreign", "skill mine linked-elsewhere"]
        );
        assert_eq!(
            p.items[1].hint.as_deref(),
            Some("links to /x; colctl adopt h skills/mine")
        );
        assert!(p.actions.is_empty());
        assert_eq!(
            p.items[0].hint.as_deref(),
            Some("colctl adopt h skills/foo")
        );
    }

    #[test]
    fn ec_17_links_are_planned_even_without_folders() {
        // The harness has no skills/ folder: the plan is the same, apply creates the folder.
        let p = run(
            &harness(&[]),
            &scan(),
            &HarnessState::default(),
            &Config::default(),
            &source(&["a"], &[], false),
        );
        assert!(matches!(p.actions[0], Action::Link { .. }));
    }

    #[test]
    fn ec_18_excluded_items_are_silent_and_owned_ones_removed() {
        let mut s = scan();
        s.skills
            .insert("foo".into(), owned("/src/skills/foo", true));
        s.skills.insert("bar".into(), Entry::Real);
        s.instructions = Instructions::File {
            content: GEN.to_vec(),
            hash: hash_bytes(GEN),
        };
        let mut st = HarnessState::default();
        st.generated.insert(CLAUDE_MD.into(), hash_bytes(GEN));
        let mut config = Config::default();
        config.plugins.push("p@anthropic-plugin-directory".into());
        let h = harness(&[
            "skills/foo",
            "skills/bar",
            "CLAUDE.md",
            "plugins/p@anthropic-plugin-directory",
        ]);
        let p = run(
            &h,
            &s,
            &st,
            &config,
            &source(&["foo", "bar", "baz"], &[], true),
        );
        assert_eq!(states(&p), vec!["skill baz missing"]);
        assert_eq!(
            p.actions[0],
            Action::RemoveGenerated {
                rel: CLAUDE_MD.into(),
                path: "/h/CLAUDE.md".into()
            }
        );
        assert!(p.actions.contains(&Action::Unlink("/h/skills/foo".into())));
        assert_eq!(p.actions.len(), 3, "{:?}", p.actions);
    }

    fn plugin_config() -> Config {
        let mut c = Config::default();
        c.marketplaces
            .insert("sadr".into(), "github:muradmadi/sadr".into());
        c.plugins = vec![
            "sadr@sadr".into(),
            "superpowers@anthropic-plugin-directory".into(),
        ];
        c
    }

    #[test]
    fn ac_12_marketplaces_then_plugins() {
        let p = run(
            &harness(&[]),
            &scan(),
            &HarnessState::default(),
            &plugin_config(),
            &source(&[], &[], false),
        );
        assert_eq!(
            p.actions,
            vec![
                Action::AddMarketplace {
                    name: "sadr".into(),
                    source: "github:muradmadi/sadr".into()
                },
                Action::Install {
                    id: "sadr@sadr".into()
                },
                Action::Install {
                    id: "superpowers@anthropic-plugin-directory".into()
                },
            ]
        );
        assert_eq!(
            p.actions[0].describe(Some(Tool::Claude)),
            "claude plugin marketplace add muradmadi/sadr"
        );
        assert!(p.actions.iter().all(Action::needs_tool));
        assert_eq!(
            states(&p),
            vec![
                "marketplace sadr marketplace-missing",
                "plugin sadr@sadr plugin-missing",
                "plugin superpowers@anthropic-plugin-directory plugin-missing"
            ]
        );
    }

    #[test]
    fn ec_19_installed_plugins_are_ok_and_not_claimed() {
        let mut s = scan();
        let mut plugins = Plugins::default();
        plugins.installed.insert("sadr@sadr".into());
        plugins
            .installed
            .insert("superpowers@anthropic-plugin-directory".into());
        plugins
            .marketplaces
            .insert("sadr".into(), serde_json::json!({}));
        s.plugins = Ok(plugins);
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &plugin_config(),
            &source(&[], &[], false),
        );
        assert!(p.in_sync(), "{:?}", states(&p));
        assert!(p.actions.is_empty());
    }

    #[test]
    fn ac_16_removed_plugins_are_uninstalled_only_if_ours() {
        let mut s = scan();
        let mut plugins = Plugins::default();
        plugins.installed.insert("old@sadr".into());
        plugins.installed.insert("hand@sadr".into());
        plugins
            .marketplaces
            .insert("gone".into(), serde_json::json!({}));
        s.plugins = Ok(plugins);
        let mut st = HarnessState::default();
        st.plugins.insert("old@sadr".into());
        st.plugins.insert("vanished@sadr".into());
        st.marketplaces.insert("gone".into());
        st.marketplaces.insert("gone-too".into());
        let p = run(
            &harness(&[]),
            &s,
            &st,
            &Config::default(),
            &source(&[], &[], false),
        );
        assert_eq!(
            p.actions,
            vec![
                Action::Uninstall {
                    id: "old@sadr".into()
                },
                Action::ForgetPlugin {
                    id: "vanished@sadr".into()
                },
                Action::RemoveMarketplace {
                    name: "gone".into()
                },
                Action::ForgetMarketplace {
                    name: "gone-too".into()
                },
            ]
        );
        assert!(states(&p).contains(&"plugin hand@sadr foreign".to_string()));
        assert!(states(&p).contains(&"plugin old@sadr orphaned".to_string()));
        let foreign = p.items.iter().find(|i| i.name == "hand@sadr").unwrap();
        assert_eq!(
            foreign.hint.as_deref(),
            Some("colctl adopt h plugin hand@sadr")
        );
    }

    #[test]
    fn ec_13_unreadable_plugin_state_blocks_plugin_actions() {
        let mut s = scan();
        s.plugins = Err("broken json".into());
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &plugin_config(),
            &source(&[], &[], false),
        );
        assert_eq!(states(&p), vec!["plugin * invalid"]);
        assert_eq!(p.items[0].hint.as_deref(), Some("broken json"));
        assert!(p.actions.is_empty());
    }

    #[test]
    fn ec_06_missing_harness_has_no_items() {
        let mut s = scan();
        s.root = Root::Missing;
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &plugin_config(),
            &source(&["a"], &[], true),
        );
        assert!(p.items.is_empty() && p.actions.is_empty());
        assert!(!p.in_sync());
    }

    #[test]
    fn ec_21_temps_are_cleaned_first() {
        let mut s = scan();
        s.temps.push("/h/.colctl-tmp-1-x".into());
        let p = run(
            &harness(&[]),
            &s,
            &HarnessState::default(),
            &Config::default(),
            &source(&["a"], &[], false),
        );
        assert_eq!(
            p.actions[0],
            Action::CleanupTemp("/h/.colctl-tmp-1-x".into())
        );
    }

    #[test]
    fn descriptions() {
        let all = [
            Action::CleanupTemp("/t".into()),
            Action::Link {
                link: "/l".into(),
                target: "/t".into(),
            },
            Action::Unlink("/l".into()),
            Action::WriteGenerated {
                rel: CLAUDE_MD.into(),
                path: "/c".into(),
                content: vec![],
            },
            Action::RemoveGenerated {
                rel: "CLAUDE.md".into(),
                path: "/c".into(),
            },
            Action::RecordGenerated {
                rel: "CLAUDE.md".into(),
                hash: "h".into(),
            },
            Action::AddMarketplace {
                name: "m".into(),
                source: "https://x".into(),
            },
            Action::RemoveMarketplace { name: "m".into() },
            Action::ForgetMarketplace { name: "m".into() },
            Action::Install { id: "p@m".into() },
            Action::Uninstall { id: "p@m".into() },
            Action::ForgetPlugin { id: "p@m".into() },
        ];
        let text: Vec<String> = all.iter().map(|a| a.describe(Some(Tool::Claude))).collect();
        assert_eq!(all[6].describe(None), "add marketplace https://x");
        assert_eq!(all[9].describe(None), "install plugin p@m");
        assert_eq!(text[1], "link /l -> /t");
        assert_eq!(text[6], "claude plugin marketplace add https://x");
        assert_eq!(text[10], "claude plugin uninstall p@m");
        assert_eq!(all.iter().filter(|a| a.needs_tool()).count(), 4);
        for (kind, s) in [
            (ItemKind::Instructions, "instructions"),
            (ItemKind::Skill, "skill"),
            (ItemKind::Agent, "agent"),
            (ItemKind::Marketplace, "marketplace"),
            (ItemKind::Plugin, "plugin"),
        ] {
            assert_eq!(kind.as_str(), s);
        }
        let item = Item {
            kind: ItemKind::Marketplace,
            name: "m".into(),
            state: ItemState::Ok,
            hint: None,
            address: Item::address_for(ItemKind::Marketplace, Kind::Claude, "m"),
            target: None,
        };
        assert_eq!(item.address(), "marketplace m");
        assert_eq!(
            Item::address_for(ItemKind::Agent, Kind::Codex, "r"),
            "agents/r.toml"
        );
        assert_eq!(
            Item::address_for(ItemKind::Agent, Kind::Claude, "r"),
            "agents/r.md"
        );
    }
}
