# Cycle: claude-profile-sync

**Created**: 2026-11-12T01:06:12.870968+00:00
**Priority**: high
**Status**: Active

## Overview

Cycle 1 of colctl: one source of truth in `~/.colctl` for `AGENTS.md`, skills, agents and plugins, kept in step across
every Claude Code profile on the machine (native `~/.claude` and kilo-proxy's `profiles/claude`) by symlinks, a
generated `CLAUDE.md` import, and Claude's own plugin CLI.

Docs: `docs/prd/claude-profile-sync.md`, `docs/tasks/claude-profile-sync.md`, `docs/claude-profile-sync/`.

---

<!-- FORGE_PHASE:Focus:Complete -->
## Phase 1: Focus

**Purpose**: Define what you're building and why.

### Required Outputs
- [x] Problem statement and target users defined
- [x] Testable success criteria written
- [x] System Context diagram (C4 L1) created
- [x] Clear boundaries on what you WON'T build

### Notes

Agreed with the user in conversation on 2026-11-12, backed by four probes against the real harnesses (see
`.forge/learnings.md`). Decisions: Rust; links where formats match; `CLAUDE.md` imports `AGENTS.md`; plugins via
`claude plugin` per profile; `~/.colctl` is a local git repo; public repo under muradmadi-leadtech.
PRD: `docs/prd/claude-profile-sync.md` (SC-1…8, in/out of scope). C4 L1: `docs/claude-profile-sync/system-context.md`.

---

<!-- FORGE_PHASE:Orchestrate:Complete -->
## Phase 2: Orchestrate

**Purpose**: Break the work into session-sized pieces.

### Required Outputs
- [x] Container architecture (C4 L2) designed
- [x] Component architecture (C4 L3) designed
- [x] Dependency map created
- [x] Tasks sized for single AI sessions

### Tasks

C4 L2 `docs/claude-profile-sync/containers.md`, C4 L3 `docs/claude-profile-sync/components.md`.
Tasks and dependency map: `docs/tasks/claude-profile-sync.md`.

- T0 Scaffold: cargo init, sadr init --public, publish.yml, coverage gate (DevOps)
- T1 Test harness: temp HOME, fake claude (Tester)
- T2 config · T3 fsx + state · T4 runner + git (parallel after T1)
- T5 claude adapter scan + source → T6 plan (pure) → T7 apply + plugin actions
- T8 init + status · T9 sync/add/rm · T10 adopt (parallel after T7)
- T11 Diataxis docs (Documentation) → T12 review + smoke on the author's Mac

---

<!-- FORGE_PHASE:Refine:Complete -->
## Phase 3: Refine

**Purpose**: Define exactly what "done" looks like.

### Required Outputs
- [x] Acceptance criteria in Given-When-Then format
- [x] Interface specifications documented
- [x] Edge cases enumerated by category
- [x] Constraints vs criteria documented

**CRITICAL**: No code in this phase - specifications only.

### Specifications

- `docs/claude-profile-sync/acceptance-criteria.md`: AC-01…35 (Given/When/Then), constraints at the end
- `docs/claude-profile-sync/interfaces.md`: CLI, exit codes, config.toml, state.json, status --json, CLAUDE.md, claude/git calls
- `docs/claude-profile-sync/edge-cases.md`: EC-01…31 by category

**Awaiting user confirmation before advancing to Generate.**

---

<!-- FORGE_PHASE:Generate:Complete -->
## Phase 4: Generate

**Purpose**: AI writes code following TDD.

### Process
- [x] RED: Write failing tests
- [x] GREEN: Minimal code to pass
- [x] REFACTOR: Improve while green
- [x] Code review: Linter and type checks pass
- [x] Code review: TDD compliance verified
- [x] Code review: Acceptance criteria alignment checked

### Implementation Notes

PRs #1 (publish), #2 (core, T1–T4), #4 (engine and commands, T5–T10), #5 (docs, T11), #7 (fix from the security
review), #6/#3/#8 (v1.0.0 release). 147 tests, 97.7% line coverage, every AC/EC traced to a named test.
**Caveat on "TDD compliance":** tests were written with each module rather than strictly one failing test at a time;
only #7 has a recorded RED → GREEN. See `docs/claude-profile-sync/cycle-review.md` § Issues.

---

<!-- FORGE_PHASE:Evaluate:Active -->
## Phase 5: Evaluate

**Purpose**: Verify output matches intent.

### Checklist
- [x] Criteria verified line-by-line
- [x] Edge cases tested
- [x] Code review: Full test suite passes with coverage threshold met
- [x] Code review: Security review completed
- [x] Code review: Integration and interface contracts verified
- [x] Cycle review summary emitted (docs/<cycle>/cycle-review.md + HTML sibling)
- [ ] Disposition decision made

### Disposition

Proposed: **Accept with issues** (see cycle-review.md). Awaiting the owner's sign-off. No HTML sibling of the
review: the project doesn't render its docs.

---

## Learnings

<!-- Capture learnings during and after the cycle -->
