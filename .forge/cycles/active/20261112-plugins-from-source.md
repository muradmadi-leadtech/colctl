# Cycle: plugins-from-source

**Created**: 2026-11-12T11:34:15.308656+00:00
**Priority**: high
**Status**: Active

## Overview

Cycle 3: plugins from source. A source plugin is a repo + pinned commit in config.toml, cloned once into
~/.colctl/plugins, installed from that clone into every Claude/Codex profile the repo packages for; adopt replaces
aggregator installs; `colctl update` moves pins. Docs: docs/prd/plugins-from-source.md,
docs/tasks/plugins-from-source.md, docs/plugins-from-source/.

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

Confirmed by the owner on 2026-11-12. Decisions: source install replaces the aggregator's plugin (marketplace kept);
pinned commit + explicit `colctl update` (`--check` is the only online check; status/sync stay offline, owner asked
about `sync --check` and agreed `update --check` fits better); install where the repo packages, `unsupported`
elsewhere; colctl writes a one-plugin manifest when a repo has none; migrate through adopt, pinning to the installed
commit. Out: quarantine (cycle 4), converting packaging (later). Probes in .forge/learnings.md.

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

Confirmed by the owner on 2026-11-12. T1 config · T2 git+vault · T3 packaging · T4 origin · T5 fakes · T6 plan+apply ·
T7 commands · T8 docs · T9 evaluate + v1.2.0. New components vault, packaging, origin. See docs/tasks/plugins-from-source.md.

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

AC3-01…28, EC3-01…18, interfaces (CLI: add source, rm source, update [--check] [--rev]; [[source]] config; files;
packaging detection; calls; finding a plugin's source; state; status JSON). **Awaiting owner confirmation.**

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
