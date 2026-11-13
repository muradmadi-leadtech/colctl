# Cycle: cli-output-design

**Created**: 2026-11-13T09:23:22.709026+00:00
**Priority**: medium
**Status**: Active

## Overview

Design how colctl presents information and interacts, as one system applied to every command: answers first, calm defaults, live progress, disciplined colour, width-aware layout, errors with fixes, safe interaction, guiding help. `--json` and exit codes unchanged. Cycle 6 of the roadmap in `.forge/context.md`.

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

- PRD: `docs/prd/cli-output-design.md`; system context: `docs/cli-output-design/system-context.md`.
- Owner's decisions (2026-11-13): rich + interactive in a terminal (confirm, pickers, pager; no full-screen views);
  small standard crates allowed; `status` summary first, problems only, `--all` for everything; minor release
  (2.2.0) with human text declared unstable.

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

See `docs/tasks/cli-output-design.md` (T1–T8), `docs/cli-output-design/containers.md`, `components.md`.
Dependency map: T1 → T2, T3, T4, T5 → T6 → T7 → T8.

---

<!-- FORGE_PHASE:Refine:Active -->
## Phase 3: Refine

**Purpose**: Define exactly what "done" looks like.

### Required Outputs
- [ ] Acceptance criteria in Given-When-Then format
- [ ] Interface specifications documented
- [ ] Edge cases enumerated by category
- [ ] Constraints vs criteria documented

**CRITICAL**: No code in this phase - specifications only.

### Specifications

`docs/cli-output-design/interfaces.md` (flags, Caps, tokens, components, per-command layouts, interaction, help), `acceptance-criteria.md` (AC6-01–30), `edge-cases.md` (EC6-01–20). Constraints: `--json` and exit codes unchanged; no full-screen views; nothing asks without a terminal.

<!-- Document specifications here -->

---

<!-- FORGE_PHASE:Generate:Pending -->
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
