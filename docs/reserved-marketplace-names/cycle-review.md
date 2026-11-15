# Cycle review: reserved marketplace names (HIL fix)

_After cycle 8 · Evaluate · 2026-11-15_

| | |
|---|---|
| **Cycle** | `20261115-reserved-marketplace-names` (FORGE, HIL mode: Refine → Generate → Evaluate) |
| **Intent** | typescript-lsp failed in both Claude profiles on every sync and was installed nowhere |
| **Released** | [v2.5.1](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.5.1) (#122, release #123) |
| **Production impact** | None. Real profiles were only read; the smoke test ran on a copy |
| **Disposition** | _Yours to decide_ |

## Cause

typescript-lsp lives in `anthropics/claude-plugins-official`, whose Claude marketplace is named
`claude-plugins-official`. colctl had replaced the marketplace copy and installed the plugin from its pinned clone,
added as a local marketplace under the repo's own name. Claude refuses that name: "reserved for official Anthropic
marketplaces and can only be used with GitHub sources from the 'anthropics' organization". This was reproduced with
the real claude on a copy of the profile.

## Fix

- A plugin in a repo whose Claude marketplace has a reserved name is installed through colctl's own one-plugin
  marketplace `colctl-<source>`, pointing at the plugin's folder in the pinned clone. This is the same mechanism
  databricks already uses.
- The fake claude refuses a reserved name from a local folder, as the real one does.
- A Claude-only source with nothing Codex can use (no skills, no hook Codex runs; typescript-lsp only declares an LSP
  server) no longer suggests `colctl convert`, which refuses. It says there's nothing Codex can use.

## Smoke test (a copy of your machine, real claude)

```
✓ Synced 5 profiles · 4 changes

  ✓  claude       added marketplace colctl-typescript-lsp
                  installed typescript-lsp
  ✓  kilo-claude  added marketplace colctl-typescript-lsp
                  installed typescript-lsp
  ·  codex        · typescript-lsp: the repo has no Codex packaging, and there's nothing Codex can use in it (no skills, no hook Codex runs)
```

Then `✓ All 5 profiles in sync · 4 sources · nothing held for review`. Both Claude profiles list
`typescript-lsp@colctl-typescript-lsp`.

## Tests

ACH1-01 (integration, fake claude refusing the reserved name), ACH1-02 (status doesn't suggest convert), ECH1-01
(unit: a subfolder plugin is wrapped; other names are used as the repo has them). RED failed on CI on those tests;
`just check` passes, line coverage 93.6%.

## Decided with you

An LSP converter for Codex is **not** on the roadmap. A bridge would give Codex tools to call, not Claude's built-in
code intelligence, so it isn't the same thing.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.5.1`
- [ ] `colctl sync`: claude and kilo-claude add `colctl-typescript-lsp` and install typescript-lsp
- [ ] `colctl status` says all 5 profiles are in sync
- [ ] Decide the disposition
