# Acceptance criteria: Discovery

_Cycle 5 · Refine · 2026-11-13_

IDs are `AC5-NN`; each needs a test whose name contains `ac5_NN`. Formats are in [interfaces.md](interfaces.md),
edge cases in [edge-cases.md](edge-cases.md). Tests use fixture profile folders (manifests, a small directory cache)
in a temp home, and the fake `claude`/`codex`.

## Catalog (T1)

**AC5-01 Claude marketplaces are read**
```gherkin
Given a Claude profile whose known_marketplaces.json lists marketplace m with installLocation L
When the catalog is built
Then every plugin in L/.claude-plugin/marketplace.json is an entry with id <name>@m, tool claude, its description,
  keywords and tags, author, category, version and homepage
And its repo and subpath come from its source: a relative path (the marketplace's own repo + that folder), github,
  url, git, or git-subdir (url + path, with sha as the commit); a command source has no repo
```

**AC5-02 The plugin directory is read**
```gherkin
Given a Claude profile with plugins/plugin-directory-cache-v2.json (schemaVersion 2)
When the catalog is built
Then every listing whose surfaces include claude_code is an entry with id <entryName>@anthropic-plugin-directory,
  its summary as description, its release version, and its repository url, path and reviewed_commit
```

**AC5-03 Codex marketplaces on disk are read**
```gherkin
Given a Codex profile whose config.toml has [marketplaces.m] with a local source folder F
When the catalog is built
Then every plugin in F/.agents/plugins/marketplace.json is an entry with id <name>@m and tool codex
```

**AC5-04 colctl's sources are entries**
```gherkin
Given a [[source]] superpowers in config.toml
When the catalog is built
Then its packaged plugins are entries from the catalog "colctl", marked as source plugin superpowers
```

**AC5-05 Where each plugin is installed**
```gherkin
Given plugins installed (user scope) in some Claude profiles and enabled in some Codex profiles
When the catalog is built
Then each entry lists the harnesses where its id is installed
```

**AC5-06 One bad catalog doesn't stop the rest**
```gherkin
Given one marketplace manifest that isn't valid JSON
When I run colctl search
Then the other catalogs are searched, and stderr has "warning: <file>: <error>"
```

**AC5-07 git-subdir sources are understood everywhere**
```gherkin
Given a plugin whose marketplace source is {"source": "git-subdir", "url": U, "path": P, "sha": S}
Then the catalog and colctl adopt (cycle 3's origin lookup) both resolve it to repo U, subpath P, commit S
```

## Search (T2)

**AC5-08 The same plugin is one result**
```gherkin
Given the same plugin (same normalised repo, subpath and plugin name) in claude-plugins-official, the plugin
  directory and colctl's sources
When it matches a search
Then it is one result, listing every id and catalog it appears under
```

**AC5-09 Matching and ranking**
```gherkin
Given terms "api security"
When I search
Then a result matches only if every term appears (case-insensitive) in its name, display name, keywords, description,
  developer or author
And results rank by where the terms hit: exact name, then name, then keywords, then description, then developer;
  ties by name
```

**AC5-10 Filters**
```gherkin
When I search with --tool claude|codex, --marketplace m, --developer d, --installed or --limit n
Then only matching results are shown, at most n (default 20; 0 for all), and the footer says how many matched
```

**AC5-11 Developers**
```gherkin
Given entries with repos on GitHub (or another host) and entries without a repo
Then an entry's developer is its repo's owner (the first path segment, lowercased), else its author name, else
  "unknown"
```

## search and show (T3)

**AC5-12 search prints results**
```gherkin
When I run colctl search api security
Then each result is one line: id, developer, description (cut to fit), and [installed: …] when installed
And the footer reads "<shown> of <matched> results" with the hint to use --limit when some are hidden
And with no terms, everything is listed by name (still limited)
And the exit code is 0 with results, 1 with none ("no plugins match")
```

