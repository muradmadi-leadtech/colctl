# Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. For `status`: everything is in sync and the source is committed. |
| 1 | Out of sync, or one or more actions failed, were skipped or were refused. The output says which and why. |
| 2 | Usage, config, state or environment error: a bad argument, invalid `config.toml`, a corrupt state file, no source yet, `git` not found, an unknown harness or item. Nothing was changed. |
| 3 | Another colctl is running (it holds `~/.colctl/.state/lock`). |
