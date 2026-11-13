# System context (C4 level 1): CLI output design

```mermaid
flowchart TB
    person(["Person at a terminal"])
    script(["Scripts, CI, the future TUI"])
    colctl["<b>colctl</b><br/>commands → output layer"]
    term[("Terminal<br/>width, colour support, TTY or pipe")]
    env[("Environment<br/>NO_COLOR, CLICOLOR_FORCE, PAGER, CI, TERM")]
    pager["Pager ($PAGER, less -R)"]
    tools["claude / codex / git"]

    person -->|"commands, answers to prompts and pickers"| colctl
    colctl -->|"styled, width-aware output; live progress; prompts"| term
    colctl -->|"long output, in a terminal"| pager
    script -->|"--json, exit codes (unchanged)"| colctl
    colctl -->|"reads"| env
    colctl -->|"runs (shown as progress; details with -v)"| tools
```

**Boundaries**
- The output layer decides how things look and whether to ask; commands decide what to say. `--json` and exit codes
  bypass the output layer's styling and are unchanged.
- Interaction (prompts, pickers, pager) happens only when stdin and stdout are terminals and `--json` isn't set;
  otherwise colctl behaves exactly as a script expects.
- No full-screen mode: everything prints into the normal scrollback.
