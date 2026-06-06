# Migration Analysis: open-terminal

**Date:** 2026-06-04
**Fork of:** microsoft/terminal (Windows Terminal)
**Scope:** SuperInstance integration — WTA agent pane enhancements

---

## Current State

| Metric | Value |
|--------|-------|
| Behind upstream | 30+ commits |
| Our commits | 24 commits |
| Files changed (ours) | 61 files, +16,639 / -416 lines |
| Upstream WTA files changed | 159 files |
| Shared file conflicts | **62 files** (complete overlap) |

## Our Changes

SuperInstance enhancements to the WTA (Windows Terminal Agent) subsystem:

- **Module system** — dynamic module registry with lifecycle, 8 builtins, 56 tests
- **Griot history** — decaying command history with retelling reinforcement + skill detector (renormalization-based)
- **Forecast engine** — command prediction via ergodic theory, transition matrices, anomaly detection
- **Math analysis** — spectral dashboard, verification entropy, agent disagreement visualization (sheaf cohomology)
- **Context triggers** — module lifecycle integration, autoconfig
- **Trending harness** — trending repo discovery module
- **UI** — entropy bar, agent disagreement overlay
- **Docs** — README rewrite, ARCHITECTURE.md, harness docs, induction analysis

Key files: `tools/wta/src/` (most changes), plus root-level docs.

## Upstream Changes

Major upstream activity overlapping our work:

- Per-pane `/model` picker with local-wins override
- Modal busy overlay during save/install
- Hot-reload for ACP model & delegate settings
- Slash-command refactoring (`/fix` command, Enter handling)
- Agent pane connection state fixes
- Localization batches (resw dedup/collisions)
- Conhost/CAS bypass, font size persistence, sixel OOB fix
- New GitHub Copilot agent skills and instructions

## Conflict Risk: 🔴 HIGH

**62 files shared between our branch and upstream** — nearly every file we modified has also been modified upstream. This is a near-total overlap scenario.

High-risk conflict areas:
- `tools/wta/src/main.rs` — both sides likely restructured entry points
- `tools/wta/src/ui/layout.rs` — we added entropy bar; upstream added modal overlay
- `tools/wta/src/ui/mod.rs` — both added module registrations
- `tools/wta/src/protocol/acp/client.rs` — we touched; upstream has hot-reload changes
- `tools/wta/Cargo.toml` / `Cargo.lock` — dependency additions on both sides
- All doc files (README, ARCHITECTURE.md, etc.) — both sides rewrote

## Recommended Strategy: Fresh Clone + Layered Cherry-Pick

Given the near-total overlap, a standard `git rebase` will be extremely painful with 62 file conflicts across 24 commits.

### Steps

1. **Fresh clone** of upstream at latest main
2. **Identify independent feature groups** in our 24 commits:
   - Group A: Module system (4 commits)
   - Group B: Griot history + skill detector (5 commits)
   - Group C: Forecast engine (3 commits)
   - Group D: Math analysis + UI (5 commits)
   - Group E: Context triggers (3 commits)
   - Group F: Trending harness + docs (4 commits)
3. **Cherry-pick each group** onto fresh clone, resolving conflicts per group
4. **Re-integrate** upstream's new WTA features (hot-reload, /model picker, slash-commands) alongside ours

### Why Not Rebase?
- 24 sequential cherry-picks with 62 file conflicts = ~100+ manual conflict resolutions
- Grouping reduces this to ~6 conflict resolution sessions
- Each group is logically independent, making conflict resolution contextual

## Estimated Effort

| Phase | Time |
|-------|------|
| Fresh clone + setup | 15 min |
| Cherry-pick groups A–F (conflict resolution) | 4–6 hours |
| Integration testing | 2–3 hours |
| Doc reconciliation | 1 hour |
| **Total** | **7–10 hours** |

## Files to Watch

Highest conflict risk (resolve carefully):
- `tools/wta/src/main.rs`
- `tools/wta/src/ui/layout.rs`
- `tools/wta/src/ui/mod.rs`
- `tools/wta/src/protocol/acp/client.rs`
- `tools/wta/Cargo.toml`
- `README.md`
