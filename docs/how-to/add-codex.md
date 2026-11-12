# Add Codex

colctl keeps Codex CLI profiles in step with the same source as your Claude profiles.

## 1. Register the profiles

Run Codex (and `kilo-codex`, if you use it) once so their folders exist, then:

```sh
colctl init
```

`init` appends whichever of these it finds and doesn't already know:

| Harness | Kind | Folder |
|---|---|---|
| `codex` | `codex` | `~/.codex` |
| `kilo-codex` | `codex` | `~/Library/Application Support/kilo-proxy-headless/profiles/codex` |
| `agents` | `agents` | `~/.agents` (where Codex reads skills from) |

For another Codex profile, add a block with `kind = "codex"` and its `CODEX_HOME` as the path.

## 2. Sync

```sh
colctl sync
```

Each Codex profile now has:
- `AGENTS.md`, a link to `~/.colctl/AGENTS.md`
- `agents/<name>.toml`, generated from each `~/.colctl/agents/<name>.md`

and `~/.agents/skills` has a link to each of your skills.

## 3. Add Codex plugins

Codex plugins are a separate list, because their ids and marketplaces aren't Claude's:

```sh
colctl add plugin superpowers@superpowers-dev --codex --marketplace github:obra/superpowers
colctl add plugin github@openai-curated --codex      # Codex's built-in marketplace
```

The name before `@` and the marketplace name must be what the marketplace calls them. If sync reports
`config calls it "x", the marketplace calls itself "y"`, rename the key in `[codex.marketplaces]`.

## 4. Bring in what Codex already has

```sh
colctl adopt --dry-run
colctl adopt
```

This adopts a hand-written `AGENTS.md`, skills in `~/.agents/skills`, and installed Codex plugins. A hand-made
`agents/*.toml` is refused: recreate it as `~/.colctl/agents/<name>.md` and colctl will generate the TOML.
