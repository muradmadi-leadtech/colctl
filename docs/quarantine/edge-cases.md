# Edge cases: Quarantine

_Cycle 4 · Refine · 2026-11-12_

IDs are `EC4-NN`; each needs a test whose name contains `ec4_NN`.

## Empty / invalid

| ID | Case | Expected |
|---|---|---|
| EC4-01 | Nothing declared or adopted | `update`: "no sources declared", exit 0. `refresh`: as in 1.x. `review`: "nothing held for review", exit 0. `approve --all` / `reject --all`: "nothing held", exit 0 |
| EC4-02 | `review`/`approve`/`reject` a name that is declared but has nothing held | Exit 2: "nothing held for <name>" (for `approve`/`reject`, before anything changes) |
| EC4-03 | `review`/`approve`/`reject` an unknown name, or `approve`/`reject` with no names and no `--all`, or names together with `--all` | Exit 2 with usage; nothing changes |
| EC4-04 | A `quarantine` entry in state.json is malformed (a rev that isn't 40 hex characters, unknown kind) | Treated as corrupt state: exit 2 with the existing "state is corrupt" message |

## Boundary

| ID | Case | Expected |
|---|---|---|
| EC4-05 | `update --rev` resolves to the pin | "<name>: up to date"; nothing held; a held candidate stays |
| EC4-06 | Upstream's head goes back to the pin while a candidate is held | "<name>: up to date"; the held candidate stays and can still be reviewed, approved or rejected |
| EC4-07 | Upstream's head is a rejected commit | "<name>: <short> was rejected; nothing held", exit 0; `--check` doesn't count it |
| EC4-08 | The candidate is older than the pin (`--rev` to an earlier commit) | Held. `review` lists the commits it removes under "goes back n commits", and the diff runs from the pin to the candidate |
| EC4-09 | A plugin in a monorepo (`plugin` picks one of several) | `review` covers the whole repo diff, because manifests and shared scripts outside the plugin folder can affect it |
| EC4-10 | A held skill's origin returns to the content it had when adopted | `refresh`: "<key> is up to date with <origin>"; the held snapshot stays |
| EC4-11 | Approving one candidate leaves others held | Only the named ones change; the rest stay held and listed |

## Tampering and drift

| ID | Case | Expected |
|---|---|---|
| EC4-12 | A snapshot in `.state/quarantine` was edited after it was held | `review` and `approve` refuse: "the held copy of <key> changed since it was held; reject it and refresh again"; exit 1; nothing changes |
| EC4-13 | The candidate ref is missing or points elsewhere (clone re-made, ref edited) | `review` and `approve` refuse: "the held commit for <name> is missing; run colctl update <name>"; exit 1 |
| EC4-14 | The pin in config.toml was changed by hand after the candidate was held | `approve` refuses: "<name>'s pin changed since it was held; run colctl update <name> again"; exit 1. `review` still shows it, with a warning |
| EC4-15 | The source copy of a held skill was edited after it was held | `approve` refuses: "changed in both places", as `refresh` does; exit 1 |
| EC4-16 | The clone is dirty or invalid when approving | `approve` refuses before committing anything, naming the clone; exit 1 |

## Failure

| ID | Case | Expected |
|---|---|---|
| EC4-17 | Upstream unreachable during `update` | That source prints the error and holds nothing; the others continue; exit 1 |
| EC4-18 | Copying the origin into a snapshot fails, or the copy's hash doesn't match | Refused: "the copy didn't match the origin; nothing held"; no partial snapshot left; exit 1 |
| EC4-19 | After `approve` commits, sync fails in one harness | The approval stands (committed, candidate cleared); the failure is reported as sync reports it; exit 1; `colctl sync` retries |

## Concurrency

| ID | Case | Expected |
|---|---|---|
| EC4-20 | Another colctl run holds the lock | `update` (without `--check`/`--dry-run`), `refresh`, `approve` and `reject` exit 2 with the existing lock message. `review` and `status` don't take the lock |

## Security

| ID | Case | Expected |
|---|---|---|
| EC4-21 | A held skill contains a symlink that points outside it | The snapshot keeps it as a symlink (never followed or copied through), and it's flagged "points outside" |
| EC4-22 | File names, commit subjects or authors contain control characters or ANSI escapes | `review` and `status` text output shows them escaped (`\x1b`, `\n`), so they can't rewrite the terminal. JSON carries them as JSON strings. The diff body is passed through as text, with control characters other than tab and newline escaped |
| EC4-23 | A candidate contains a path with `..` or an absolute path in a symlink target | Never resolved by writing; it's only reported, and flagged "points outside" when it leaves the repo or skill |
