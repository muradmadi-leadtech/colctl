# Why checks are saved, and when they're redone

`colctl status` and `colctl review` used to read every held version from scratch: its commits, every changed file and
each file's risk flags. A plugin's update is often 50 or 60 commits touching hundreds of files, and git was asked
for each file's contents in its own process. With two such updates held, that was 2 227 processes and 14 seconds,
every time you ran `status` or opened the TUI.

Two changes fix that, and they solve different halves.

**Files are read in one git process.** `git cat-file --batch` hands over every blob through one pipe, so the first
check of a held version costs about a second instead of fourteen. That check happens once, when `update` (or
`refresh`) holds the version, so you pay for it while you're already waiting on the network.

**The check is saved.** A held version doesn't change until you decide it, so what `review` found can be kept. Every
later `status`, `review` and TUI start reads it back in a few milliseconds.

## Never stale

A saved check is only as good as its key. colctl keys it by exactly what was held:

- for a source plugin, the pin it was held against and the held commit;
- for a taken-over skill or agent, the hash of its held snapshot;
- and in both cases, colctl's version and the version of the risk rules.

If any of these differ, the check is redone: a newer update replaces the hold; a pin edited by hand shows its
warning; an edited snapshot is refused as before; a new colctl or new malware patterns check everything again.
Approving or rejecting forgets the check. A check that can't be read is simply redone.

The diff is never saved. It's the biggest part and the one you read least, and git produces it quickly on its own.

## Derived, not state

The saved checks and the TUI's saved status live in `~/.colctl/.state/cache/`. Nothing there is the source of
truth: deleting it costs one slow run and changes nothing else. `colctl status` itself still writes nothing but these
checks; the TUI's saved status is written only by the TUI.
