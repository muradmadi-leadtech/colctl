# Edge cases: convert

| ID | Category | Case | Expected |
|---|---|---|---|
| EC7-01 | Invalid | `convert nope` (no such source) | Error exit 2, hint `colctl status` |
| EC7-02 | Boundary | The source already packages for Codex | "qdrant already packages for Codex", exit 0, nothing held |
| EC7-03 | Boundary | Already converted | "qdrant is already converted", exit 0 |
| EC7-04 | Boundary | A conversion is already held | Built again and held again (same digest: unchanged), exit 0, hint review |
| EC7-05 | Invalid | The source has no Claude packaging (nothing to convert from) | Error exit 1: "qdrant has no Claude packaging to convert" |
| EC7-06 | Invalid | `hooks/hooks.json` doesn't parse, or has an unknown shape | Error exit 1 naming the file and the problem; nothing held |
| EC7-07 | Empty | Claude's `plugin.json` has no `name` | The source's plugin name is used |
| EC7-08 | Invalid | The manifest's `skills` or `hooks` path leaves the plugin folder (`../x`, absolute) | Error exit 1; nothing held |
| EC7-09 | Security | A symlink in the plugin folder points outside it | Not followed or copied; listed under not carried over |
| EC7-10 | Empty | No Codex profile is registered | Error exit 1: "no Codex profile is registered", hint `colctl init` |
| EC7-11 | Empty | Nothing Codex can use (no skills, no hook it runs) | Error exit 1: "qdrant has nothing Codex can use", with the report |
| EC7-12 | Concurrent | Another colctl holds the lock | Exit 3, as every writing command |
| EC7-13 | Failure | `codex plugin install` fails for one profile | That profile's step fails (exit 1); config keeps `convert`; the next sync retries |
| EC7-14 | Boundary | `convert --undo` when it isn't converted | "qdrant isn't converted", exit 0 |
| EC7-15 | Boundary | `convert --undo` while a conversion is held | The held conversion is dropped, exit 0 |
| EC7-16 | Boundary | `rm source qdrant` on a converted source | Its conversion goes too, from Codex profiles and disk |
| EC7-17 | Boundary | Plugin files that aren't UTF-8, or are executable | Copied byte for byte; the executable bit is kept, other mode bits normalised |
| EC7-18 | Boundary | `hooks` declared inline in Claude's `plugin.json` | Mapped like `hooks/hooks.json`, written to the package's `hooks/hooks.json` |
| EC7-19 | Failure | Building the package fails half way (disk full) | Nothing half-built is left where Codex reads it: the package is built in a temp folder and renamed |
| EC7-20 | Timing | A newer colctl converts the same commit differently | The digest differs, so sync rebuilds and reinstalls once |
