# Your first sync

In this tutorial you'll put colctl in charge of two Claude Code profiles, bring over what they already have, and see
an edit reach both of them. It takes about ten minutes.

You need colctl installed (`mise use -g github:muradmadi-leadtech/colctl`), `git`, Claude Code, and at least your
normal `~/.claude` profile. A second profile, such as kilo-proxy's, makes it more interesting; launch it once first
so its folder exists.

## 1. Create the source

```sh
colctl init
```

colctl creates `~/.colctl`, registers every Claude profile it recognises, makes a git repo there, and prints a status
report. Open the config it wrote:

```sh
cat ~/.colctl/config.toml
```

You'll see one `[[harness]]` block per profile, such as `~/.claude` and, if you have kilo-proxy,
`~/Library/Application Support/kilo-proxy-headless/profiles/claude`.

## 2. See what's out of step

```sh
colctl status
```

Anything you already had (skills, agents, a `CLAUDE.md`, plugins) shows up as **foreign**: it exists in a profile
but colctl doesn't manage it yet. Each line ends with the command that fixes it.

## 3. Bring it all in

```sh
colctl adopt --dry-run    # see what would happen
colctl adopt
```

For each foreign item, colctl copies it into `~/.colctl`, checks that the copy is identical, and replaces the
original with a link. Your `CLAUDE.md` becomes `~/.colctl/AGENTS.md`, and plugins are added to `config.toml`. Then it
syncs, so everything you had in one profile now reaches the others too.

If the same skill exists in two profiles with different content, colctl refuses that one and shows you both paths.
Pick the version you want, delete the other, and run `colctl adopt` again.

## 4. Edit once, see it everywhere

```sh
echo "Always answer in British English." >> ~/.colctl/AGENTS.md
```

Start Claude Code in either profile and ask it what its instructions say about spelling. Both read the same file:
each profile's `CLAUDE.md` is just an import of `~/.colctl/AGENTS.md`.

## 5. Check and record

```sh
colctl status
```

The status lists `AGENTS.md` as **uncommitted**. Run `colctl sync` and it commits the edit to `~/.colctl`'s git
history, then reports `in sync`.

```sh
colctl sync
git -C ~/.colctl log --oneline
```

## Where next

- [Add a Claude profile](../how-to/add-a-claude-profile.md)
- [Add a plugin](../how-to/add-a-plugin.md)
- [Why links and not copies](../explanation/links-not-copies.md)
