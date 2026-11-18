//! MCP servers declared once in `config.toml` (cycle 10, docs/whole-setup/interfaces.md § config.toml):
//! the entry, its per-profile overrides, and the values that are references to secrets.

use crate::config::{Harness, Kind};
use crate::tool::Tool;
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
    /// The entry for `harness`, or `None` when that harness doesn't take it.
    pub fn for_harness(&self, harness: &Harness) -> Option<Effective> {
        let _ = (harness, Kind::Agents);
        todo!()
    }
}

impl Effective {
    /// `sha256:` of the entry with references as references: never a resolved secret.
    pub fn config_hash(&self) -> String {
        todo!()
    }
}
