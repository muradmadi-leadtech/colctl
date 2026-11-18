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

`docs/whole-setup/acceptance-criteria.md` (AC10-01 – AC10-71), `interfaces.md` (CLI behaviour verified 2026-11-18 against Claude Code 2.1.327 and codex-cli 0.169.0 in throwaway folders), `edge-cases.md` (EC10-01 – EC10-30). Confirmed by the owner 2026-11-18, with Generate to run autonomously (PRs auto-merged once `check` passes).

Findings that shaped the spec: Claude expands `${VAR}` in user-scope MCP env and headers; `codex mcp add` can't set `env_vars`, so Codex stdio env references are resolved at sync; Codex 0.169 has no user prompts, so commands become skills in the generated `colctl-setup` plugin (renamed from `colctl-hooks`).

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

T1 #144, T2 #146, T3 #147, T4 #148, T5 #149, T6 #150, T7 #151, T8 (docs, smoke test, review). Each task opened on a
RED commit, then GREEN. Changed from the spec: Codex commands go to `~/.agents/skills` (T5); hooks show as
`source colctl-setup` (T6); `colctl adopt mcp <name>` added in the smoke test. Not done: EC10-07's status note,
EC10-21's not-executable flag, AC10-50's "runs code" mark. Details: `docs/whole-setup/cycle-review.md`.

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