**AC5-13 search --json**
```gherkin
When I run colctl search … --json
Then the output is the JSON in interfaces.md: matched count, results with every field, and warnings
```

**AC5-14 show**
```gherkin
When I run colctl show <id> (any id of a merged result, or a bare name that's unique)
Then it prints the name, ids and catalogs, developer, author, description, keywords, repo, subpath, version or
  commit, homepage, where it's installed, and what to run: "colctl add source <id>", "already source plugin <name>",
  or for no repo "colctl add plugin <id>"
And --json gives the same as one result object
```

**AC5-15 Fast**
```gherkin
Given catalogs with 4,000 plugins
When I run colctl search with two terms
Then it finishes in under 1 second
```

**AC5-16 Read-only and offline**
```gherkin
When I run search, show, developers or marketplace list
Then nothing under the home changes, the lock isn't taken, and no claude or codex command runs
```

## developers (T4)

**AC5-17 Listing developers**
```gherkin
When I run colctl developers
Then each developer is one line with their plugin count and catalogs, by count then name; --json has the same
```

**AC5-18 One developer**
```gherkin
When I run colctl developers obra
Then it lists obra's plugins (case-insensitive), the same results as colctl search --developer obra --limit 0
```

## Marketplaces (T5)

**AC5-19 marketplace list**
```gherkin
When I run colctl marketplace list
Then every marketplace any profile knows is one line: name, tool, source, plugin count, the profiles that have it,
  and "managed" when config.toml declares it; the plugin directory is listed as anthropic-plugin-directory (built in)
And --json has the same
```

**AC5-20 marketplace refresh**
```gherkin
When I run colctl marketplace refresh
Then each Claude profile runs claude plugin marketplace update, and each Codex profile codex plugin marketplace upgrade,
  with colctl's usual environment rules, and the result per profile is printed
And --dry-run prints the calls and runs none; exit 1 if any call failed
```

## connectors (T6)

**AC5-21 connectors**
```gherkin
Given Codex profiles
When I run colctl connectors gmail
Then codex plugin list --available --json runs once per profile (in parallel), and entries with a remote source are
  listed, de-duplicated by id, filtered by the terms, with [installed: …]
And --json gives them as results with kind "connector"
```

**AC5-22 Connectors stay out of search**
```gherkin
When I run colctl search gmail
Then no connector is listed, and no codex command runs
```

## Add from a result (T7)

**AC5-23 add source <id>**
```gherkin
Given a search result superpowers@claude-plugins-official whose repo is github.com/obra/superpowers
When I run colctl add source superpowers@claude-plugins-official
Then it does what colctl add source https://github.com/obra/superpowers --plugin superpowers does, named superpowers
And when the catalog has a commit (directory reviewed_commit, git-subdir sha), that commit is the pin unless --rev
  is given
```

**AC5-24 A plugin in a subfolder**
```gherkin
Given a result whose repo is a marketplace repo with the plugin in plugins/x (relative source, git-subdir or a
  directory path)
When I run colctl add source <id>
Then the source is the repo with path = "plugins/x", and the plugin installs from that folder
```

**AC5-25 Results without a repo**
```gherkin
Given a result with a command source, or a connector
When I run colctl add source <id>
Then it exits 2: "<id> has no repo to clone; colctl add plugin <id> keeps it on the marketplace list"
```

**AC5-26 Already a source**
```gherkin
Given a result that is already source plugin superpowers
When I run colctl add source <its id>
Then it exits 2: "<id> is already source plugin superpowers"
```

## Docs and traceability (T8)

**AC5-27 Docs**
```gherkin
Then docs/reference has search.md, show.md, developers.md, marketplace.md and connectors.md; add.md covers
  add source <id>; commands.md lists the new commands; docs/how-to/find-and-add-a-plugin.md and
  docs/explanation/catalogs.md exist; every relative link resolves
```

**AC5-28 Traceability and earlier suites**
```gherkin
Then every AC5/EC5 ID has a test named after it, and every earlier suite keeps its test count with none ignored
```
