//! Secrets in MCP entries (cycle 10, T4; interfaces.md § `secret`): resolving references, telling a
//! literal that looks like a secret, hiding resolved values, and the leak check before a commit.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The marker that tells the leak check a value on that line isn't a secret (EC10-17).
pub const NOT_SECRET: &str = "# colctl: not-secret";

const SECRET_WORDS: [&str; 7] = [
    "TOKEN",
    "SECRET",
    "KEY",
    "PASSWORD",
    "PASSWD",
    "AUTH",
    "CREDENTIAL",
];

const TOKEN_PREFIXES: [&str; 9] = [
    "ghp_",
    "gho_",
    "ghs_",
    "github_pat_",
    "sk-",
    "xoxb-",
    "xoxp-",
    "glpat-",
    "AKIA",
];

/// True when `value`, set for `key`, looks like a secret: the key names one, or the value has a
/// known token's shape, or it's a long run of base64 or hex with letters and digits.
pub fn looks_secret(key: &str, value: &str) -> bool {
    if value.is_empty() || (value.starts_with("${") && value.ends_with('}')) {
        return false;
    }
    let upper = key.to_ascii_uppercase();
    if SECRET_WORDS.iter().any(|w| upper.contains(w)) {
        return true;
    }
    let bare = value.strip_prefix("Bearer ").unwrap_or(value);
    if TOKEN_PREFIXES
        .iter()
        .any(|p| bare.starts_with(p) && bare.len() > p.len() + 8)
    {
        return true;
    }
    if bare.starts_with("eyJ") && bare.matches('.').count() == 2 {
        return true;
    }
    bare.len() >= 32
        && bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c))
        && bare.chars().any(|c| c.is_ascii_digit())
        && bare.chars().any(|c| c.is_ascii_alphabetic())
}

/// A literal in `config.toml` that looks like a secret: where it is, never what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// e.g. `mcp.github.env.GITHUB_TOKEN`.
    pub path: String,
}

/// Every literal value in `[[mcp]]` entries that looks like a secret, unless its line carries
/// [`NOT_SECRET`]. Text that doesn't parse has no findings: loading it reports the error.
pub fn leak_check(config_text: &str) -> Vec<Finding> {
    let Ok(doc) = toml::from_str::<toml::Table>(config_text) else {
        return Vec::new();
    };
    let silenced = |value: &str| {
        config_text
            .lines()
            .any(|l| l.contains(NOT_SECRET) && l.contains(value))
    };
    let mut out = Vec::new();
    let mut check = |path: String, key: &str, v: &toml::Value| {
        if let Some(s) = v.as_str()
            && looks_secret(key, s)
            && !silenced(s)
        {
            out.push(Finding { path });
        }
    };
    let entries = doc
        .get("mcp")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for e in &entries {
        let name = e.get("name").and_then(toml::Value::as_str).unwrap_or("?");
        let mut tables: Vec<(String, &toml::Value)> = vec![(format!("mcp.{name}"), e)];
        if let Some(profiles) = e.get("profile").and_then(toml::Value::as_table) {
            for (h, o) in profiles {
                tables.push((format!("mcp.{name}.profile.{h}"), o));
            }
        }
        for (prefix, t) in tables {
            for field in ["env", "headers"] {
                if let Some(m) = t.get(field).and_then(toml::Value::as_table) {
                    for (k, v) in m {
                        check(format!("{prefix}.{field}.{k}"), k, v);
                    }
                }
            }
            if let Some(b) = t.get("bearer") {
                check(format!("{prefix}.bearer"), "bearer", b);
            }
        }
    }
    out
}

/// Keychain lookups made in this run, so each item is asked for once (EC10-13). `Debug` shows item
/// names only.
#[derive(Clone, Default)]
pub struct Keychain(Arc<Mutex<BTreeMap<String, Result<String, String>>>>);

impl std::fmt::Debug for Keychain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<String> = self
            .0
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        f.debug_set().entries(names).finish()
    }
}

impl Keychain {
    /// The item's value, asking `lookup` only the first time this run.
    pub fn get(
        &self,
        item: &str,
        lookup: impl FnOnce() -> Result<String, String>,
    ) -> Result<String, String> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(item.to_string()).or_insert_with(lookup).clone()
    }

    /// Forgets every value, at the start of a run.
    pub fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// Replaces every value in `secrets` found in `text` with `•••` (AC10-34).
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    let mut sorted: Vec<&String> = secrets.iter().filter(|s| !s.is_empty()).collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for s in sorted {
        out = out.replace(s.as_str(), "•••");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec10_17_toolport_ids_are_not_secrets() {
        assert!(!looks_secret(
            "TOOLPORT_CLIENT_ID",
            "claude-code@f32399132c8e1a62"
        ));
        assert!(!looks_secret("TOOLPORT_PROFILE", "trimmed"));
        assert!(!looks_secret("GITHUB_TOKEN", "${GH_TOKEN}"));
        assert!(looks_secret("GITHUB_TOKEN", "anything"));
        assert!(looks_secret(
            "X",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789"
        ));
        assert!(looks_secret("X", "Bearer sk-abcdefghijklmnop"));
        assert!(looks_secret("X", "eyJhbGciOi.eyJzdWIiOi.c2lnbmF0dXJl"));
        assert!(looks_secret("X", "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6"));
        assert!(!looks_secret(
            "X",
            "/Applications/Toolport.app/Contents/MacOS/gateway"
        ));
        assert!(!looks_secret("X", ""));
    }

    #[test]
    fn redact_hides_longest_first() {
        let s = vec!["abc".to_string(), "abcdef".to_string(), String::new()];
        assert_eq!(redact("x abcdef y abc", &s), "x ••• y •••");
    }

    #[test]
    fn keychain_asks_once() {
        let k = Keychain::default();
        let mut n = 0;
        assert_eq!(
            k.get("i", || {
                n += 1;
                Ok("v".into())
            }),
            Ok("v".into())
        );
        assert_eq!(k.get("i", || Err("asked again".into())), Ok("v".into()));
        assert_eq!(n, 1);
        assert_eq!(format!("{k:?}"), "{\"i\"}");
        k.clear();
        assert_eq!(k.get("i", || Err("no".into())), Err("no".into()));
    }
}
