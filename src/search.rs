//! Merging, matching, ranking and grouping catalog entries (cycle 5, interfaces.md § Ranking).
//! Pure: it works on [`catalog::Entry`] values and touches nothing else.

use crate::catalog::{COLCTL, Entry};
use crate::config::PLUGIN_DIRECTORY;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// A catalog a plugin appears in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CatalogRef {
    pub name: String,
    pub tool: String,
}

/// One plugin, merged from every catalog that lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plugin {
    pub name: String,
    pub display_name: Option<String>,
    pub ids: Vec<String>,
    pub catalogs: Vec<CatalogRef>,
    pub kind: String,
    pub developer: String,
    pub author: Option<String>,
    pub description: String,
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub homepage: Option<String>,
    pub repo: Option<String>,
    pub subpath: Option<String>,
    pub version: Option<String>,
    pub commit: Option<String>,
    pub installed: BTreeSet<String>,
    pub source_plugin: Option<String>,
    /// What to run to add it, or why there's nothing to run.
    pub add: String,
}

/// A repo URL in one canonical form (EC5-08): `https://host/owner/repo`, lowercase scheme and host,
/// no `.git` or trailing `/`, with `#path` split off as a subpath. Local paths stay as they are.
pub fn normalise_repo(repo: &str) -> (String, Option<String>) {
    let (repo, hash) = match repo.split_once('#') {
        Some((r, p)) => (
            r,
            Some(p.trim_matches('/').to_string()).filter(|p| !p.is_empty()),
        ),
        None => (repo, None),
    };
    let mut url = repo.trim().to_string();
    if let Some(rest) = url.strip_prefix("github:") {
        url = format!("https://github.com/{rest}");
    } else if let Some(rest) = url.strip_prefix("git@")
        && let Some((host, path)) = rest.split_once(':')
    {
        url = format!("https://{host}/{path}");
    } else if !url.contains("://") && !url.starts_with('/') && url.contains('/') {
        url = format!("https://{url}");
    }
    if let Some((scheme, rest)) = url.split_once("://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let path = path.trim_end_matches('/').trim_end_matches(".git");
        url = format!("{}://{}/{path}", scheme.to_lowercase(), host.to_lowercase());
        url = url.trim_end_matches('/').to_string();
    }
    (url, hash)
}

/// The owner of a hosted repo (first path segment, lowercased), or `None` for local paths.
fn repo_owner(repo: &str) -> Option<String> {
    let (url, _) = normalise_repo(repo);
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))?;
    let mut parts = rest.split('/');
    let _host = parts.next()?;
    parts
        .next()
        .filter(|o| !o.is_empty())
        .map(|o| o.trim_start_matches("git@").to_lowercase())
}

