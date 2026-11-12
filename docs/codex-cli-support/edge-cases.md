# Edge cases: Codex CLI support

_Cycle 2 · Refine · 2026-11-12_

IDs are `EC2-NN`; each needs a test whose name contains `ec2_NN`.

## Empty / null

| ID | Case | Expected |
|---|---|---|
| EC2-01 | Codex harness has no `agents/` folder | Created when the first TOML is written |
| EC2-02 | Source has no agents | No `agents/*.toml` written; existing generated ones are removed as orphaned |
| EC2-03 | `~/.agents` has no `skills/` folder | Created when the first link is needed |
| EC2-04 | Codex `config.toml` missing | Nothing installed; plugins planned normally |

## Invalid

| ID | Case | Expected |
|---|---|---|
| EC2-05 | Agent frontmatter isn't valid YAML | Agent `invalid` in codex harnesses with the parse error; Claude link unaffected |
| EC2-06 | Frontmatter `name` differs from the file name | TOML `name` = frontmatter name; file is `<stem>.toml` |
| EC2-07 | Codex `config.toml` doesn't parse | That harness's plugins `invalid`; no `codex` calls for it; other items still sync |
| EC2-08 | Codex plugin uses an undeclared marketplace | Config error, exit 2 |
| EC2-09 | Exclude entry wrong for the kind | Config error, exit 2 (AC2-03) |
| EC2-10 | Frontmatter has Claude-only keys (`model`, `tools`, `color`) | Ignored; not in the TOML |

## Boundary / ownership

| ID | Case | Expected |
|---|---|---|
| EC2-11 | Description or body contains quotes, `"""`, `'''`, backslashes, non-ASCII | TOML parses back to the exact strings |
| EC2-12 | Codex `AGENTS.md` is a symlink pointing outside the source | Foreign link; never changed; adopt refuses |
| EC2-13 | `~/.agents/skills/<name>` is a real folder with the same name as a source skill | Foreign; untouched; sync exit 1 |
| EC2-14 | Marketplace records itself under another name than the `[codex.marketplaces]` key | Action fails with both names; its plugins skipped |
| EC2-15 | Native harness written as an absolute or symlinked `~/.codex` | Recognised by canonical path; `CODEX_HOME` unset for it |
| EC2-16 | Plugin installed with `enabled = false` | Counted as installed (ok); colctl never re-enables it |
| EC2-17 | A source agent and a hand-made TOML share a name | The TOML is foreign; no generated file replaces it |

## Failure

| ID | Case | Expected |
|---|---|---|
| EC2-18 | `codex` hangs | Killed after the timeout, reported "timed out" |
| EC2-19 | One harness's `agents/` isn't writable | That item fails; other harnesses continue; exit 1 |

## Concurrent / timing / compatibility

| ID | Case | Expected |
|---|---|---|
| EC2-20 | colctl runs inside a Codex session (`CODEX_HOME`, `OPENAI_API_KEY` set) | Never inherited (AC2-14) |
| EC2-21 | kilo rewrites `config.toml` between runs (reordered, comments gone) | Status unchanged: plugin state is parsed, not hashed |
| EC2-22 | A 1.0 `state.json` | Loads unchanged; codex entries are added beside it |

## Taking over linked skills

| ID | Case | Expected |
|---|---|---|
| EC2-23 | Link target is itself a symlink chain | Resolved to the final folder; that folder is copied and recorded as the origin |
| EC2-24 | Origin folder is deleted later (tool uninstalled) | Skill stays ok; refresh reports "origin is gone" and changes nothing |
| EC2-25 | The same origin is linked under two names | Each name is adopted separately, both with that origin |
| EC2-26 | Origin contains a `.git` folder | Copied as is (adopt keeps the exact tree, unlike `add skill`) |
| EC2-27 | Relative link target (`../../.config/twg/skills/x`) | Resolved against the link's folder |
| EC2-28 | `refresh` of a skill with no recorded origin | Exit 2, "skills/<name> has no recorded origin" |
