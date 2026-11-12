# Cycle: codex-cli-support

**Created**: 2026-11-12T08:54:31.853910+00:00
**Priority**: high
**Status**: Active

## Overview

Cycle 2: Codex CLI support. New harness kinds `codex` (a CODEX_HOME: AGENTS.md link, generated agents/*.toml,
Codex plugins via `codex plugin`) and `agents` (the shared ~/.agents/skills folder). Docs: `docs/prd/codex-cli-support.md`,
`docs/tasks/codex-cli-support.md`, `docs/codex-cli-support/`.

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

Confirmed by the owner on 2026-11-12. Decisions (asked with options): install Codex CLI for probes; link skills into
~/.agents/skills alongside other tools' skills, which stay foreign; a separate Codex plugin list; generate Codex agent
TOML from the .md agents. Probes (codex-cli 0.167.2 via kilo-codex) recorded in .forge/learnings.md.

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

Confirmed by the owner on 2026-11-12. T1 refactor (no behaviour change) · T2 fake codex + test support · T3 config ·
T4 convert · T5 scanning · T6 plan + apply · T7 commands · T8 docs · T9 evaluate + v1.1.0. Strict TDD: RED commit
pushed before GREEN on each task branch. See docs/tasks/codex-cli-support.md.

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

AC2-01…28 (acceptance-criteria.md, constraints at the end), interfaces.md (additions to cycle 1's), EC2-01…28
(edge-cases.md). Task mapping in docs/tasks/codex-cli-support.md. Owner review added "taking over skills other tools link in"
(AC2-24…28, EC2-23…28, `colctl refresh`): twg installs skills in ~/.config/twg/skills and links them into
~/.agents/skills; colctl adopts the target, records the origin, and refreshes on request. **Awaiting owner confirmation.**

---

<!-- FORGE_PHASE:Generate:Active -->
## Phase 4: Generate

**Purpose**: AI writes code following TDD.

### Process
- [ ] RED: Write failing tests
- [ ] GREEN: Minimal code to pass
- [ ] REFACTOR: Improve while green
- [ ] Code review: Linter and type checks pass
- [ ] Code review: TDD compliance verified
- [ ] Code review: Acceptance criteria alignment checked

### Implementation Notes

<!-- Document implementation progress here -->

---

<!-- FORGE_PHASE:Evaluate:Pending -->
## Phase 5: Evaluate

**Purpose**: Verify output matches intent.

### Checklist
- [ ] Criteria verified line-by-line
- [ ] Edge cases tested
- [ ] Code review: Full test suite passes with coverage threshold met
- [ ] Code review: Security review completed
- [ ] Code review: Integration and interface contracts verified
- [ ] Cycle review summary emitted (docs/<cycle>/cycle-review.md + HTML sibling)
- [ ] Disposition decision made

### Disposition

<!-- Accept / Accept with issues / Revise / Reject -->

---

## Learnings

<!-- Capture learnings during and after the cycle -->
