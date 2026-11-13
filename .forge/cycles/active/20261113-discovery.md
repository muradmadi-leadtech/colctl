# Cycle: discovery

**Created**: 2026-11-13T07:59:06.388484+00:00
**Priority**: medium
**Status**: Active

## Overview

Find plugins across every catalog Claude and Codex already keep on this machine (search, show, by developer, marketplaces, connectors), offline and as JSON for the cycle 6 TUI, and add one from source in one step. Cycle 5 of the roadmap in `.forge/context.md`.

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

- PRD: `docs/prd/discovery.md`; system context: `docs/discovery/system-context.md`.
- Owner's decisions (2026-11-13): local catalogs only; Codex connectors in a separate command; results show the command
  and `add source <id>` works in one step; developer = repo owner, author name as fallback.

---

<!-- FORGE_PHASE:Orchestrate:Active -->
## Phase 2: Orchestrate

**Purpose**: Break the work into session-sized pieces.

### Required Outputs
- [ ] Container architecture (C4 L2) designed
- [ ] Component architecture (C4 L3) designed
- [ ] Dependency map created
- [ ] Tasks sized for single AI sessions

### Tasks

See `docs/tasks/discovery.md` (T1–T9), `docs/discovery/containers.md`, `docs/discovery/components.md`.
Dependency map: T1, T2 independent; T1 → T3–T7; T2 → T3, T4; T3–T7 → T8 → T9.

---

<!-- FORGE_PHASE:Refine:Pending -->
## Phase 3: Refine

**Purpose**: Define exactly what "done" looks like.

### Required Outputs
- [x] Acceptance criteria in Given-When-Then format
- [x] Interface specifications documented
- [x] Edge cases enumerated by category
- [x] Constraints vs criteria documented

**CRITICAL**: No code in this phase - specifications only.

### Specifications

`docs/discovery/acceptance-criteria.md` (AC5-01–28), `interfaces.md`, `edge-cases.md` (EC5-01–21). Constraints: no new crates, discovery read-only and offline except refresh/add/connectors, JSON is a contract for the TUI.

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
