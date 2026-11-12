# colctl init

```
colctl init [--dry-run]
```

**First run** (no `config.toml` in the source): creates the source with `config.toml`, `skills/`, `agents/`,
`.state/` and a `.gitignore` containing `.state/`, runs `git init`, makes one commit (`colctl: init`), then prints
`colctl status`. It does not create `AGENTS.md`.

**Later runs**: appends a `[[harness]]` block for each known profile folder that now exists and isn't registered yet
(compared by real path), leaving every existing line of `config.toml` untouched. A name already taken gets a suffix
(`kilo-claude-2`). Missing folders or `.gitignore` entries are restored and git is initialised if needed.

**Profiles it recognises**

| Name | Path |
|---|---|
| `claude` | `~/.claude` |
| `kilo-claude` | `~/Library/Application Support/kilo-proxy-headless/profiles/claude` |

A path is registered only if it is an existing folder. Others go in `config.toml` by hand
([how-to](../how-to/add-a-claude-profile.md)).

Exit code: 0, or 2 on a config or environment error.
