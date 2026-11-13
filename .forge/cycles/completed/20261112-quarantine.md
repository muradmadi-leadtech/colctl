# Cycle: quarantine

**Created**: 2026-11-12T17:13:30.366230+00:00
**Priority**: medium
**Status**: Active

## Overview

New versions of source plugins (`update`) and taken-over skills/agents (`refresh`) are held as candidates until the owner reviews (diff + risk flags) and approves them. Cycle 4 of the roadmap in `.forge/context.md`.

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

- PRD: `docs/prd/quarantine.md`; system context: `docs/quarantine/system-context.md`.
- Owner's decisions (2026-11-12): hold updates and refreshes, not first-time adds; `update` stages and `approve`
  installs, `reject` remembers; `review` shows diff + risk flags; list plugins are flagged `unreviewed` only.

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

See `docs/tasks/quarantine.md` (T1–T10), `docs/quarantine/containers.md`, `docs/quarantine/components.md`.
Dependency map: T1 → T3, T4; T2 → T5; T3 + T4 → T5–T8; T5–T8 → T9 → T10.

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

`docs/quarantine/acceptance-criteria.md` (AC4-01–33), `interfaces.md`, `edge-cases.md` (EC4-01–23). Constraints vs criteria: the Constraints section of interfaces.md (no new crates, sync offline, first-time adds unchanged, JSON shapes are a contract for the TUI).

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

T1–T9 merged as #41, #42, #44–#50 (RED run per PR in `docs/quarantine/cycle-review.md`); #51 typos fix; v2.0.0 released (#43).
Plan changes during Generate (AC4-29 in_sync, AC4-31 moved to T6, EC4-20 exit 3, `warning` JSON field) are listed in the cycle review.

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
- [x] Disposition decision made

### Disposition

**Accept** (owner, 2026-11-13). Follow-ups: ETXTBSY flake in apply tests; flag noise and column alignment in `review` (cycle 6 or a small HIL cycle).

---

## Learnings

<!-- Capture learnings during and after the cycle -->
