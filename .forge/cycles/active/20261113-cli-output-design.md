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

`docs/cli-output-design/interfaces.md` (flags, Caps, tokens, components, per-command layouts, interaction, help), `acceptance-criteria.md` (AC6-01–36), `edge-cases.md` (EC6-01–23; AC6-34–36 and EC6-23 from the Revise). Constraints: `--json` and exit codes unchanged; no full-screen views; nothing asks without a terminal.

<!-- Document specifications here -->

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

T0 #66, #69 (v2.1.1, v2.1.2); T1 #68; T2 #72; T3 #73; T4+T5 #74; T6 #75; T7 #76; smoke fixes #77; v2.2.0 (#71). Revise: T9 + T10 #79 (v2.2.1). RED runs, slips and plan changes in `docs/cli-output-design/cycle-review.md`.

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

**Revise** (owner, 2026-11-13), back to Refine: profile lines should say what each profile has (Claude 17 vs Codex 2 looked wrong), paths in a column were shortened inconsistently, and the help should welcome someone new. AC6-34 – 36, EC6-23.

---

## Learnings

<!-- Capture learnings during and after the cycle -->
