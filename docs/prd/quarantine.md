# PRD: Quarantine

_Cycle 4 · Focus · 2026-11-12_

## Problem statement

colctl installs other people's code into every harness: plugins cloned from their author's repo, and skills that
tools like twg link in from their own folders. Once a plugin or skill is trusted, its next version reaches every
harness with one command and no chance to look first:

- `colctl update` fetches a source plugin, moves its pin and installs the new commit in every Claude and Codex
  profile, all in one step.
- `colctl refresh` copies a taken-over skill from its origin over the source copy, commits it and syncs it.

A plugin can carry hooks that run on every tool call, MCP servers, and scripts. A skill can grant itself Bash. A
new version can add any of these. In colctl 1.2, the first time the owner sees a change is when it's already
installed in every profile.

## Story

> `colctl update` tells me superpowers has 12 new commits. Nothing is installed yet. `colctl review superpowers`
> shows me the commits and the diff, and points out that this version adds a `SessionStart` hook and a new shell
> script. I read them, then `colctl approve superpowers`, and every harness gets exactly the version I reviewed.
> If I don't like it, `colctl reject superpowers` and colctl won't offer that version again. The same goes for a
> twg skill whose origin changed: `colctl refresh` holds the new copy until I approve it.

## Target users

colctl users who run plugins and taken-over skills from authors they don't control, and want to see a new version
before it runs in every agent harness on their machine.

## Solution summary

- **Held, not installed.** `colctl update` fetches each source plugin's new version and holds it as a
  **candidate**: the commit is recorded, nothing is checked out, the pin doesn't move, no harness changes.
  `colctl refresh` does the same for a taken-over skill or agent: it snapshots the origin's content into
  quarantine inside `~/.colctl`, and leaves the source copy as it is.
- **Review.** `colctl review [<name>]` lists every held candidate, or shows one in detail: the commits (for
  plugins), the file diff against what's installed, and **risk flags**, the changes that let code run or widen
  what an agent may do:
  - hooks (new or changed), MCP and LSP servers;
  - scripts and executable files, new symlinks;
  - `allowed-tools` (Bash especially) in skill, command or agent frontmatter;
  - changes to the plugin manifest.
- **Decide.**
  - `colctl approve <name>` installs exactly what was reviewed. A plugin's pin moves to the candidate commit; a
    skill's snapshot replaces the source copy. colctl commits and syncs every harness.
  - `colctl reject <name>` drops the candidate and remembers it, so the same version isn't offered again. A newer
    one will be.
- **Visible.** `colctl status` shows held candidates as `pending-review`. List plugins (`[plugins]`,
  `[codex.plugins]`) whose versions colctl can't hold back are flagged `unreviewed`, with a hint to adopt them as a
  source plugin, which puts them under quarantine.
- **Unchanged:**
  - `update --check` stays read-only.
  - `sync` stays offline and never installs a candidate.
  - First-time `add source` and `adopt` stay as they are: choosing a new item is already explicit.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | Nothing new installs without approval. | After `update` or `refresh`, every harness's installed commit and every source copy are byte-for-byte what they were before. Only `approve` changes them. |
| SC-2 | What's approved is what was reviewed. | If the upstream or origin changes between `review` and `approve`, `approve` installs the held candidate, never the newer content. A tampered snapshot is refused. |
| SC-3 | Risky changes are pointed out. | For a fixture plugin that adds a hook, an MCP server, an executable script and `allowed-tools: Bash`, `review` flags each one by file. A candidate with none of these has no flags. |
| SC-4 | A rejected version stays rejected. | After `reject`, `update` doesn't hold the same commit (or the same origin content) again; a newer one is held. |
| SC-5 | Pending work is visible. | `status` (text and `--json`) lists every candidate as `pending-review` and every list plugin as `unreviewed`. `review --json` is machine-readable for the TUI in cycle 6. |
| SC-6 | Nothing regresses. | Cycle 1–3 suites pass, adjusted only where `update` and `refresh` now hold instead of install, and every such change is listed in the cycle review. |
| SC-7 | Test-heavy and strict TDD. | AC4/EC4 tests named after IDs; ≥ 90% coverage; RED pushed before GREEN on each PR. |
| SC-8 | Docs. | Reference pages for `review`, `approve`, `reject`, changed `update`/`refresh`/`status`; a how-to for reviewing an update; an explanation of quarantine and its limits. |

## Scope

**In:**
- Holding source-plugin updates (`update`, `update --rev`) and taken-over skill and agent refreshes (`refresh`) as
  candidates.
- `review`, `approve`, `reject`, with `--json` where it reads.
- Risk flags from a fixed list of file patterns and frontmatter keys.
- `pending-review` and `unreviewed` in `status`.
- Docs, and a major release: `update` and `refresh` no longer install on their own.

**Out (won't build this cycle):**
- **Holding first-time adds.** `add source` and `adopt` install as they do today.
- **Holding list-plugin versions.** colctl doesn't own their source and won't change Claude's or Codex's
  auto-update settings. They are flagged `unreviewed` only.
- **Judging code.** No malware scanning, signature checks or AI review. Risk flags are pattern matches that point
  the owner at files; they don't say a version is safe.
- **Background checks and notifications.** Nothing runs on a schedule; the owner runs `update`/`refresh`.
- **Any UI beyond the CLI.** The TUI is cycle 6; this cycle only makes sure its data is available as JSON.
