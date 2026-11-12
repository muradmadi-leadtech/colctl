# Update plugins

Source plugins stay on their pinned commit until you move them.

```sh
colctl update --check        # what's newer upstream; changes nothing
colctl update                # move every source to its newest commit, then update every profile
colctl update superpowers    # just one
colctl update superpowers --rev v7.0.1    # a specific tag, branch or commit (also to go back)
```

Each move is a commit in `~/.colctl`, so `git -C ~/.colctl log -p config.toml` shows every pin change, and reverting
one and running `colctl sync` puts every profile back.

`status` and `sync` never look upstream, so they work offline and never change versions on their own.
