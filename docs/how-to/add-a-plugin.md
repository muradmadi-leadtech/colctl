# Add a plugin

```sh
# A plugin from a marketplace colctl doesn't know yet
colctl add plugin sadr@sadr --marketplace github:muradmadi/sadr

# Another plugin from a marketplace already in config.toml
colctl add plugin other@sadr

# A plugin from Anthropic's plugin directory (no marketplace needed)
colctl add plugin superpowers@anthropic-plugin-directory
```

colctl adds the plugin (and the marketplace, if new) to `~/.colctl/config.toml`, keeping your comments, commits,
then syncs. Syncing runs, for each profile:

```sh
claude plugin marketplace add muradmadi/sadr     # once per profile that lacks it
claude plugin install sadr@sadr
```

with `CLAUDE_CONFIG_DIR` pointing at that profile (unset for `~/.claude`).

## The marketplace name must match

The name before `=` in `[marketplaces]` must be the name the marketplace gives itself in its `marketplace.json`. If
it isn't, sync reports `config calls it "x", the marketplace calls itself "y"`; rename the key in `config.toml`.

## Removing a plugin

```sh
colctl rm plugin sadr@sadr
```

colctl uninstalls it only from profiles where colctl installed it. A copy you installed by hand stays, and shows up
as foreign.

## Updating plugins

That's Claude Code's job: `claude plugin update`. colctl only makes sure every profile has the same set.
