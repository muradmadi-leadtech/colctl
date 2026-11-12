# colctl docs

Two kinds of documentation live here.

**Intent** is captured per [FORGE](https://github.com/scottfeltham/forge-framework) cycle, in FORGE's own layout:
`prd/<cycle>.md`, `tasks/<cycle>.md` and `<cycle>/`. These are written before the code and are the yardstick it's
evaluated against. Cycle state lives in `../.forge/`.

**User docs** follow [Diataxis](https://diataxis.fr), four folders, each answering one kind of need:

| Folder | Answers | Example |
|---|---|---|
| [tutorials/](tutorials/) | "Teach me" — a guided first run | Your first `colctl init` and `sync` |
| [how-to/](how-to/) | "Help me do X" — recipes for a goal | Add a second Claude profile; bring existing skills under colctl |
| [reference/](reference/) | "What exactly does X do" — complete, dry facts | Every command and flag, `config.toml`, exit codes |
| [explanation/](explanation/) | "Why is it like this" — background and trade-offs | Why links and not copies; what "owned" means |

A feature isn't done until its reference page exists.
