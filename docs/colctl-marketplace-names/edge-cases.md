# Edge cases: colctl marketplace names

| ID | Category | Case | Expected |
|---|---|---|---|
| ECH2-01 | Invalid | The repo's entry names a folder with no plugin manifest (Claude's `strict: false`: the entry itself defines the plugin) | Wrapped all the same: the entry's fields carry the definition |
| ECH2-02 | Boundary | The repo's entry points at a folder outside the clone (`../x`) | Never wrapped from outside: the repo's own marketplace is used, as before |
| ECH2-03 | Boundary | A plain plugin-list entry `<plugin>@<repo marketplace>` (not a source) | Untouched: only sources are renamed |
| ECH2-04 | Failure | The old marketplace is also used by a plugin-list entry | It stays; only the source moves |
