# Edge cases: TUI

| ID | Category | Case | Expected |
|---|---|---|---|
| EC8-01 | Empty | `~/.colctl` doesn't exist | Overview: "colctl isn't set up yet. Quit and run colctl init." Other tabs show the same line, except Discover, which still searches catalogs |
| EC8-02 | Invalid | `config.toml` doesn't parse | The error overlay with the CLI's error, cause and hint, then Overview with the same message. The app stays open, and `r` retries |
| EC8-03 | Empty | No profiles registered | Overview's verdict says so, with `→ colctl init` |
| EC8-04 | Empty | No catalogs known | Discover: "No plugin catalogs yet", with the CLI's hint |
| EC8-05 | Empty | A search with no results | "No plugins match "<q>"", and Esc clears it |
| EC8-06 | Boundary | 1000+ items, or a 10 000-line diff | Lists and diffs scroll smoothly. Only visible rows render, and each frame takes ≤ 16 ms in the benchmark test |
| EC8-07 | Boundary | Names and paths with wide characters (CJK, emoji) | Columns stay aligned, using the CLI's width rules |
| EC8-08 | Boundary | Resize during a job or with an overlay open | The layout redraws at once, and the job and overlay are kept |
| EC8-09 | Boundary | Exactly 60×20 | Every tab draws without overlap. There's a snapshot for it |
| EC8-10 | Concurrent | Another colctl holds the lock when a write starts | Error overlay: "Another colctl is running", hint "Wait for it, then press the key again". Nothing changes |
| EC8-11 | Concurrent | The CLI changes `~/.colctl` while the TUI is open | Nothing changes on screen until `r` or the next write's reload, and the write uses the current state from disk |
| EC8-12 | Failure | One profile fails during sync | Its row ends ✗ with the reason, the others continue, and the verdict is the CLI's. ⏎ on the row shows the error |
| EC8-13 | Failure | `git` or `claude` missing | Reads work. Writes show the CLI's error with its hint |
| EC8-14 | Failure | A catalog fails to load | Discover shows the others and a warning line naming the catalog, as `search` warns |
| EC8-15 | Failure | Panic inside the worker thread | Reported as an error overlay ("colctl hit a bug"). The UI stays usable, and the next write starts a fresh worker |
| EC8-16 | Failure | Writing to the terminal fails (closed tty, SIGHUP) | The app exits and restores what it can. No partial write is left: the job finishes first, as for q |
| EC8-17 | Timing | Keys pressed faster than frames (key repeat, paste) | Every key is handled in order. A paste into the search line counts as one change |
| EC8-18 | Timing | A held item is approved from the CLI while the TUI shows it | `a` on it reports "superpowers is no longer held" from core, then reloads |
| EC8-19 | Timing | Esc during sync (not cancellable) | Toast "Sync can't be stopped half way". The job continues |
| EC8-20 | Invalid | `COLCTL_REDUCED_MOTION` set to something other than `1` | `0` or empty: motion on. Any other value: motion off |
| EC8-21 | Security | Item names, descriptions or diffs with escape sequences or control characters | Rendered escaped (as `review::escape` does), never sent to the terminal raw |
| EC8-22 | Security | The terminal is restored after the panic hook ran, and a nested panic happens in restore | The second panic is caught, and the process exits 101 without a loop |
| EC8-23 | Boundary | `TERM` unset | Treated like a terminal, if stdin and stdout are terminals |
| EC8-24 | Boundary | `colctl --color never` with no command | Opens the TUI without colour (global flags apply to the TUI) |
| EC8-25 | Boundary | `colctl -q` or `-v` with no command | The TUI opens; `-q` and `-v` don't apply to it |
| EC8-26 | Boundary | Search query with only spaces | Treated as empty: every plugin listed |
