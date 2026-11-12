//! Turning an agent written for Claude Code (`agents/<name>.md`: YAML frontmatter + a Markdown
//! body) into a Codex custom agent (`agents/<name>.toml`). Pure: text in, text out.

use toml_edit::{DocumentMut, value};
use yaml_rust2::{Yaml, YamlLoader};

/// The parts of an agent every harness needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Parses an agent file. `stem` is the file name without `.md`, used when the frontmatter has
/// no `name`. Errors say why the agent can't be converted (AC2-11).
pub fn parse_agent(markdown: &str, stem: &str) -> Result<Agent, String> {
    let text = markdown.replace("\r\n", "\n");
    let mut lines = text.split('\n');
    if lines.next() != Some("---") {
        return Err("no frontmatter: the file must start with a --- line".into());
    }
    let mut yaml = Vec::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        yaml.push(line);
    }
    if !closed {
        return Err("no frontmatter: no closing --- line".into());
    }
    let docs = YamlLoader::load_from_str(&yaml.join("\n"))
        .map_err(|e| format!("frontmatter isn't valid YAML: {e}"))?;
    let doc = docs.into_iter().next().unwrap_or(Yaml::Null);
    let map = match doc {
        Yaml::Hash(map) => map,
        Yaml::Null => Default::default(),
        _ => return Err("frontmatter must be a mapping of keys to values".into()),
    };
    let text_field = |key: &str| -> Result<Option<String>, String> {
        match map.get(&Yaml::String(key.into())) {
            None | Some(Yaml::Null) => Ok(None),
            Some(Yaml::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("{key} must be text")),
        }
    };
    let description = text_field("description")?
        .filter(|d| !d.trim().is_empty())
        .ok_or("no description in the frontmatter")?;
    let name = text_field("name")?
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| stem.to_string());

    let rest: Vec<&str> = lines.collect();
    let first = rest.iter().position(|l| !l.trim().is_empty());
    let last = rest.iter().rposition(|l| !l.trim().is_empty());
    let body = match (first, last) {
        (Some(a), Some(b)) => rest[a..=b].join("\n"),
        _ => return Err("empty body: there are no instructions after the frontmatter".into()),
    };
    Ok(Agent {
        name,
        description,
        body,
    })
}

/// The Codex TOML for `agent`, with a header pointing at its source (interfaces.md).
pub fn render_codex(agent: &Agent, source_display: &str, stem: &str) -> String {
    let mut doc = DocumentMut::new();
    doc["name"] = value(agent.name.as_str());
    doc["description"] = value(agent.description.as_str());
    doc["developer_instructions"] = value(agent.body.as_str());
    format!(
        "# Managed by colctl. Edit {source_display}/agents/{stem}.md instead; this file is regenerated.\n{doc}"
    )
}

/// `parse_agent` then `render_codex`, as bytes ready to write.
pub fn codex_agent(markdown: &str, stem: &str, source_display: &str) -> Result<Vec<u8>, String> {
    let agent = parse_agent(markdown, stem)?;
    Ok(render_codex(&agent, source_display, stem).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEWER: &str = "---\nname: reviewer\ndescription: Reviews diffs\nmodel: opus\ntools: Read, Grep\ncolor: blue\n---\n\nYou review diffs.\n\nBe brief.\n\n";

    fn parse_toml(text: &str) -> toml::Table {
        toml::from_str(text).unwrap_or_else(|e| panic!("{e}\n{text}"))
    }

    #[test]
    fn ac2_08_renders_the_generated_toml() {
        let out =
            String::from_utf8(codex_agent(REVIEWER, "reviewer", "~/.colctl").unwrap()).unwrap();
        assert!(
            out.starts_with(
                "# Managed by colctl. Edit ~/.colctl/agents/reviewer.md instead; this file is regenerated.\n"
            ),
            "{out}"
        );
        let t = parse_toml(&out);
        assert_eq!(t["name"].as_str(), Some("reviewer"));
        assert_eq!(t["description"].as_str(), Some("Reviews diffs"));
        assert_eq!(
            t["developer_instructions"].as_str(),
            Some("You review diffs.\n\nBe brief.")
        );
        let keys: Vec<&String> = t.keys().collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        let order: Vec<usize> = ["name =", "description =", "developer_instructions ="]
            .iter()
            .map(|k| out.find(k).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{out}");
    }

    #[test]
    fn ac2_11_agents_that_cannot_be_converted() {
        for (md, why) in [
            ("No frontmatter at all.\n", "no frontmatter"),
            ("---\nname: x\n---\nbody\n", "no description"),
            ("---\ndescription: \"  \"\n---\nbody\n", "no description"),
            ("---\ndescription: d\n---\n\n   \n", "empty body"),
            (
                "---\ndescription: d\nbody without a closing fence\n",
                "no frontmatter",
            ),
            (
                "---\ndescription: [1, 2]\n---\nbody\n",
                "description must be text",
            ),
            (
                "---\n- just\n- a list\n---\nbody\n",
                "frontmatter must be a mapping",
            ),
        ] {
            let err = parse_agent(md, "x").unwrap_err();
            assert!(err.contains(why), "{md:?}: {err}");
        }
    }

    #[test]
    fn ec2_05_invalid_yaml_reports_the_parse_error() {
        let err = parse_agent("---\ndescription: [unclosed\n---\nbody\n", "x").unwrap_err();
        assert!(err.starts_with("frontmatter isn't valid YAML"), "{err}");
    }

    #[test]
    fn ec2_06_frontmatter_name_wins_over_the_file_name() {
        let a = parse_agent("---\nname: code-reviewer\ndescription: d\n---\nb\n", "rev").unwrap();
        assert_eq!(a.name, "code-reviewer");
        let a = parse_agent("---\ndescription: d\n---\nb\n", "rev").unwrap();
        assert_eq!(a.name, "rev");
        let out = render_codex(&a, "/src", "rev");
        assert!(out.starts_with("# Managed by colctl. Edit /src/agents/rev.md instead;"));
    }

    #[test]
    fn ec2_10_claude_only_keys_are_dropped() {
        let out =
            String::from_utf8(codex_agent(REVIEWER, "reviewer", "~/.colctl").unwrap()).unwrap();
        for key in ["model", "tools", "color", "opus"] {
            assert!(!out.contains(key), "{key} leaked into:\n{out}");
        }
    }

    #[test]
    fn ec2_11_awkward_text_round_trips_exactly() {
        let description = "Says \"hi\", 'bye', back\\slash, ''' and \"\"\" — naïve ✓";
        let body = "Line with \"\"\" inside.\nAnd ''' too.\n\tTabbed \\n not a newline.\nÜnïcödé ✓";
        let md = format!(
            "---\ndescription: '{}'\n---\n{body}\n",
            description.replace('\'', "''")
        );
        let agent = parse_agent(&md, "weird").unwrap();
        assert_eq!(agent.description, description);
        assert_eq!(agent.body, body);
        let t = parse_toml(&render_codex(&agent, "~/.colctl", "weird"));
        assert_eq!(t["description"].as_str(), Some(description));
        assert_eq!(t["developer_instructions"].as_str(), Some(body));
    }

    #[test]
    fn windows_line_endings_and_leading_blank_lines() {
        let a = parse_agent(
            "---\r\ndescription: d\r\n---\r\n\r\n  indented first line\r\nnext\r\n\r\n",
            "x",
        )
        .unwrap();
        assert_eq!(a.description, "d");
        assert_eq!(a.body, "  indented first line\nnext");
    }
}
