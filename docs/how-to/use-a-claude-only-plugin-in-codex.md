# Use a Claude-only plugin in Codex

Some plugins only package for Claude Code. `colctl status` shows them on your Codex profiles as `not packaged`:

```
  ✓  codex   ~/.codex   2 plugins · skills via ~/.agents · 1 not packaged
```

`colctl status --all` names the plugin and the fix:

```
  ·  unsupported  source  qdrant-skills  → the repo has no Codex packaging; colctl convert qdrant-skills packages it
```

## 1. Convert it

```sh
colctl convert qdrant-skills
```

colctl builds a Codex package from the pinned clone and holds it. It says what the package carries and what it
can't: commands and agents have no Codex equivalent in a plugin, and some hooks never fire in Codex.

## 2. Review it

```sh
colctl review qdrant-skills
```

Read the hooks first: they run code on your machine. Each is shown with its event, matcher and command, and flagged.

## 3. Approve it

```sh
colctl approve qdrant-skills
```

Every Codex profile installs it. If it has hooks, open Codex and trust them with `/hooks`; Codex won't run them
before you do.

## Updates

`colctl update` holds a new version of the plugin together with its new Codex package. `colctl review` shows what
changes in each; approving installs both, rejecting keeps both where they were.

## Undo

```sh
colctl convert --undo qdrant-skills
```

Codex profiles uninstall it; Claude profiles keep it.

See [convert](../reference/convert.md) for what carries over, and [Converting plugins](../explanation/converting-plugins.md)
for why it works this way.
