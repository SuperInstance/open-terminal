# Merge Strategy: open-terminal → upstream/main

**Date:** 2026-06-04
**Risk Level:** HIGH (26 our commits, 31 upstream commits, likely conflicts in shared files)

## Current State

- **Our branch:** 26 commits ahead of fork point (`c89f10b`)
- **Upstream:** ~31 commits ahead of fork point
- **Fork point:** `c89f10b` — `docs: add Alt+Shift+B shortcut and FAQ page (#185)`

## Our 26 Commits Grouped by Feature Area

### Group 1: Griot History Module (2 commits) — 🟢 CLEAN PICK
- `49aa931` feat(wta): add griot-history module — decaying command history with retelling reinforcement
- `51ce73c` feat: add math-aware command analysis + griot history modules

**Files:** New `griot_history/` module, isolated. Very low conflict risk.

### Group 2: Module System (6 commits) — 🟢 CLEAN PICK
- `9bdd57f` feat: context trigger engine with module lifecycle
- `ef0bce1` feat: add module registry system with TerminalModule trait
- `b26fb09` feat: add context-triggers feature + wire module-system into main
- `4241fd6` feat(module-system): add Module Registry for dynamic module discovery and lifecycle
- `bdb7166` feat: module registry with 56 tests + all 8 builtin modules
- `dc9c259` docs: add ARCHITECTURE.md documenting module system design

**Files:** New `module_system/`, `context_triggers/` dirs. May conflict with upstream changes to `main()` or `Cargo.toml`.

### Group 3: Math Tools & Visualization (4 commits) — 🟢 CLEAN PICK
- `c91fed6` feat(math-tools): add verification entropy status bar
- `62adfb3` feat: add verification entropy bar + agent disagreement visualization
- `1f895b7` feat(math-tools): add agent disagreement visualization with sheaf cohomology
- `00eba07` Fix math_analysis tests: adjust test parameters for correct numerical behavior

**Files:** New `math_tools/` module, isolated.

### Group 4: Skill Detector (2 commits) — 🟢 CLEAN PICK
- `ae41935` feat(skill-detector): renormalization-based skill detection via coarse-graining
- `30e409a` feat: renormalization skill detector - 57 tests, 5 files

**Files:** New `skill_detector/` module, isolated.

### Group 5: Command Forecasting (1 commit) — 🟡 LOW CONFLICT
- `91dca34` feat(forecast): command forecasting system using ergodic theory

**Files:** New `forecast/` module. May touch shared imports.

### Group 6: Bug Fixes (2 commits) — 🔴 HIGH CONFLICT
- `c8edf61` fix: replace byte-index truncate with char-safe UTF-8 truncation
- `40c1e57` fix: replace shift-invert solver with deflated power iteration in spectral analysis

**Files:** These modify existing upstream code. UTF-8 truncation fix likely conflicts with upstream terminal changes. Spectral fix is in our own code but may conflict.

### Group 7: Architecture & Docs (5 commits) — 🟡 LOW CONFLICT
- `daa6118` README: add SuperInstance enhancements section
- `dd1ddbf` chore: remaster with corrected three-layer architecture
- `d527ed9` docs: add Universal Harness Architecture document
- `c3d5c70` Add Harness Architecture section to WTA README
- `1775883` docs: rewrite README as universal harness

**Files:** README.md (major conflict risk — upstream has changed README too), new doc files.

### Group 8: Integration & Dependencies (2 commits) — 🟡 MEDIUM CONFLICT
- `3a6e493` feat: link terminal to metal library fleet as optional dependencies
- `7f40616` Add trending-repo harness module

**Files:** `Cargo.toml` changes (conflict with upstream dep updates).

### Group 9: Induction Analysis (2 commits) — 🟢 CLEAN PICK
- `329c5b6` Add induction analysis and tripartite map for SuperInstance integration
- `7865ea3` Add open-mind tree-sitter induction results: 11,528 functions extracted

**Files:** New analysis files, isolated.

### Group 10: Ternary Agent Integration (1 commit) — 🟡 MEDIUM CONFLICT
- `484f8ed` feat: add ternary agent integration — CommandPredictor, PatternAnalyzer, ConservationMonitor

**Files:** New modules but may touch `Cargo.toml` and integration points.

## Cherry-Pick Plan

### Phase 1: Clean Picks (apply first, should be conflict-free)
```bash
git checkout -b merge-upstream upstream/main
# Group 1: Griot
git cherry-pick 49aa931 51ce73c
# Group 3: Math tools
git cherry-pick c91fed6 62adfb3 1f895b7 00eba07
# Group 4: Skill detector
git cherry-pick ae41935 30e409a
# Group 9: Induction
git cherry-pick 329c5b6 7865ea3
```

### Phase 2: New Modules (may need Cargo.toml merge)
```bash
# Group 2: Module system
git cherry-pick 9bdd57f ef0bce1 b26fb09 4241fd6 bdb7166 dc9c259
# Group 5: Forecasting
git cherry-pick 91dca34
# Group 8: Dependencies
git cherry-pick 3a6e493 7f40616
# Group 10: Ternary
git cherry-pick 484f8ed
```

### Phase 3: High-Conflict Picks (manual merge expected)
```bash
# Group 6: Bug fixes — inspect each carefully
git cherry-pick c8edf61  # UTF-8 truncation — likely conflicts with upstream terminal code
git cherry-pick 40c1e57  # spectral fix — may be in our own code only
# Group 7: Docs — will need README merge
git cherry-pick daa6118 dd1ddbf d527ed9 c3d5c70 1775883
```

## Upstream Conflict Hotspots

These upstream commits touch areas we likely conflict with:
- `815adae` Redesign new tab menu page → dropdown — UI changes, probably safe
- `f389c39` Keep font size delta across settings reloads — settings code
- `d68c1e6` Per-pane /model picker — could conflict with our module wiring
- `0f3067e` Hot-reload acp-model settings — could conflict with module system

## Recommendation

1. **Start with Phase 1** (13 clean picks) — should go smoothly
2. **Phase 2** needs careful Cargo.toml merging after each batch
3. **Phase 3** requires manual conflict resolution — budget 30-60 min
4. **Alternative:** Consider squashing our 26 commits into ~10 feature commits before merging, which makes conflict resolution simpler

## Estimated Effort
- Phase 1: 10 minutes
- Phase 2: 20-30 minutes  
- Phase 3: 30-60 minutes
- **Total: ~1-1.5 hours with testing**
