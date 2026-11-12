# Move a plugin to its source

You installed a plugin the usual way, say superpowers from Claude's plugin directory, and want it from its
author's repo instead, in every Claude and Codex profile.

```sh
colctl status                       # from-aggregator  plugin  superpowers@anthropic-plugin-directory
                                    #   → source: https://github.com/obra/superpowers; colctl adopt claude plugin …
colctl adopt claude plugin superpowers@anthropic-plugin-directory
```

colctl clones `github.com/obra/superpowers` into `~/.colctl/plugins/superpowers` at the commit you have installed now,
so nothing changes yet but where it comes from. Then it:

- installs `superpowers@superpowers-dev` (the repo's own id) in every Claude and Codex profile, from that clone;
- uninstalls `superpowers@anthropic-plugin-directory` everywhere it's installed (its marketplace stays).

`colctl adopt` with no arguments does this for every plugin whose source it can find. Profiles where the repo has
no packaging (a Claude-only plugin in Codex, say) show the source as `unsupported`.

To get the newer version the author has published since, see [Update plugins](update-plugins.md).

## Adding one directly

```sh
colctl add source https://github.com/obra/superpowers
```

## A plugin you already added to colctl's list

If you adopted the plugin into `[plugins]` earlier, `status` shows it as `ok` with
`source available: https://github.com/obra/superpowers; colctl adopt claude plugin superpowers@anthropic-plugin-directory`.
Run that command: it moves the plugin from the list to its source in one step.