/// A plugin's developer (AC5-11): its repo's owner, else its author, else "unknown".
pub fn developer(repo: Option<&str>, author: Option<&str>) -> String {
    repo.and_then(repo_owner)
        .or_else(|| author.map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

/// Catalog order when merging: colctl's own first, then marketplaces by name, the directory last.
fn priority(e: &Entry) -> (u8, String) {
    let rank = match e.catalog.as_str() {
        COLCTL => 0,
        PLUGIN_DIRECTORY => 2,
        _ => 1,
    };
    (rank, e.catalog.clone())
}

fn merge_key(e: &Entry) -> String {
    match &e.repo {
        Some(repo) => {
            let (url, hash) = normalise_repo(repo);
            let sub = e
                .subpath
                .clone()
                .or(hash)
                .map(|s| s.trim_matches('/').trim_start_matches("./").to_string())
                .unwrap_or_default();
            format!(
                "{}\u{0}{}\u{0}{}",
                url.to_lowercase(),
                sub,
                e.name.to_lowercase()
            )
        }
        None => format!("id\u{0}{}\u{0}{}", e.tool, e.id),
    }
}

fn add_text(p: &Plugin, tool_of_first: &str) -> String {
    if let Some(s) = &p.source_plugin {
        return format!("already source plugin {s}");
    }
    let id = &p.ids[0];
    if p.kind == "connector" {
        return format!("installed by codex: codex plugin add {id}");
    }
    if p.repo.is_some() {
        return format!("colctl add source {id}");
    }
    if tool_of_first == "codex" {
        format!("colctl add plugin {id} --codex")
    } else {
        format!("colctl add plugin {id}")
    }
}

/// Merges entries that are the same plugin (AC5-08): same normalised repo, subpath and name, or
/// (without a repo) the same tool and id.
pub fn merge(entries: &[Entry]) -> Vec<Plugin> {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by_key(|e| priority(e));
    let mut groups: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in sorted {
        groups.entry(merge_key(e)).or_default().push(e);
    }
    let mut out = Vec::new();
    for list in groups.values() {
        let first = list[0];
        let pick = |f: &dyn Fn(&Entry) -> Option<String>| list.iter().find_map(|e| f(e));
        let mut ids: Vec<String> = Vec::new();
        for e in list {
            if !ids.contains(&e.id) {
                ids.push(e.id.clone());
            }
        }
        // Prefer an id from a marketplace over colctl's own, so "add" names something findable.
        if let Some(pos) = list.iter().position(|e| e.catalog != COLCTL) {
            let preferred = list[pos].id.clone();
            ids.retain(|i| i != &preferred);
            ids.insert(0, preferred);
        }
        let catalogs: BTreeSet<CatalogRef> = list
            .iter()
            .map(|e| CatalogRef {
                name: e.catalog.clone(),
                tool: e.tool.clone(),
            })
            .collect();
        let author = pick(&|e| e.author.clone());
        let repo = pick(&|e| e.repo.clone());
        let mut keywords: Vec<String> = Vec::new();
        for k in list.iter().flat_map(|e| e.keywords.iter()) {
            if !keywords.contains(k) {
                keywords.push(k.clone());
            }
        }
        let mut p = Plugin {
            name: first.name.clone(),
            display_name: pick(&|e| e.display_name.clone()),
            catalogs: catalogs.into_iter().collect(),
            kind: first.kind.clone(),
            developer: developer(repo.as_deref(), author.as_deref()),
            author,
            description: pick(&|e| Some(e.description.clone()).filter(|d| !d.is_empty()))
                .unwrap_or_default(),
            keywords,
            category: pick(&|e| e.category.clone()),
            homepage: pick(&|e| e.homepage.clone()),
            subpath: pick(&|e| e.subpath.clone()),
            version: pick(&|e| e.version.clone()),
            commit: pick(&|e| e.commit.clone()),
            installed: list.iter().flat_map(|e| e.installed.clone()).collect(),
            source_plugin: pick(&|e| e.source_plugin.clone()),
            repo,
            ids,
            add: String::new(),
        };
        let tool = list
            .iter()
            .find(|e| e.id == p.ids[0])
            .map_or("claude", |e| e.tool.as_str());
        p.add = add_text(&p, tool);
        out.push(p);
    }
    out.sort_by(|a, b| (&a.name, &a.ids).cmp(&(&b.name, &b.ids)));
    out
}

/// What to search for (AC5-09, AC5-10).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub terms: Vec<String>,
    pub tool: Option<String>,
    pub marketplace: Option<String>,
    pub developer: Option<String>,
    pub installed: bool,
    /// 0 for all.
    pub limit: usize,
}

/// A term's best hit in a plugin, or `None` when it doesn't appear anywhere.
fn term_score(p: &Plugin, term: &str) -> Option<u32> {
    let has = |s: &str| s.to_lowercase().contains(term);
    let name = p.name.to_lowercase();
    let display = p.display_name.as_deref().unwrap_or_default().to_lowercase();
    if name == term || display == term {
        return Some(100);
    }
    if name.contains(term) || display.contains(term) {
        return Some(50);
    }
    if p.keywords.iter().any(|k| has(k)) {
        return Some(30);
    }
    if has(&p.description) {
        return Some(10);
    }
    if has(&p.developer) || p.author.as_deref().is_some_and(has) {
        return Some(5);
    }
    None
}

/// A plugin's score for all terms (every term must hit), or `None`.
pub fn score(p: &Plugin, terms: &[String]) -> Option<u32> {
    terms.iter().map(|t| term_score(p, &t.to_lowercase())).sum()
}

fn passes(p: &Plugin, q: &Query) -> bool {
    q.tool
        .as_ref()
        .is_none_or(|t| p.catalogs.iter().any(|c| &c.tool == t))
        && q.marketplace
            .as_ref()
            .is_none_or(|m| p.catalogs.iter().any(|c| &c.name == m))
        && q.developer
            .as_ref()
            .is_none_or(|d| p.developer.eq_ignore_ascii_case(d))
        && (!q.installed || !p.installed.is_empty())
}

/// The results for a query, ranked and limited, and how many matched in all (AC5-09, AC5-10).
pub fn search(plugins: &[Plugin], q: &Query) -> (Vec<Plugin>, usize) {
    let mut hits: Vec<(u32, &Plugin)> = plugins
        .iter()
        .filter(|p| passes(p, q))
        .filter_map(|p| score(p, &q.terms).map(|s| (s, p)))
        .collect();
    hits.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.ids.cmp(&b.ids))
    });
    let matched = hits.len();
    let shown = if q.limit == 0 {
        matched
    } else {
        q.limit.min(matched)
    };
    (
        hits.into_iter()
            .take(shown)
            .map(|(_, p)| p.clone())
            .collect(),
        matched,
    )
}

