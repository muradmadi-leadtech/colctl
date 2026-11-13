# Components: convert (C4 level 3)

| Component | Module | Responsibility | New / changed |
|---|---|---|---|
| Repackager | `src/repackage.rs` (new) | From a clone folder with `.claude-plugin/plugin.json`, build a `Package`: the Codex manifest, the skills, the hooks Codex supports, and a `Report` (hooks kept, hooks and matchers Codex never fires, parts not carried over). Pure until `write`, which writes it deterministically (sorted, fixed mode bits, no timestamps) | New |
| Hook mapper | `src/repackage.rs::hooks` | Keeps events Codex runs, keeps matchers that can fire (`Bash`, `Edit`, `Write`, `mcp__…`, `*`), lists the rest. Commands are copied unchanged (Codex sets `CLAUDE_PLUGIN_ROOT`) | New |
| Agent converter | `src/convert.rs` | Unchanged (cycle 2's agent `.md` → `.toml`); not used for plugin agents this cycle | — |
| Config | `src/config.rs` | `Source.convert: Vec<Tool>`; validation (only `codex`; only for a source with Claude packaging) | Changed |
| State | `src/state.rs` | `Candidate::Conversion { rev, digest }` under the key `conversions/<name>` | Changed |
| Packaging | `src/packaging.rs` | For a source with `convert = ["codex"]` and no native Codex packaging, Codex's target is the converted package; native packaging wins | Changed |
| Plan / status | `src/plan.rs`, `status_view.rs` | `unsupported` items get the fix `colctl convert <name>`; a converted plugin is `ok`, counted as a plugin, with `converted` in `--all` and JSON; a stale package is `outdated` | Changed |
| Commands | `src/commands/convert.rs` (new), `quarantine.rs` | `convert <source> [--undo] [--dry-run] [--json]`; review shows a conversion (manifest, skills, hooks with risk flags, report); approve writes `convert` and syncs; reject drops it; update holds a converted source's new commit with its new conversion shown in review | New / changed |
| Risk | `src/risk.rs` | Reused: hooks in a generated package are flagged like hooks in an update | Reused |
