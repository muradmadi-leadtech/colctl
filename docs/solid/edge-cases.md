# Edge cases: solid

| ID | Category | Case | Expected |
|---|---|---|---|
| EC9-01 | Empty | `.state/cache/` doesn't exist | Created on first write. Reads behave as a miss |
| EC9-02 | Invalid | A saved check is truncated or isn't JSON | Treated as a miss: inspected again and rewritten. No error shown |
| EC9-03 | Invalid | A saved check's key doesn't match its file name (hand-copied) | Treated as a miss |
| EC9-04 | Failure | `.state/cache` can't be written (read-only, disk full) | The command works as in 2.5, inspecting every time. One `-v` note, nothing else |
| EC9-05 | Concurrent | Two colctl processes inspect the same held version | Each writes to a temporary file and renames it, so neither reads half a file |
| EC9-06 | Concurrent | `update` runs while the TUI shows a saved status | The TUI's next refresh shows the new hold. Nothing is served from an entry for the old hold |
| EC9-07 | Timing | The clock goes backwards, or the saved status is from the future | Age shown as "earlier". The fresh snapshot replaces it as usual |
| EC9-08 | Boundary | A held version with 0 changed files (pin moved back to a parent) | Saved and served like any other; "goes back n commits" as today |
| EC9-09 | Boundary | 20 held versions | Saved checks prune to those held; status stays under 1 s |
| EC9-10 | Failure | The held commit was garbage-collected from the clone | The existing "held commit … is missing" error, never a saved check |
| EC9-11 | Invalid | `adopt`'s candidate list has an item that disappears before confirm | Adopt reports it skipped with the reason; the rest are adopted |
| EC9-12 | Empty | Discover's Connectors view with no Codex profile | "No Codex profile, so no connectors", as the CLI says |
| EC9-13 | Failure | `marketplace refresh` fails for one tool | The other tool's refresh still runs; the result overlay shows both lines |
| EC9-14 | Boundary | Palette with a query matching nothing | "No actions match "<q>"". Esc clears |
| EC9-15 | Invalid | A palette action not valid in its tab now (e.g. approve with nothing held) | Listed last, dimmed, with the reason; ⏎ shows the reason in the status line |
| EC9-16 | Concurrent | A key is pressed for a write while another write runs | Ignored with "Wait for <job> to finish", as in 2.5 |
| EC9-17 | Boundary | `x` on the last profile's only copy of a plugin someone else installed | Same refusal as `colctl rm`: colctl only removes what it owns |
