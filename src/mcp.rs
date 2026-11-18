//! MCP servers declared once in `config.toml` (cycle 10, docs/whole-setup/interfaces.md § config.toml):
//! the entry, its per-profile overrides, and the values that are references to secrets.

use crate::config::{Harness, Kind};
use crate::fsx;
use crate::tool::Tool;
use serde_json::json;
use std::collections::BTreeMap;

/// A value in `env`, `headers` or `bearer`: a literal, or a reference colctl never stores the value of.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Value {
    Literal(String),
    /// `{ env = "NAME" }`: an environment variable.
    Env(String),
    /// `{ keychain = "item" }`: a macOS keychain item.
    Keychain(String),
}

/// How a server is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    Http {
        url: String,
        headers: BTreeMap<String, Value>,
        /// `Authorization: Bearer <value>`.
        bearer: Option<Value>,
    },
}

/// What one profile changes about an entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Override {
    /// Merged key by key over the entry's `env`.
    pub env: BTreeMap<String, Value>,
    /// Replaces the entry's `args`.
    pub args: Option<Vec<String>>,
    /// Merged key by key over the entry's `headers`.
    pub headers: BTreeMap<String, Value>,
}

/// One `[[mcp]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub transport: Transport,
    pub env: BTreeMap<String, Value>,
    /// The tools it goes to; empty means every tool that takes MCP servers.
    pub tools: Vec<Tool>,
    /// Harness name → its override.
    pub profiles: BTreeMap<String, Override>,
}

/// An entry as one profile gets it, overrides applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub name: String,
    pub transport: Transport,
    pub env: BTreeMap<String, Value>,
}

impl Server {
    /// True when this entry goes to profiles of `tool`.
    pub fn goes_to(&self, tool: Tool) -> bool {
        self.tools.is_empty() || self.tools.contains(&tool)
    }

    /// The entry for `harness`, or `None` when that harness doesn't take it.
    pub fn for_harness(&self, harness: &Harness) -> Option<Effective> {
        let tool = Tool::for_kind(harness.kind)?;
        if !self.goes_to(tool) {
            return None;
        }
        let over = self.profiles.get(&harness.name);
        let mut env = self.env.clone();
        let mut transport = self.transport.clone();
        if let Some(o) = over {
            env.extend(o.env.clone());
            match &mut transport {
                Transport::Stdio { args, .. } => {
                    if let Some(a) = &o.args {
                        args.clone_from(a);
                    }
                }
                Transport::Http { headers, .. } => headers.extend(o.headers.clone()),
            }
        }
        Some(Effective {
            name: self.name.clone(),
            transport,
            env,
        })
    }
}

fn value_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Literal(s) => json!({ "literal": s }),
        Value::Env(n) => json!({ "env": n }),
        Value::Keychain(i) => json!({ "keychain": i }),
    }
}

fn map_json(m: &BTreeMap<String, Value>) -> serde_json::Value {
    m.iter()
        .map(|(k, v)| (k.clone(), value_json(v)))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

impl Effective {
    /// `sha256:` of the entry with references as references: never a resolved secret.
    pub fn config_hash(&self) -> String {
        let transport = match &self.transport {
            Transport::Stdio { command, args } => {
                json!({ "stdio": { "command": command, "args": args } })
            }
            Transport::Http {
                url,
                headers,
                bearer,
            } => json!({ "http": {
                "url": url,
                "headers": map_json(headers),
                "bearer": bearer.as_ref().map(value_json),
            } }),
        };
        let doc = json!({ "name": self.name, "transport": transport, "env": map_json(&self.env) });
        fsx::hash_bytes(doc.to_string().as_bytes())
    }
}

/// True for a server name both tools accept: `[a-z0-9][a-z0-9_-]*`, at most 64 characters.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && !key.contains(['=', '\n', '\r'])
}

