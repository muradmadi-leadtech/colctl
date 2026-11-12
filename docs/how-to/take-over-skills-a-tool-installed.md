# Take over skills a tool installed

Some CLIs install their skills into a folder of their own and symlink them into `~/.agents/skills` or
`~/.claude/skills`. twg does this with `~/.config/twg/skills`. `colctl status` shows each one as
`linked-elsewhere`, along with where it points.

## Take them over

```sh
colctl adopt --dry-run      # "would take over skills/twg-jira in agents from ~/.config/twg/skills/twg-jira"
colctl adopt                # all of them; or: colctl adopt agents skills/twg-jira
```

Each skill is copied into `~/.colctl/skills`, checked, and the link is pointed at colctl's copy. Sync then links
it into every other profile too. The tool's own folder isn't touched, and colctl remembers it as the skill's origin.

## When the tool updates its skills

`colctl status` reports `origin-changed` for each skill whose origin moved on:

```sh
colctl refresh              # or: colctl refresh skills/twg-jira — holds the new content for review
colctl review skills/twg-jira
colctl approve skills/twg-jira    # or: colctl reject skills/twg-jira
```

`refresh` doesn't change colctl's copy; it holds the tool's new version until you approve it (see
[Review an update before it installs](review-an-update.md)). Each approval is a commit in `~/.colctl`, so
`git -C ~/.colctl show` shows exactly what changed, and `git revert` undoes it. If you also edited colctl's copy,
refresh and approve refuse rather than overwrite your edit.

## When the tool re-links over colctl

Running the tool's installer again may point the links back at its folder. They show as `linked-elsewhere` again;
`colctl adopt` points them back at colctl's copy without copying anything, and tells you if there's an update to
refresh.
