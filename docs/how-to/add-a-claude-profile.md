# Add a Claude profile

Any folder Claude Code uses as its config folder (`~/.claude`, or whatever a launcher passes as
`CLAUDE_CONFIG_DIR`) can be a harness.

## If colctl knows the profile

colctl recognises `~/.claude` and kilo-proxy-headless's `profiles/claude` on macOS. Start the new profile once so its
folder exists, then:

```sh
colctl init
```

Running `init` again only appends newly found profiles to `config.toml`; your edits are left alone.

## Any other profile

Add a block to `~/.colctl/config.toml`:

```toml
[[harness]]
name = "work-claude"        # lowercase letters, digits and dashes
kind = "claude"
path = "~/.config/work-claude"
```

Then:

```sh
colctl status --harness work-claude
colctl sync --harness work-claude
```

The folder must already exist. colctl never creates a profile folder, because the tool that owns the profile may
insist on creating it itself (kilo-proxy, for example, wants a private folder it made).

## Keep something out of one profile

List it under `exclude`:

```toml
exclude = ["skills/heavy-skill", "agents/reviewer.md", "CLAUDE.md", "plugins/sadr@sadr"]
```

colctl then never reports or touches those items in that profile, and removes its own link there if it had made one.