/// Parses one `[[mcp]]` table. Errors name the entry and the field; the caller adds the line.
pub(crate) fn parse_entry(table: &toml::Table, harnesses: &[Harness]) -> Result<Server, String> {
    let name = match table.get("name") {
        Some(toml::Value::String(n)) => n.clone(),
        _ => return Err("mcp server: name is required".into()),
    };
    if !valid_name(&name) {
        return Err(format!(
            "mcp server name \"{name}\" must match [a-z0-9][a-z0-9_-]*, at most 64 characters"
        ));
    }
    let fail = |msg: String| format!("mcp server \"{name}\": {msg}");
    for key in table.keys() {
        if ![
            "name", "command", "args", "url", "headers", "bearer", "env", "tools", "profile",
        ]
        .contains(&key.as_str())
        {
            return Err(fail(format!("unknown key \"{key}\"")));
        }
    }
    let string = |key: &str| -> Result<Option<String>, String> {
        match table.get(key) {
            None => Ok(None),
            Some(toml::Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
            Some(_) => Err(fail(format!("{key} must be a non-empty string"))),
        }
    };
    let command = string("command")?;
    let url = string("url")?;
    let args = match table.get("args") {
        None => None,
        Some(v) => Some(strings(v).ok_or_else(|| fail("args must be a list of strings".into()))?),
    };
    let headers = values(table.get("headers"), "headers", &fail)?;
    let bearer = table
        .get("bearer")
        .map(|v| value(v).ok_or_else(|| fail(bad_value("bearer"))))
        .transpose()?;
    let transport = match (command, url) {
        (Some(command), None) => {
            if !headers.is_empty() {
                return Err(fail("headers only apply to a url server".into()));
            }
            if bearer.is_some() {
                return Err(fail("bearer only applies to a url server".into()));
            }
            Transport::Stdio {
                command,
                args: args.unwrap_or_default(),
            }
        }
        (None, Some(url)) => {
            if args.is_some() {
                return Err(fail("args only apply to a command server".into()));
            }
            Transport::Http {
                url,
                headers,
                bearer,
            }
        }
        _ => return Err(fail("set either command or url".into())),
    };
    let env = values(table.get("env"), "env", &fail)?;
    let mut tools = Vec::new();
    if let Some(v) = table.get("tools") {
        let bad = || fail("tools must be a list of \"claude\" and \"codex\"".into());
        for t in strings(v).ok_or_else(bad)? {
            let tool = match t.as_str() {
                "claude" => Tool::Claude,
                "codex" => Tool::Codex,
                _ => return Err(bad()),
            };
            if !tools.contains(&tool) {
                tools.push(tool);
            }
        }
        if tools.is_empty() {
            return Err(bad());
        }
    }
    let mut server = Server {
        name: name.clone(),
        transport,
        env,
        tools,
        profiles: BTreeMap::new(),
    };
    if let Some(p) = table.get("profile") {
        let p = p
            .as_table()
            .ok_or_else(|| fail("profile must be tables named after harnesses".into()))?;
        for (h, o) in p {
            let harness = harnesses
                .iter()
                .find(|x| &x.name == h)
                .ok_or_else(|| fail(format!("profile \"{h}\" isn't a harness")))?;
            let tool = match Tool::for_kind(harness.kind) {
                Some(t) if harness.kind != Kind::Agents => t,
                _ => return Err(fail(format!("{h} doesn't take MCP servers"))),
            };
            if !server.goes_to(tool) {
                return Err(fail(format!(
                    "profile \"{h}\" is a {} profile, which tools leaves out",
                    harness.kind.as_str()
                )));
            }
            server
                .profiles
                .insert(h.clone(), parse_override(o, &server, h, &fail)?);
        }
    }
    Ok(server)
}

fn parse_override(
    o: &toml::Value,
    server: &Server,
    h: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<Override, String> {
    let t = o
        .as_table()
        .ok_or_else(|| fail(format!("profile \"{h}\" must be a table")))?;
    let stdio = matches!(server.transport, Transport::Stdio { .. });
    let mut out = Override::default();
    for (k, v) in t {
        match k.as_str() {
            "env" => out.env = values(Some(v), "env", fail)?,
            "args" if stdio => {
                out.args =
                    Some(strings(v).ok_or_else(|| fail("args must be a list of strings".into()))?);
            }
            "headers" if !stdio => out.headers = values(Some(v), "headers", fail)?,
            "args" => return Err(fail("args only apply to a command server".into())),
            "headers" => return Err(fail("headers only apply to a url server".into())),
            other => {
                return Err(fail(format!(
                    "profile \"{h}\": unknown key \"{other}\" (env, args, headers)"
                )));
            }
        }
    }
    Ok(out)
}

fn strings(v: &toml::Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn bad_value(key: &str) -> String {
    format!("{key} must be a string, {{ env = \"NAME\" }} or {{ keychain = \"item\" }}")
}

/// A literal or a reference; `None` when it's neither, or a reference is empty or mixed.
fn value(v: &toml::Value) -> Option<Value> {
    match v {
        toml::Value::String(s) => Some(Value::Literal(s.clone())),
        toml::Value::Table(t) if t.len() == 1 => {
            let (k, v) = t.iter().next()?;
            let s = v.as_str().filter(|s| !s.is_empty())?.to_string();
            match k.as_str() {
                "env" => Some(Value::Env(s)),
                "keychain" => Some(Value::Keychain(s)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn values(
    v: Option<&toml::Value>,
    what: &str,
    fail: &dyn Fn(String) -> String,
) -> Result<BTreeMap<String, Value>, String> {
    let Some(v) = v else {
        return Ok(BTreeMap::new());
    };
    let t = v
        .as_table()
        .ok_or_else(|| fail(format!("{what} must be a table")))?;
    let mut out = BTreeMap::new();
    for (k, v) in t {
        if !valid_key(k) {
            return Err(fail(format!(
                "{what} key {k:?} can't be empty or contain = or a newline"
            )));
        }
        let parsed = value(v).ok_or_else(|| fail(bad_value(&format!("{what} {k}"))))?;
        out.insert(k.clone(), parsed);
    }
    Ok(out)
}

fn edit_value(v: &Value) -> toml_edit::Value {
    match v {
        Value::Literal(s) => s.as_str().into(),
        Value::Env(n) => inline([("env", n.as_str().into())]),
        Value::Keychain(i) => inline([("keychain", i.as_str().into())]),
    }
}

fn inline<'a>(pairs: impl IntoIterator<Item = (&'a str, toml_edit::Value)>) -> toml_edit::Value {
    let mut t = toml_edit::InlineTable::new();
    for (k, v) in pairs {
        t.insert(k, v);
    }
    t.fmt();
    toml_edit::Value::InlineTable(t)
}

fn edit_map(m: &BTreeMap<String, Value>) -> toml_edit::Value {
    inline(m.iter().map(|(k, v)| (k.as_str(), edit_value(v))))
}

fn edit_strings(list: &[String]) -> toml_edit::Value {
    toml_edit::Value::Array(list.iter().map(String::as_str).collect())
}

fn key(k: &str) -> String {
    toml_edit::Key::new(k).display_repr().into_owned()
}

/// The `[[mcp]]` block for `server`, starting with a blank line.
pub(crate) fn render(server: &Server) -> String {
    let mut out = format!(
        "\n[[mcp]]\nname = {}\n",
        toml_edit::Value::from(server.name.as_str())
    );
    match &server.transport {
        Transport::Stdio { command, args } => {
            out.push_str(&format!(
                "command = {}\n",
                toml_edit::Value::from(command.as_str())
            ));
            out.push_str(&format!("args = {}\n", edit_strings(args)));
        }
        Transport::Http {
            url,
            headers,
            bearer,
        } => {
            out.push_str(&format!("url = {}\n", toml_edit::Value::from(url.as_str())));
            if !headers.is_empty() {
                out.push_str(&format!("headers = {}\n", edit_map(headers)));
            }
            if let Some(b) = bearer {
                out.push_str(&format!("bearer = {}\n", edit_value(b)));
            }
        }
    }
    if !server.tools.is_empty() {
        let tools: Vec<String> = server
            .tools
            .iter()
            .map(|t| t.program().to_string())
            .collect();
        out.push_str(&format!("tools = {}\n", edit_strings(&tools)));
    }
    if !server.env.is_empty() {
        out.push_str(&format!("env = {}\n", edit_map(&server.env)));
    }
    for (h, o) in &server.profiles {
        out.push_str(&format!("\n[mcp.profile.{}]\n", key(h)));
        if !o.env.is_empty() {
            out.push_str(&format!("env = {}\n", edit_map(&o.env)));
        }
        if let Some(a) = &o.args {
            out.push_str(&format!("args = {}\n", edit_strings(a)));
        }
        if !o.headers.is_empty() {
            out.push_str(&format!("headers = {}\n", edit_map(&o.headers)));
        }
    }
    out
}
