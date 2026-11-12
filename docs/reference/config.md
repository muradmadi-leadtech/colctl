# config.toml

`~/.colctl/config.toml` registers harnesses and lists marketplaces and plugins. Unknown keys and invalid values are
errors that name the line (`config.toml: line 7: duplicate harness name "claude"`), and every command refuses to run
until they're fixed.

```toml
version = 1                      # required, must be 1

[[harness]]                      # zero or more
name = "kilo-claude"             # required, unique, [a-z0-9-]+
kind = "claude"                  # required; "claude" is the only kind so far
path = "~/Library/Application Support/kilo-proxy-headless/profiles/claude"   # absolute or ~/…
exclude = ["skills/foo"]         # optional

[marketplaces]                   # optional; name = source
sadr = "github:muradmadi/sadr"   # github:owner/repo | https://… | /absolute/path

[plugins]
enabled = ["sadr@sadr", "superpowers@anthropic-plugin-directory"]
```

| Key | Rules |
|---|---|
| `harness.name` | `[a-z0-9-]+`, unique |
| `harness.path` | Absolute or starting with `~/`. Two harnesses may not resolve to the same folder. |
| `harness.exclude` | Entries: `CLAUDE.md`, `skills/<name>`, `agents/<name>.md`, `plugins/<name@marketplace>` |
| `marketplaces.<name>` | `[A-Za-z0-9][A-Za-z0-9._-]*`, and must equal the name in the marketplace's own `marketplace.json` |
| `plugins.enabled` | `name@marketplace`; the marketplace must be in `[marketplaces]` unless it is `anthropic-plugin-directory`. No duplicates. |

`colctl add plugin`, `rm plugin`, `adopt` and `init` edit this file in place and keep its comments and formatting.