/// A developer and what they publish (AC5-17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Developer {
    pub name: String,
    pub plugins: usize,
    pub catalogs: Vec<String>,
}

/// Developers by plugin count, then name; names compare case-insensitively.
pub fn developers(plugins: &[Plugin]) -> Vec<Developer> {
    let mut by: BTreeMap<String, Developer> = BTreeMap::new();
    for p in plugins.iter().filter(|p| p.kind == "plugin") {
        let d = by
            .entry(p.developer.to_lowercase())
            .or_insert_with(|| Developer {
                name: p.developer.clone(),
                plugins: 0,
                catalogs: Vec::new(),
            });
        d.plugins += 1;
        for c in &p.catalogs {
            if !d.catalogs.contains(&c.name) {
                d.catalogs.push(c.name.clone());
            }
        }
    }
    let mut out: Vec<Developer> = by.into_values().collect();
    for d in &mut out {
        d.catalogs.sort();
    }
    out.sort_by(|a, b| {
        b.plugins
            .cmp(&a.plugins)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, catalog: &str, repo: Option<&str>) -> Entry {
        Entry {
            id: id.into(),
            name: id.split('@').next().unwrap().into(),
            catalog: catalog.into(),
            tool: "claude".into(),
            kind: "plugin".into(),
            repo: repo.map(str::to_string),
            ..Entry::default()
        }
    }

    #[test]
    fn ac5_08_the_same_plugin_is_one_result() {
        let mut src = entry(
            "superpowers@superpowers-dev",
            COLCTL,
            Some("https://github.com/obra/superpowers"),
        );
        src.source_plugin = Some("superpowers".into());
        src.commit = Some("22f0b46".into());
        src.installed.insert("codex".into());
        let mut dir = entry(
            "superpowers@anthropic-plugin-directory",
            PLUGIN_DIRECTORY,
            Some("https://github.com/obra/superpowers"),
        );
        dir.description = "Skills".into();
        dir.commit = Some("8ca22db".into());
        dir.installed.insert("claude".into());
        let market = entry(
            "superpowers@claude-plugins-official",
            "claude-plugins-official",
            Some("git@github.com:Obra/superpowers.git"),
        );
        let merged = merge(&[dir, market, src]);
        assert_eq!(merged.len(), 1);
        let p = &merged[0];
        assert_eq!(
            p.ids,
            vec![
                "superpowers@claude-plugins-official",
                "superpowers@superpowers-dev",
                "superpowers@anthropic-plugin-directory"
            ]
        );
        assert_eq!(p.catalogs.len(), 3);
        assert_eq!(p.description, "Skills");
        assert_eq!(
            p.commit.as_deref(),
            Some("22f0b46"),
            "colctl's own source comes first"
        );
        assert_eq!(p.installed.len(), 2);
        assert_eq!(p.add, "already source plugin superpowers");
        assert_eq!(p.developer, "obra");
    }

    #[test]
    fn ec5_07_one_repo_several_plugins_stay_separate() {
        let repo = Some("https://github.com/anthropics/claude-plugins-official");
        let mut a = entry("a@official", "official", repo);
        a.subpath = Some("plugins/a".into());
        let mut b = entry("b@official", "official", repo);
        b.subpath = Some("plugins/b".into());
        assert_eq!(merge(&[a, b]).len(), 2);
    }

    #[test]
    fn ec5_08_repo_urls_normalise() {
        for url in [
            "https://github.com/O/r.git",
            "git@github.com:o/r",
            "github.com/o/r/",
            "HTTPS://GitHub.com/o/r",
            "github:o/r",
        ] {
            let (n, _) = normalise_repo(url);
            assert_eq!(n.to_lowercase(), "https://github.com/o/r", "{url}");
            assert_eq!(developer(Some(url), None), "o", "{url}");
        }
        assert_eq!(
            normalise_repo("github.com/o/r#plugins/x"),
            ("https://github.com/o/r".into(), Some("plugins/x".into()))
        );
        // The same plugin under two URL spellings merges.
        let a = entry("p@m1", "m1", Some("https://github.com/O/r.git"));
        let b = entry("p@m2", "m2", Some("git@github.com:o/r"));
        assert_eq!(merge(&[a, b]).len(), 1);
    }

    #[test]
    fn ec5_09_ac5_11_developers_from_hosts_paths_and_authors() {
        assert_eq!(developer(Some("https://gitlab.com/Team/x"), None), "team");
        assert_eq!(
            developer(Some("/Users/me/.colctl/plugins/x"), Some("Ann")),
            "Ann"
        );
        assert_eq!(developer(None, Some("Ann")), "Ann");
        assert_eq!(developer(None, None), "unknown");
        assert_eq!(developer(Some("file:///tmp/x"), Some("Bo")), "Bo");
    }

    fn plugins() -> Vec<Plugin> {
        let mut api = entry(
            "42crunch-api-security-testing@official",
            "official",
            Some("https://github.com/42crunch-ai/p"),
        );
        api.description = "Automate API security directly in Claude Code".into();
        let mut sec = entry(
            "security-guidance@dir",
            PLUGIN_DIRECTORY,
            Some("https://github.com/anthropics/x"),
        );
        sec.description = "Security reminders while editing an API".into();
        sec.installed.insert("claude".into());
        let mut kw = entry("linter@official", "official", None);
        kw.keywords = vec!["security".into(), "api".into()];
        kw.author = Some("Ann".into());
        let mut exact = entry("api@codexm", "codexm", Some("https://github.com/o/api"));
        exact.tool = "codex".into();
        exact.description = "security".into();
        merge(&[api, sec, kw, exact])
    }

    fn names(r: &[Plugin]) -> Vec<&str> {
        r.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn ac5_09_matching_and_ranking() {
        let all = plugins();
        let q = Query {
            terms: vec!["API".into(), "security".into()],
            ..Query::default()
        };
        let (got, matched) = search(&all, &q);
        assert_eq!(matched, 4);
        // api: exact name 100 + description 10; 42crunch: name 50 + name 50; linter: kw 30 + kw 30;
        // security-guidance: desc 10 + name 50.
        assert_eq!(
            names(&got),
            vec![
                "api",
                "42crunch-api-security-testing",
                "linter",
                "security-guidance"
            ]
        );
        let none = Query {
            terms: vec!["api".into(), "nothing".into()],
            ..Query::default()
        };
        assert_eq!(search(&all, &none).1, 0);
        let author = Query {
            terms: vec!["ann".into()],
            ..Query::default()
        };
        assert_eq!(names(&search(&all, &author).0), vec!["linter"]);
        // No terms: everything, by name.
        assert_eq!(
            search(&all, &Query::default()).0[0].name,
            "42crunch-api-security-testing"
        );
    }

    #[test]
    fn ac5_10_filters_and_limit() {
        let all = plugins();
        let q = |f: fn(&mut Query)| {
            let mut q = Query::default();
            f(&mut q);
            names(&search(&all, &q).0)
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(q(|q| q.tool = Some("codex".into())), vec!["api"]);
        assert_eq!(
            q(|q| q.marketplace = Some(PLUGIN_DIRECTORY.into())),
            vec!["security-guidance"]
        );
        assert_eq!(
            q(|q| q.developer = Some("42CRUNCH-AI".into())),
            vec!["42crunch-api-security-testing"]
        );
        assert_eq!(q(|q| q.installed = true), vec!["security-guidance"]);
        let (shown, matched) = search(
            &all,
            &Query {
                limit: 2,
                ..Query::default()
            },
        );
        assert_eq!((shown.len(), matched), (2, 4));
    }

    #[test]
    fn add_commands() {
        let with_repo = &merge(&[entry("a@m", "m", Some("https://github.com/o/a"))])[0];
        assert_eq!(with_repo.add, "colctl add source a@m");
        let no_repo = &merge(&[entry("b@m", "m", None)])[0];
        assert_eq!(no_repo.add, "colctl add plugin b@m");
        let mut codex = entry("c@m", "m", None);
        codex.tool = "codex".into();
        assert_eq!(
            merge(&[codex.clone()])[0].add,
            "colctl add plugin c@m --codex"
        );
        codex.kind = "connector".into();
        assert_eq!(
            merge(&[codex])[0].add,
            "installed by codex: codex plugin add c@m"
        );
    }

    #[test]
    fn ac5_17_developers_by_count_then_name() {
        let es = [
            entry("a@m", "m", Some("https://github.com/obra/a")),
            entry("b@n", "n", Some("https://github.com/Obra/b")),
            entry("c@m", "m", Some("https://github.com/zed/c")),
            entry("d@m", "m", Some("https://github.com/alf/d")),
        ];
        let d = developers(&merge(&es));
        assert_eq!(
            d.iter()
                .map(|d| (d.name.as_str(), d.plugins))
                .collect::<Vec<_>>(),
            vec![("obra", 2), ("alf", 1), ("zed", 1)]
        );
        assert_eq!(d[0].catalogs, vec!["m", "n"]);
    }
}
