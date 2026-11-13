# Edge cases: Discovery

_Cycle 5 · Refine · 2026-11-13_

IDs are `EC5-NN`; each needs a test whose name contains `ec5_NN`.

## Empty / missing

| ID | Case | Expected |
|---|---|---|
| EC5-01 | No harnesses, or none with a catalog | `search`: "no catalogs found; start Claude or Codex once, or add a marketplace", exit 1. `developers` and `marketplace list` say the same |
| EC5-02 | The plugin-directory cache is missing | Skipped silently. An unknown `schemaVersion` is skipped with a warning |
| EC5-03 | A known marketplace's folder is missing | Warning "marketplace <m>: <path> is missing"; the rest is searched |
| EC5-04 | A manifest entry without a name, or the same name twice in one manifest | The nameless one is skipped; the first of the duplicates is used |
| EC5-05 | A harness whose folder is missing, or `exclude`s a plugin | Its catalogs are skipped; excludes don't hide catalog entries (they're about syncing, not browsing) |

## Matching and merging

| ID | Case | Expected |
|---|---|---|
| EC5-06 | The same id in two profiles (claude and kilo-claude) | One entry; installed lists both |
| EC5-07 | One repo, several plugins in different folders | Separate results |
| EC5-08 | Repo URLs written differently: `https://github.com/O/r.git`, `git@github.com:o/r`, `github.com/o/r#plugins/x` | Normalise equal (host and owner lowercased, `.git` and trailing `/` dropped, `#path` as subpath); developer `o` |
| EC5-09 | A non-GitHub host, or a local path as the repo | Host repos: the first path segment is the developer. Local paths: the author name |
| EC5-10 | No result matches | "no plugins match <terms>", exit 1; `--json` gives `"matched": 0` and exit 1 |
| EC5-11 | `show` with an unknown id, or a bare name that several results share | Exit 2: "no plugin <x>", or "<x> is ambiguous:" with the candidate ids |
| EC5-12 | `--limit 0`; a `--tool` other than claude or codex | All results; usage error, exit 2 |

## Output

| ID | Case | Expected |
|---|---|---|
| EC5-13 | Names or descriptions with control characters or ANSI escapes | Escaped in text output (as cycle 4's review); raw in JSON |
| EC5-14 | Very long descriptions | Cut to the line in `search` (ending in "…"); full in `show` and JSON |

## Tool calls

| ID | Case | Expected |
|---|---|---|
| EC5-15 | `connectors` without `codex` on PATH, or with no Codex profiles | Exit 2: "connectors come from codex, and it isn't on PATH" / "no Codex profiles registered" |
| EC5-16 | `codex plugin list` fails or prints invalid JSON in one profile | Warning for that profile; the others are listed; exit 1 |
| EC5-17 | `marketplace refresh` with `claude` or `codex` missing | That tool's profiles print "skipped: <tool> isn't on PATH"; exit 1 |

## Adding

| ID | Case | Expected |
|---|---|---|
| EC5-18 | `add source git@github.com:o/r.git` (contains `@`) | Treated as a repo, as before: an argument that's a valid repo is never looked up as an id |
| EC5-19 | `add source <id>` where several results share the bare name, or the id is unknown | Exit 2, as EC5-11, nothing written |
| EC5-20 | `add source <id>` when the repo can't be cloned | As cycle 3: exit 2 with git's error, nothing written |
| EC5-21 | `add source <id> --name n --plugin p` | `--name` and `--plugin` override the defaults taken from the result |
