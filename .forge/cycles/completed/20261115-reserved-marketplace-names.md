# Cycle: reserved marketplace names

**Created**: 2026-11-15T05:37:10.254731+00:00
**Priority**: medium
**Status**: Active
**Mode**: HIL (Human-in-the-Loop)

## Overview

<!-- Describe the change/update this iteration addresses -->

---

<!-- FORGE_PHASE:Refine:Complete -->
## Phase 1: Refine

**Purpose**: Define exactly what "done" looks like for this change.

### Required Outputs
- [x] Acceptance criteria in Given-When-Then format
- [x] Interface changes documented (if any)
- [x] Edge cases enumerated
- [x] Constraints vs criteria documented

**CRITICAL**: No code in this phase - specifications only.

### Specifications

`docs/reserved-marketplace-names/acceptance-criteria.md` (ACH1-01, 02), `edge-cases.md` (ECH1-01). Owner said go ahead on 2026-11-15; Generate autonomous.

<!-- Document specifications here -->

---

<!-- FORGE_PHASE:Generate:Complete -->
## Phase 2: Generate

**Purpose**: Implement the change following TDD.

### Process
- [x] RED: Write failing tests
- [x] GREEN: Minimal code to pass
- [x] REFACTOR: Improve while green
- [x] Code review: Linter and type checks pass
- [x] Code review: TDD compliance verified
- [x] Code review: Acceptance criteria alignment checked

### Implementation Notes

<!-- Document implementation progress here -->

---

<!-- FORGE_PHASE:Evaluate:Active -->
## Phase 3: Evaluate

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

**Accept** (owner, 2026-11-15). v2.5.1, #122; review `docs/reserved-marketplace-names/cycle-review.md`.

---

## Learnings

- Claude reserves some marketplace names for Anthropic's GitHub repos; a local folder can't use them. The fake claude now refuses them too.
- When a repo's own packaging can't be used as-is, colctl's one-plugin wrapper marketplace is the general fallback.
- A hint must name a command that works: suggest `convert` only when Codex can use something in the plugin.
