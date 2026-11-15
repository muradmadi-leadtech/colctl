# Interfaces: solid

## `review_cache` (new, `src/review_cache.rs`)

```text
Identity = Source { from, to } | Copy { hash } | Conversion { rev }
CachedReview { schema: u32, colctl: String, rules: u32, key, identity,
               direction, commits, files, flags, recommendation, conversion: Option<Change> }

get(ctx, key, &Identity) -> Option<CachedReview>   // miss on: no file, bad JSON, key/identity/colctl/rules mismatch
put(ctx, &CachedReview) -> ()                      // temp file + rename; failures only noted with -v
remove(ctx, key) -> ()
prune(ctx, held_keys) -> ()                        // drops entries for keys no longer held
```

- Path: `~/.colctl/.state/cache/reviews/<key with / as __>.json`.
- `risk::RULES_VERSION: u32`, bumped whenever a flag or pattern changes.
- Callers: `inspect` (when `with_diff` is false), `update`, `refresh`, `approve`, `reject`.

## Status cache

```text
core::status::snapshot(ctx, filter) -> Snapshot           // unchanged, and also saves it when filter is None
core::status::last(ctx) -> Option<(Snapshot, SystemTime)> // None on missing, unparsable or other colctl version
```

Path: `~/.colctl/.state/cache/status.json`.

## Jobs (`core::run::Job`)

| Variant | Same as | Errors |
|---|---|---|
| `Rm(String)` | `colctl rm <name>` | the command's |
| `Adopt { profile, items: Vec<String> }` | `colctl adopt <profile> <items…>` | per item, reported, others continue |
| `Convert(String)` | `colctl convert <source>` | "nothing Codex can use" as a refusal, not a failure |
| `ConvertUndo(String)` | `colctl convert --undo <source>` | |
| `Refresh(Vec<String>)` | `colctl refresh [<names>]` | |
| `MarketplaceRefresh` | `colctl marketplace refresh` | per tool |

## Reads

```text
core::adopt::candidates(ctx, profile) -> Vec<AdoptCandidate { key, kind, path, movable: Result<(), String> }>
core::discover::marketplaces(ctx) -> Vec<Marketplace>   // what `marketplace list --json` prints
core::discover::connectors(ctx, terms) -> Vec<Connector>
core::discover::developers(ctx) -> Vec<Developer>
```

## TUI keys (new)

| Key | Where | Action |
|---|---|---|
| `x` | Items | remove |
| `c` | Items, on a source | convert, or undo a conversion |
| `f` | Items, on a taken-over item | refresh (`r` stays reload, as in 2.5) |
| `a` | Profiles | adopt |
| `v` | Discover | next view: Plugins, Developers, Marketplaces, Connectors |
| `u` | Discover › Marketplaces | refresh marketplaces |
| `:` / Ctrl-K | anywhere | palette |

## CLI changes

- `colctl marketplace` with no subcommand prints help, exit 2.
- `review` and `--json`: `allowed-tools` grouped into one flag with a `files` list. JSON keeps every existing field.
- No other output changes.
