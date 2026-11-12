# Cycle: quarantine

**Created**: 2026-11-12T17:13:30.366230+00:00
**Priority**: medium
**Status**: Active

## Overview

New versions of source plugins (`update`) and taken-over skills/agents (`refresh`) are held as candidates until the owner reviews (diff + risk flags) and approves them. Cycle 4 of the roadmap in `.forge/context.md`.

---

<!-- FORGE_PHASE:Focus:Active -->
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

<!-- FORGE_PHASE:Orchestrate:Pending -->
## Phase 2: Orchestrate

**Purpose**: Break the work into session-sized pieces.

### Required Outputs
- [ ] Container architecture (C4 L2) designed
- [ ] Component architecture (C4 L3) designed
- [ ] Dependency map created
- [ ] Tasks sized for single AI sessions

### Tasks

<!-- List tasks here -->

---

<!-- FORGE_PHASE:Refine:Pending -->
## Phase 3: Refine

**Purpose**: Define exactly what "done" looks like.

### Required Outputs
- [ ] Acceptance criteria in Given-When-Then format
- [ ] Interface specifications documented
- [ ] Edge cases enumerated by category
- [ ] Constraints vs criteria documented

**CRITICAL**: No code in this phase - specifications only.

### Specifications

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
