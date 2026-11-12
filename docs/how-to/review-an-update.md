# Review an update before it installs

colctl holds new versions of your source plugins and taken-over skills until you've looked at them.

## 1. Fetch what's new

```sh
colctl update      # source plugins: fetch upstream, hold any newer commit
colctl refresh     # taken-over skills and agents: hold what changed in their origin folders
```

Nothing is installed yet. `colctl status` now lists each held version as `pending-review`.

## 2. See what's held

```sh
colctl review
```
```
sources/superpowers   8ca22db → 22f0b46   12 commits   3 flags
skills/twg-jira       ~/.config/twg/skills/twg-jira   2 files   1 flag
```

## 3. Look at one

```sh
colctl review superpowers | less -R
```

Start with the **flags**: they point at hooks, MCP servers, scripts, executables, symlinks and new tool permissions,
the changes that let code run on your machine or widen what an agent may do. Read those files in the diff. Then skim
the commit list and the rest of the diff.

A flag isn't a verdict. A release that adds a `SessionStart` hook may be perfectly fine; you're checking that it does
what the commit says.

## 4. Decide

```sh
colctl approve superpowers          # install exactly what you reviewed, in every profile
colctl reject skills/twg-jira       # drop it; this version won't be offered again
colctl approve --all                # everything held
```

`approve` installs the held version even if upstream has moved on while you were reading. To look at the newest
one instead, run `colctl update` again: it replaces the held version, and you review that.

## Undoing an approval

Each approval is a commit in `~/.colctl` (`colctl: approve source superpowers at 22f0b46`). Revert it and sync:

```sh
git -C ~/.colctl revert HEAD
colctl sync
```
