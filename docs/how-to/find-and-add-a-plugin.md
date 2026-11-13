# Find a plugin and add it from its source

## 1. Search

```sh
colctl search api security
colctl search --developer obra
colctl search --tool codex --limit 0
```

`search` reads the catalogs Claude and Codex already keep, so it's offline and instant. If a marketplace looks stale,
`colctl marketplace refresh` asks the tools to update theirs first.

## 2. Look closer

```sh
colctl show 42crunch-api-security-testing@claude-plugins-official
colctl developers obra        # everything else this developer publishes
```

`show` tells you where the code lives (`repo`, and `subpath` when it's a folder in a bigger repo), the version or commit
the catalog lists, and where it's installed already.

## 3. Add it from its source

```sh
colctl add source 42crunch-api-security-testing@claude-plugins-official
```

colctl clones the plugin's own repo into `~/.colctl/plugins`, pins the commit the catalog lists, and installs it into
every Claude and Codex profile the repo packages it for. From then on, new versions are held for your review: see
[Review an update before it installs](review-an-update.md).

Plugins without a repo (built by a command, or Codex connectors) can't come from source. `show` says so, and suggests
`colctl add plugin <id>` to keep them on colctl's marketplace list instead.
