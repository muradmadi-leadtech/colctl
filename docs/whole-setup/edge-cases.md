# Edge cases: whole setup

| ID | Category | Case | Expected |
|---|---|---|---|
| EC10-01 | Empty | No `[[mcp]]`, no `commands/`, no `hooks/` | Status and sync are as in 2.6. No MCP lines, no `colctl-setup` |
| EC10-02 | Empty | A profile has no `.claude.json` / no `[mcp_servers]` | Read as no servers. The tool's CLI creates the file on the first add |
| EC10-03 | Invalid | A profile's `.claude.json` or `config.toml` doesn't parse | That profile's MCP items are `unreadable`, with the file named. Other kinds and profiles carry on. Nothing is added there (it could duplicate) |
| EC10-04 | Invalid | An env key or header name with `=` or a newline | Config error naming the entry |
| EC10-05 | Invalid | `{ env = "" }`, `{ keychain = "" }`, or a table with both keys | Config error |
| EC10-06 | Boundary | An arg starting with `-` (e.g. `--stdio`) | Passed after `--`, so neither CLI reads it as its own flag |
| EC10-07 | Boundary | A literal value containing `${…}` | Passed as is. Claude will expand it, and `status` notes "Claude expands ${…} in this value" |
| EC10-08 | Boundary | An MCP server and a plugin's bundled server with the same name | They're different things (`plugin:<p>:<name>` in Claude). Both shown, no conflict |
| EC10-09 | Boundary | A server name that is valid for colctl but not for one tool | Checked at load against the stricter of both. Error names the rule |
| EC10-10 | Failure | `claude mcp add` fails for one profile | That item failed with the tool's message (redacted). Other profiles and items go on. Nothing recorded for it |
| EC10-11 | Failure | `mcp remove` reports "not found" (removed by hand meanwhile) | Treated as removed. Forgotten in state |
| EC10-12 | Failure | `security` isn't there (Linux) or the item doesn't exist / access denied | That server is skipped in every profile that needs the value: "keychain item gh: <reason>". Exit 1 |
| EC10-13 | Failure | A keychain prompt is cancelled by the owner | Same as EC10-12, reason "access denied". Asked once per run, not once per profile |
| EC10-14 | Timing | A secret's value changes in the keychain | Status can't see it (it never resolves). `sync --replace-mcp <name>` re-adds with the new value. The how-to says so |
| EC10-15 | Concurrent | The tool edits `.claude.json` while colctl reads it | colctl reads once per run. A half-written file is EC10-03 for that run |
| EC10-16 | Concurrent | Claude Code is running while sync re-adds a server | The CLI handles it, as for plugins. Docs: restart the session to pick it up |
| EC10-17 | Invalid | Leak-check false positive (e.g. a long hex id that isn't secret) | `# colctl: not-secret` on that line silences it. Documented. Tested both ways |
| EC10-18 | Invalid | A command file without frontmatter or with an empty body | Linked for Claude as is. For Codex, the description is the first non-empty line, or "Command <name>" |
| EC10-19 | Boundary | A command named like an existing skill | Codex skills live inside `colctl-setup` (namespaced), so no clash. Claude: a command and a skill may share a name, and that's noted in status |
| EC10-20 | Invalid | `hooks.json` doesn't parse or isn't Claude's format | Hooks item `invalid` with the parser's message. The previously installed `colctl-setup` is left as it was |
| EC10-21 | Boundary | A hook script that isn't executable | Copied with its mode. Status flags "guard.sh isn't executable" |
| EC10-22 | Boundary | Only commands, no hooks | `colctl-setup` is built for Codex (skills) only. Claude gets links and no plugin |
| EC10-23 | Failure | A foreign marketplace already called `colctl-setup` | Not replaced. Status names it and the fix (`claude plugin marketplace remove colctl-setup`) |
| EC10-24 | Invalid | `adopt`: two profiles have servers with the same name and different commands | Offered as two choices ("toolport from claude" / "from codex"). Adopting one makes the other `taken` |
| EC10-25 | Boundary | `adopt`: identical server in all profiles | One entry, no overrides |
| EC10-26 | Invalid | `adopt` hooks: a profile's `settings.json` has a hook with a relative script path | Copied as is, with a warning that the path is relative to where Claude runs, not to `~/.colctl/hooks` |
| EC10-27 | Failure | The commit is refused by the leak check during `add mcp` | Nothing synced. `config.toml` is restored to before the command. The message names the key |
| EC10-28 | Boundary | `--only claude` on a server that's already in Codex profiles | Codex copies become `orphaned` and are removed on that sync |
| EC10-29 | Invalid | Override for `agents` (no MCP) | Config error: "agents doesn't take MCP servers" |
| EC10-30 | Boundary | A Codex HTTP server with `headers` | `unsupported` in Codex profiles: "codex mcp add can't set headers". Claude profiles get it |
