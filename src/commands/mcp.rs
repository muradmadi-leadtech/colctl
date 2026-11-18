//! `colctl add mcp`, `colctl rm mcp` and adopting MCP servers (cycle 10, T3; interfaces.md § Commands).

use super::{
    Planned, Workspace, commit, commit_pending, confirmed, dry_or_write, lead, load, plan_all,
    plural, read_mcp, run_sync_led, write_config,
};
use crate::config::{self, CONFIG_FILE, Harness};
use crate::ctx::Ctx;
use crate::error::{CliError, EXIT_DRIFT};
use crate::git::Git;
use crate::mcp::{Effective, Override, Server, Transport, Value};
use crate::plan::{ItemKind, ItemState};
use crate::state::{self, Lock, McpRecord};
use crate::tool::Tool;
use crate::ui::style::Token;
use std::collections::BTreeMap;

/// What `colctl add mcp` was given.
#[derive(Debug, Default)]
pub struct AddMcp {
    pub name: String,
    pub only: Option<Tool>,
    pub env: Vec<String>,
    pub env_from: Vec<String>,
    pub keychain: Vec<String>,
    pub url: Option<String>,
    pub headers: Vec<String>,
    pub bearer_from: Option<String>,
    pub bearer_keychain: Option<String>,
    /// `-- <command> [args]`.
    pub command: Vec<String>,
}

fn pair<'a>(flag: &str, raw: &'a str) -> Result<(&'a str, &'a str), CliError> {
    raw.split_once('=')
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .ok_or_else(|| CliError::usage(format!("{flag} takes KEY=VALUE, got \"{raw}\"")))
}

/// The config entry `add mcp` describes, or a usage error.
pub fn entry(a: &AddMcp) -> Result<Server, CliError> {
    if !crate::mcp::valid_name(&a.name) {
        return Err(CliError::usage(format!(
            "MCP server name \"{}\" must match [a-z0-9][a-z0-9_-]*, at most 64 characters",
            a.name
        )));
    }
    let mut env = BTreeMap::new();
    for raw in &a.env {
        let (k, v) = pair("--env", raw)?;
        if crate::secret::looks_secret(k, v) {
            return Err(CliError::usage(format!(
                "That looks like a secret. Use --env-from {k}=NAME or --keychain {k}=ITEM."
            )));
        }
        env.insert(k.to_string(), Value::Literal(v.to_string()));
    }
    for raw in &a.env_from {
        let (k, v) = pair("--env-from", raw)?;
        env.insert(k.to_string(), Value::Env(v.to_string()));
    }
    for raw in &a.keychain {
        let (k, v) = pair("--keychain", raw)?;
        env.insert(k.to_string(), Value::Keychain(v.to_string()));
    }
    let bearer = match (&a.bearer_from, &a.bearer_keychain) {
        (Some(_), Some(_)) => {
            return Err(CliError::usage(
                "pass --bearer-from or --bearer-keychain, not both",
            ));
        }
        (Some(n), None) => Some(Value::Env(n.clone())),
        (None, Some(i)) => Some(Value::Keychain(i.clone())),
        (None, None) => None,
    };
    let transport = match (&a.url, a.command.split_first()) {
        (Some(url), None) => {
            let mut headers = BTreeMap::new();
            for raw in &a.headers {
                let (k, v) = pair("--header", raw)?;
                headers.insert(k.to_string(), Value::Literal(v.to_string()));
            }
            Transport::Http {
                url: url.clone(),
                headers,
                bearer,
            }
        }
        (None, Some((command, args))) => {
            if !a.headers.is_empty() || bearer.is_some() {
                return Err(CliError::usage(
                    "--header and --bearer-* only apply with --url",
                ));
            }
            Transport::Stdio {
                command: command.clone(),
                args: args.to_vec(),
            }
        }
        _ => {
            return Err(CliError::usage(
                "give the server a command after -- or a --url, not both",
            ));
        }
    };
    Ok(Server {
        name: a.name.clone(),
        transport,
        env,
        tools: a.only.into_iter().collect(),
        profiles: BTreeMap::new(),
    })
}

