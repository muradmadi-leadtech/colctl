# Share an MCP server across Claude and Codex

**If every profile already has it** (you set it up by hand in each), adopt it:

```sh
colctl adopt
colctl status
```

colctl writes one `[[mcp]]` entry. Values that differ between profiles, such as toolport's `TOOLPORT_CLIENT_ID`,
become per-profile overrides; nothing is re-added, and `status` says each profile's copy is ok.

**If it's new**, add it once:

```sh
colctl add mcp github --env-from GITHUB_TOKEN=GH_TOKEN -- npx -y @modelcontextprotocol/server-github
```

Every Claude and Codex profile gets it. `--only claude` or `--only codex` keeps it to one tool.

**To change it**, edit its entry in `~/.colctl/config.toml` and run `colctl sync`: each profile's copy is removed and
added again. **To remove it**: `colctl rm mcp github`.

Start a new Claude or Codex session to pick up the change. Keep tokens out of `config.toml`:
[keep a secret out of ~/.colctl](keep-a-secret-out-of-colctl.md).
