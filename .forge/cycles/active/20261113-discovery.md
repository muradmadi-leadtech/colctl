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

<!-- FORGE_PHASE:Orchestrate:Complete -->
## Phase 2: Orchestrate

**Purpose**: Break the work into session-sized pieces.

### Required Outputs
- [x] Container architecture (C4 L2) designed
- [x] Component architecture (C4 L3) designed
- [x] Dependency map created
- [x] Tasks sized for single AI sessions

### Tasks

See `docs/tasks/discovery.md` (T1–T9), `docs/discovery/containers.md`, `docs/discovery/components.md`.
Dependency map: T1, T2 independent; T1 → T3–T7; T2 → T3, T4; T3–T7 → T8 → T9.

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

`docs/discovery/acceptance-criteria.md` (AC5-01–28), `interfaces.md`, `edge-cases.md` (EC5-01–21). Constraints: no new crates, discovery read-only and offline except refresh/add/connectors, JSON is a contract for the TUI.

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

T1–T8 merged as #55, #57–#61; smoke fixes #62 (EC5-22–24); v2.1.0 released (#56). RED run per PR and plan changes in `docs/discovery/cycle-review.md`.

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

<!-- Accept / Accept with issues / Revise / Reject -->

---

## Learnings

<!-- Capture learnings during and after the cycle -->