/// `colctl add mcp` (AC10-20, AC10-21).
pub fn add(ctx: &Ctx, a: &AddMcp, dry_run: bool) -> Result<i32, CliError> {
    let server = entry(a)?;
    let git = Git::for_ctx(ctx)?;
    let ws = load(ctx)?;
    if ws.config.mcp.iter().any(|s| s.name == server.name) {
        return Err(CliError::usage(format!(
            "MCP server {} is already in {CONFIG_FILE}",
            server.name
        )));
    }
    let text = config::add_mcp(&ws.config_text, &server).map_err(CliError::usage)?;
    if dry_or_write(
        ctx,
        dry_run,
        &format!("add MCP server {} to {CONFIG_FILE}", server.name),
    ) {
        return Ok(0);
    }
    let _lock = Lock::acquire_with(&ctx.source, &ctx.out)?;
    commit_pending(&git, ctx)?;
    write_config(ctx, &text)?;
    commit(&git, ctx, &format!("colctl: add mcp {}", server.name))?;
    run_sync_led(
        ctx,
        &git,
        None,
        &lead(ctx, &format!("Added MCP server {}", server.name), &[]),
    )
}

/// `colctl rm mcp` (AC10-22, AC10-13).
pub fn rm(ctx: &Ctx, ws: &Workspace, name: &str, dry_run: bool) -> Result<i32, CliError> {
    if !ws.config.mcp.iter().any(|s| s.name == name) {
        let elsewhere = ws
            .config
            .harnesses
            .iter()
            .any(|h| read_mcp(ctx, h).is_ok_and(|e| e.contains_key(name)));
        return Err(CliError::usage(if elsewhere {
            format!("{name} wasn't added by colctl; remove it with the tool that added it")
        } else {
            format!("no managed MCP server named \"{name}\"")
        }));
    }
    let git = Git::for_ctx(ctx)?;
    if dry_or_write(ctx, dry_run, &format!("remove MCP server {name}")) {
        return Ok(0);
    }
    let profiles = ws
        .config
        .harnesses
        .iter()
        .filter(|h| h.path.is_dir() && Tool::for_kind(h.kind).is_some())
        .count();
    if !confirmed(
        ctx,
        &format!(
            "Remove MCP server {name} from {}?",
            plural(profiles, "profile")
        ),
    ) {
        return Ok(EXIT_DRIFT);
    }
    let _lock = Lock::acquire_with(&ctx.source, &ctx.out)?;
    commit_pending(&git, ctx)?;
    let text = config::remove_mcp(&ws.config_text, name).map_err(CliError::usage)?;
    write_config(ctx, &text)?;
    commit(&git, ctx, &format!("colctl: rm mcp {name}"))?;
    run_sync_led(
        ctx,
        &git,
        None,
        &lead(ctx, &format!("Removed MCP server {name}"), &[]),
    )
}

/// A profile's entry, as colctl would write it in config: transport and env, every value a literal.
fn from_profile(
    tool: Tool,
    raw: &serde_json::Value,
) -> Option<(Transport, BTreeMap<String, Value>)> {
    let strings = |v: &serde_json::Value| -> BTreeMap<String, Value> {
        v.as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), Value::Literal(s.into()))))
                    .collect()
            })
            .unwrap_or_default()
    };
    // AC10-33: a value that looks like a secret becomes a reference to a variable of that name.
    let env: BTreeMap<String, Value> = strings(&raw["env"])
        .into_iter()
        .map(|(k, v)| match &v {
            Value::Literal(s) if crate::secret::looks_secret(&k, s) => (k.clone(), Value::Env(k)),
            _ => (k, v),
        })
        .collect();
    let args: Vec<String> = raw["args"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if let Some(command) = raw["command"].as_str() {
        return Some((
            Transport::Stdio {
                command: command.into(),
                args,
            },
            env,
        ));
    }
    let url = raw["url"].as_str()?.to_string();
    let mut headers = strings(&raw["headers"]);
    let mut bearer = None;
    match tool {
        Tool::Claude => {
            if let Some(Value::Literal(h)) = headers.get("Authorization")
                && let Some(n) = h
                    .strip_prefix("Bearer ${")
                    .and_then(|r| r.strip_suffix('}'))
            {
                bearer = Some(Value::Env(n.to_string()));
                headers.remove("Authorization");
            }
        }
        Tool::Codex => {
            bearer = raw["bearer_token_env_var"]
                .as_str()
                .map(|n| Value::Env(n.to_string()));
        }
    }
    Some((
        Transport::Http {
            url,
            headers,
            bearer,
        },
        env,
    ))
}

