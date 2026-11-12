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
- [x] Revise: AC3-29, AC3-30, EC3-19 written (list plugins with a known source)

**CRITICAL**: No code in this phase - specifications only.

### Specifications

AC3-01…28, EC3-01…18, interfaces (CLI: add source, rm source, update [--check] [--rev]; [[source]] config; files;
packaging detection; calls; finding a plugin's source; state; status JSON). **Awaiting owner confirmation.**

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

PRs #23 plan, #24 T1, #25 T2, #27 T3, #29 T4 (rebased; #28 closed), #30 T5+T6, #31 T7, #32 T8, #33 fix from the
smoke test, #26 v1.2.0. Strict TDD: RED pushed before GREEN on every PR; T4's RED run failed at format-check rather
than on its tests (see cycle-review.md). 228 tests, 95.7% lines.

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

**Revise** (owner, 2026-11-12). Evaluating on the real profiles showed a gap: superpowers was adopted into
`[plugins]` in cycle 1, so status reports it `ok` (not `from-aggregator`) and `adopt` refuses it ("not a foreign
item"). There was no one-step path from a list plugin to a source plugin. Back to Refine (AC3-29, AC3-30, EC3-19),
then Generate, then Evaluate again; patch release v1.2.1.

---

## Learnings

<!-- Capture learnings during and after the cycle -->
