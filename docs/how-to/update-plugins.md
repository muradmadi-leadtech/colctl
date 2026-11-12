# Update plugins

Source plugins stay on their pinned commit until you approve a new one.

```sh
colctl update --check        # what's newer upstream; changes nothing
colctl update                # hold every source's newest commit for review
colctl update superpowers    # just one
colctl update superpowers --rev v7.0.1    # hold a specific tag, branch or commit (also to go back)
colctl review superpowers    # what it changes, with risk flags
colctl approve superpowers   # move the pin and update every profile
```

See [Review an update before it installs](review-an-update.md) for the review step.

Each approval is a commit in `~/.colctl`, so `git -C ~/.colctl log -p config.toml` shows every pin change, and
reverting one and running `colctl sync` puts every profile back.

`status` and `sync` never look upstream, so they work offline and never change versions on their own.