/// Same command (or URL), so one entry can describe both.
fn same_server(a: &Transport, b: &Transport) -> bool {
    match (a, b) {
        (Transport::Stdio { command: x, .. }, Transport::Stdio { command: y, .. }) => x == y,
        (Transport::Http { url: x, .. }, Transport::Http { url: y, .. }) => x == y,
        _ => false,
    }
}

/// One profile's copy of a server: the profile, its tool, and the entry read three ways.
type Sighting = (
    Harness,
    Tool,
    Transport,
    BTreeMap<String, Value>,
    serde_json::Value,
);

/// One server found in profiles: the first profile's version, and every profile that has the same.
struct Found {
    entry: Server,
    /// (harness, entry as the profile holds it).
    have: Vec<(Harness, serde_json::Value)>,
    /// Profiles whose server under this name is a different one.
    differ: Vec<String>,
}

/// Groups the servers profiles have that config doesn't into one entry each (AC10-23 – 24,
/// EC10-24 – 25): values every profile shares go in the entry, the rest in per-profile overrides.
fn gather(ctx: &Ctx, planned: &[Planned], only: Option<&str>) -> Vec<Found> {
    let mut found: BTreeMap<String, Found> = BTreeMap::new();
    let mut seen: BTreeMap<String, Vec<Sighting>> = BTreeMap::new();
    for p in planned {
        if only.is_some_and(|o| o != p.harness.name) {
            continue;
        }
        let Some(tool) = Tool::for_kind(p.harness.kind) else {
            continue;
        };
        let Ok(entries) = read_mcp(ctx, &p.harness) else {
            continue;
        };
        for i in p
            .plan
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Mcp && i.state == ItemState::Foreign)
        {
            let Some(raw) = entries.get(&i.name) else {
                continue;
            };
            if let Some((t, env)) = from_profile(tool, raw) {
                seen.entry(i.name.clone()).or_default().push((
                    p.harness.clone(),
                    tool,
                    t,
                    env,
                    raw.clone(),
                ));
            }
        }
    }
    for (name, list) in seen {
        let first = &list[0].2;
        let (same, differ): (Vec<_>, Vec<_>) = list
            .iter()
            .partition(|(_, _, t, _, _)| same_server(first, t));
        let mut tools: Vec<Tool> = Vec::new();
        for (_, tool, ..) in &same {
            if !tools.contains(tool) {
                tools.push(*tool);
            }
        }
        // Shared: what every profile that has it agrees on.
        let shared_env: BTreeMap<String, Value> = same[0]
            .3
            .iter()
            .filter(|(k, v)| same.iter().all(|s| s.3.get(*k) == Some(*v)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let args_of = |t: &Transport| match t {
            Transport::Stdio { args, .. } => Some(args.clone()),
            Transport::Http { .. } => None,
        };
        let headers_of = |t: &Transport| match t {
            Transport::Http { headers, .. } => headers.clone(),
            Transport::Stdio { .. } => BTreeMap::new(),
        };
        let shared_args =
            args_of(first).filter(|a| same.iter().all(|s| args_of(&s.2).as_ref() == Some(a)));
        let first_headers = headers_of(first);
        let shared_headers: BTreeMap<String, Value> = first_headers
            .iter()
            .filter(|(k, v)| same.iter().all(|s| headers_of(&s.2).get(*k) == Some(*v)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut transport = first.clone();
        match &mut transport {
            Transport::Stdio { args, .. } => *args = shared_args.clone().unwrap_or_default(),
            Transport::Http { headers, .. } => headers.clone_from(&shared_headers),
        }
        let mut profiles = BTreeMap::new();
        for (h, _, t, env, _) in &same {
            let o = Override {
                env: env
                    .iter()
                    .filter(|(k, v)| shared_env.get(*k) != Some(*v))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                args: match (&shared_args, args_of(t)) {
                    (None, Some(a)) => Some(a),
                    _ => None,
                },
                headers: headers_of(t)
                    .into_iter()
                    .filter(|(k, v)| shared_headers.get(k) != Some(v))
                    .collect(),
            };
            if o != Override::default() {
                profiles.insert(h.name.clone(), o);
            }
        }
        // Found in both tools' profiles: it goes everywhere, so no `tools`.
        let tools = if tools.len() == 2 { Vec::new() } else { tools };
        found.insert(
            name.clone(),
            Found {
                entry: Server {
                    name,
                    transport,
                    env: shared_env,
                    tools,
                    profiles,
                },
                have: same.iter().map(|s| (s.0.clone(), s.4.clone())).collect(),
                differ: differ.iter().map(|s| s.0.name.clone()).collect(),
            },
        );
    }
    found.into_values().collect()
}

/// Adopts the MCP servers profiles have that config doesn't: one entry each, recorded as colctl's in
/// the profiles that have it, so nothing is re-added. Returns the lines to print.
pub fn adopt(
    ctx: &Ctx,
    git: &Git,
    only: Option<&str>,
    wanted: Option<&str>,
    dry_run: bool,
) -> Result<Vec<(Token, String)>, CliError> {
    let ws = load(ctx)?;
    let planned = plan_all(ctx, &ws, None)?;
    let found: Vec<Found> = gather(ctx, &planned, only)
        .into_iter()
        .filter(|f| wanted.is_none_or(|w| w == f.entry.name))
        .collect();
    let mut said = Vec::new();
    if found.is_empty() {
        return Ok(said);
    }
    if dry_run {
        for f in &found {
            said.push((
                Token::Info,
                format!(
                    "would adopt MCP server {} from {}",
                    f.entry.name,
                    names(&f.have)
                ),
            ));
        }
        return Ok(said);
    }
    let mut text = ws.config_text.clone();
    for f in &found {
        text = config::add_mcp(&text, &f.entry).map_err(CliError::usage)?;
    }
    write_config(ctx, &text)?;
    commit(
        git,
        ctx,
        &format!(
            "colctl: adopt mcp {}",
            found
                .iter()
                .map(|f| f.entry.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )?;
    let mut st = state::load(&ctx.source)?;
    for f in &found {
        for (h, raw) in &f.have {
            if let Some(e) = f.entry.for_harness(h) {
                st.harness_mut(&h.name).mcp.insert(
                    e.name.clone(),
                    McpRecord {
                        config: Effective::config_hash(&e),
                        written: crate::mcp::written_hash(raw),
                    },
                );
            }
        }
        let mut line = format!(
            "adopted MCP server {} from {}",
            f.entry.name,
            names(&f.have)
        );
        for (k, v) in &f.entry.env {
            if matches!(v, Value::Env(n) if n == k) {
                let item = format!("colctl-{}", f.entry.name);
                line.push_str(&format!(
                    ". {k} looked like a secret, so config refers to it: Set {k} in your shell, or store it with security add-generic-password -s {item} -w and switch to {{ keychain = \"{item}\" }}"
                ));
            }
        }
        if !f.differ.is_empty() {
            line.push_str(&format!(
                "; {} has a different {} and keeps it (colctl sync --replace-mcp {} replaces it)",
                f.differ.join(", "),
                f.entry.name,
                f.entry.name
            ));
        }
        said.push((Token::Ok, line));
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    Ok(said)
}

fn names(have: &[(Harness, serde_json::Value)]) -> String {
    have.iter()
        .map(|(h, _)| h.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
