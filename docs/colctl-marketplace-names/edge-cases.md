# Edge cases: colctl marketplace names

| ID | Category | Case | Expected |
|---|---|---|---|
| ECH2-01 | Invalid | The repo's entry names a folder with no plugin manifest (Claude's `strict: false`: the entry itself defines the plugin) | Wrapped all the same: the entry's fields carry the definition |
| ECH2-02 | Boundary | The repo's entry points at a folder outside the clone (`../x`) | Never wrapped from outside: the repo's own marketplace is used, as before |
| ECH2-03 | Boundary | A plain plugin-list entry `<plugin>@<repo marketplace>` (not a source) | Untouched: only sources are renamed |
| ECH2-04 | Failure | The old marketplace is also used by a plugin-list entry | It stays; only the source moves |
| ECH3-01 | Boundary | The plugin's manifest names component paths (`"commands": "./commands/"`), which Claude refuses when they resolve outside the plugin and marketplace folders | The wrapper holds a real copy of the plugin's folder, not a link into the clone, so every path resolves inside it |
| ECH3-02 | Boundary | The plugin's folder links to a folder elsewhere in the clone (`skills -> ../../../skills`) | The copy holds what the link points to; a link leading outside the clone, dangling or looping is left out |
| ECH3-03 | Boundary | An earlier colctl left a link in the wrapper, or the clone moved to a new pin | The next sync replaces it with a fresh copy, atomically; an unchanged copy isn't rewritten |
