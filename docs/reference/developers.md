# colctl developers

```
colctl developers [<developer>] [--json]
```

Without a name, lists every developer in the [catalogs](../explanation/catalogs.md) with their plugin count and the
catalogs they appear in, by count, then name:

```
anthropics      58 plugins  anthropic-plugin-directory, claude-plugins-official
obra             3 plugins  anthropic-plugin-directory, claude-plugins-official, colctl
```

With a name, lists that developer's plugins exactly as `colctl search --developer <name> --limit 0` does. Names compare
case-insensitively.

A developer is the owner of the plugin's repo (the first part of its path on GitHub or another host, lowercased). For a
plugin without a hosted repo it's the manifest's author name, or `unknown`.

`--json`: `{"version": 1, "developers": [{"name": "obra", "plugins": 3, "catalogs": ["…"]}], "warnings": []}`;
with a name, the [`search`](search.md#--json) shape.

Exit codes: **0**; **1** when there are no catalogs, or the developer has no plugins.
