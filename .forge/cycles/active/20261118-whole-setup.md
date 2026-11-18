# Cycle: whole-setup

**Created**: 2026-11-18T09:56:14.352156+00:00
**Priority**: medium
**Status**: Active

## Overview

One list of MCP servers, slash commands and hooks in `~/.colctl`, in every Claude and Codex profile: `toolport` declared once with per-profile overrides, secrets as references, commands and hooks delivered through a plugin colctl generates. Roadmap: `.forge/roadmap.md` cycle 10.

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

PRD: `docs/prd/whole-setup.md`. System context: `docs/whole-setup/system-context.md`. Confirmed by the owner 2026-11-18. Owner's decisions: scope is MCP servers + commands + hooks (permissions out); per-profile overrides for values that differ; MCP changes apply directly, not held.

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

Tasks: `docs/tasks/whole-setup.md` (T1–T8). Containers: `docs/whole-setup/containers.md`. Components:
`docs/whole-setup/components.md`. Confirmed by the owner 2026-11-18.

T1 MCP config and state · T2 MCP in the planner · T3 MCP commands · T4 secrets · T5 commands · T6 hooks · T7 TUI · T8 docs, smoke, release.

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
