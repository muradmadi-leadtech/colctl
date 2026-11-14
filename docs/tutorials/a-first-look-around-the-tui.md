# A first look around colctl's TUI

In this tutorial you'll open colctl's full-screen view, find what needs you, review a held update, and sync, all
without typing a command. It takes about five minutes. You need colctl 2.4 or later, and `~/.colctl` set up (see
[Your first sync](first-sync.md)).

## 1. Open it

```sh
colctl
```

You land on **Overview**. Its first line is the same verdict `colctl status` prints, such as `✓ All 5 profiles in
sync` or `! 2 things need attention`. Below it are your profiles, one per line, with what each has. If anything needs
you, it's listed under **Needs you**, with the command that fixes it.

The bar at the bottom shows the keys that work here. Press `?` for all of them, and Esc to close the list.

## 2. Look around

Press `2` for **Profiles**, `3` for **Items**, or Tab to go through the tabs in order. You can also click a tab.

On **Items**, every skill, agent and plugin appears once, with a symbol for each profile: ✓ fine, ! needs you, –
not there. The numbers in the header match the legend above them. Press `/`, type part of a name, and ⏎. Then ⏎
again on an item to see where it lives in each profile.

## 3. Review what's held

If the tab bar says `Review !1`, an update is waiting. Press `4`, then ⏎ on it. You see exactly what
`colctl review <name>` prints: commits, files, risk flags first, then the diff. Scroll with ↓ and PgDn, or press `]`
to jump to the next file.

Press `a` to approve or `x` to reject. A confirmation says what will happen. Press `y` to go ahead or `n` to change
nothing. While it runs, the status line shows each profile's progress. When it's done, the tab reloads.

No update held? Press `u` to check for updates. colctl fetches each source and holds anything new for review.

## 4. Sync

Back on **Overview** (`1`), press `s`, read the confirmation, and press `y`. Each profile shows its steps as they run,
then ✓ or ✗. The verdict at the end is the one `colctl sync` would print.

## 5. Quit

Press `q`. If something is still running, colctl asks first and finishes it before quitting, so nothing is left half
done.

## Where next

- [The TUI reference](../reference/tui.md): every key, when it opens, the environment variables.
- Everything here is also a command: `colctl status`, `review`, `approve`, `sync`. Scripts should use those, with
  `--json`.
