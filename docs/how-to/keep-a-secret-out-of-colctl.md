# Keep a secret out of ~/.colctl

`~/.colctl` is a git repository, so a token written into `config.toml` would stay in its history. Point at the secret
instead.

**From your shell's environment:**

```sh
colctl add mcp github --env-from GITHUB_TOKEN=GH_TOKEN -- npx -y @modelcontextprotocol/server-github
```

Claude keeps `${GH_TOKEN}` and reads it when it starts the server, so the token is never written anywhere. Codex can't
keep a reference for a command server's env, so colctl passes the value it sees at sync: set `GH_TOKEN` in the shell
you run `colctl sync` from. If it isn't set, Codex profiles are skipped with "set GH_TOKEN, then colctl sync".

**From the macOS keychain:**

```sh
security add-generic-password -s colctl-github -a "$USER" -w
colctl add mcp github --keychain GITHUB_TOKEN=colctl-github -- npx -y @modelcontextprotocol/server-github
```

colctl reads the item once per sync and hands the value to `claude mcp add` and `codex mcp add`. It ends up in
`~/.claude.json` and `~/.codex/config.toml`, as it would if you added the server by hand, but never in `~/.colctl`.
colctl doesn't read secrets to compare them, so after changing the item run `colctl sync --replace-mcp github`.

colctl refuses to commit a literal that looks like a secret, naming the key but never the value. If it isn't one, end
the line with `# colctl: not-secret`.
