# colctl rm

```
colctl rm skill <name> [--dry-run]
colctl rm agent <name> [--dry-run]
colctl rm plugin <name@marketplace> [--codex] [--dry-run]
colctl rm source <name> [--dry-run]
```

Removes the item from the source (or from `[plugins].enabled`), commits (`colctl: rm <kind> <name>`), then runs
[sync](sync.md), which removes colctl's links from every profile and uninstalls the plugin wherever colctl installed
it. A plugin's marketplace stays in `config.toml`. Copies you installed by hand are left alone (and then show as
foreign).

Exit 2, with nothing changed, if no managed item has that name.

With `--codex`, the plugin leaves `[codex.plugins]`, is committed as `colctl: rm codex plugin <id>`, and sync runs
`codex plugin remove` in each codex harness where colctl installed it.

`colctl rm source <name>` removes the `[[source]]` entry (`colctl: rm source <name>`). Sync then uninstalls its plugin
and removes its local marketplace wherever colctl installed them, and deletes its clone (and any manifest colctl
wrote for it).
