# How colctl talks to you

colctl is a command-line tool first, so what it prints carries the whole experience. Version 2.2 treats that output as
designed, the way cargo, uv, gh or jj do, rather than as a log.

## Answer first

You run `colctl status` to learn one thing: is everything fine, and if not, what do I do? So the first line answers
that, and the rest only exists to back it up. On a machine with five profiles and a hundred items all in sync, that's
seven lines, not a hundred. Asking for everything (`--all`) is one flag away.

## Say what happened, not how

A sync calls `claude` and `codex` many times. Those calls are how colctl does its work, not what you asked for, so a
run reports outcomes ("installed superpowers · linked skills a, b") and keeps the calls for `-v`. When something
fails, you get the step, the tool's whole error, and how to retry.

## Problems carry their fix

Every state that needs you comes with the command that resolves it. Informational states, like a plugin its repo
doesn't package for Codex, are marked `·` and never counted as problems, so colctl can't say "needs attention" and
"in sync" in the same breath.

## Colour is a second channel

Each state has a symbol and a word; colour only reinforces them. That keeps output readable with `NO_COLOR`, in logs,
for people who don't see colour well, and when pasted into an issue.

## Nothing important is cut

Terminals are narrow. colctl measures yours and fits to it, but chooses what gives: a description is cut before a
plugin id, and a fix, a risk flag or a run's outcomes move to the next line rather than lose words. A hostile repo
can't use long names or escape codes to hide or rewrite what you see.

## Ask only when it helps

Removing a plugin from every profile, or approving everything held at once, is worth a question. Picking a held
version from a list beats retyping its name. But a script must never hang on a prompt, so colctl only asks when both
your input and output are a terminal, never with `--json` or in CI, and `--yes` always answers for you.

## Text is for people, JSON for programs

Because the text is designed for reading, it will keep improving. Anything you script against should use `--json`
and exit codes, which stay stable within a major version.
