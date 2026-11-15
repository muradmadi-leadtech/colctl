# Acceptance criteria: reserved marketplace names (HIL fix, 2.5.1)

_HIL cycle after cycle 8 · Refine · 2026-11-15. IDs are `ACH1-NN` / `ECH1-NN` (H1: the first HIL fix cycle)._

Found on the owner's machine: typescript-lsp, added as a source from `anthropics/claude-plugins-official`, failed in
both Claude profiles on every sync. Claude refuses a local marketplace named `claude-plugins-official` ("reserved
for official Anthropic marketplaces and can only be used with GitHub sources from the 'anthropics' organization"),
and colctl added the clone under the repo's own marketplace name. Since colctl had already replaced the
marketplace copy, the plugin was installed nowhere.

**ACH1-01 A reserved marketplace name installs through colctl's own**
```gherkin
Given a source whose repo's Claude marketplace is named "claude-plugins-official" and lists the plugin in the repo
When colctl syncs a Claude profile
Then it installs the plugin from colctl's one-plugin marketplace "colctl-<source>", pointing at the plugin's folder
  in the pinned clone, and the source is ok
```

**ACH1-02 Codex isn't pointed at a convert that refuses**
```gherkin
Given a Claude-only source with nothing Codex can use (no skills, no hook Codex runs; e.g. only an LSP server)
Then status and sync say it's not packaged for Codex because there's nothing Codex can use
And they don't suggest colctl convert, which would refuse
```
