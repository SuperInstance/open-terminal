# CONFLICTS.md — upstream sync 2026-10-02

Merge of `microsoft/intelligent-terminal` (`upstream/main` @ `0ed68b9bb`) into
`SuperInstance/open-terminal` (`main` @ `bc7ae0c2a`).

- **Strategy:** `git merge` (never rebase). Base `c89f10b`.
- **Delta at start:** 472 upstream commits, 37 fork commits.
- **Conflict surface:** 14 conflicted files (all `modify/modify`; no add/add, no delete/modify).
- **Threshold rule:** if >50 conflicted files → abort. Not triggered (14 ≤ 50).

Resolution rules applied (per sync brief):
- **(a)** files added only by the 37 fork commits → keep ours.
- **(b)** files upstream evolved where we also touched → take upstream's structure, re-apply our additions on top where still meaningful.
- **(c)** upstream natively implements an equivalent ensign feature → prefer upstream, document supersession.
- **(d)** never wholesale-delete a file without logging (no file was deleted).

## Conflict table

| # | File | Rule | Resolution | Semantic risk |
|---|------|------|------------|----------------|
| 1 | `README.md` | b (+keep both) | Took upstream's evolved README (Win10 support, new Get Started/Config sections). Re-applied fork banner + `## SuperInstance Enhancements` section + `### SuperInstance Privacy`. | Low. Both preserved; upstream page is now canonical, fork identity via banner. |
| 2 | `tools/wta/Cargo.lock` | b | Took upstream's lockfile (generated artifact). | Low-Med. Fork's optional deps (nalgebra/simba, chrono, reqwest, tempfile, SuperInstance git deps) absent until next `cargo update`/build. Enabling `math-tools`/`trending` re-resolves. |
| 3 | `tools/wta/Cargo.toml` | b + a | Took upstream's dep graph (agent-client-protocol 1.3, notify, image, uuid, sha2, strsim, time, extra windows-sys features) + re-applied fork optional deps and `[features]`. | Low. Features additive; default build unaffected. |
| 4 | `tools/wta/README.md` | b + c | Took upstream's expanded docs (build/run model, sidebar sessions). Fork's build/alias notes were subsets. | Low. Fork-specific `--features all` build note dropped (see below). |
| 5 | `tools/wta/rust-toolchain.toml` | b + c | Took upstream's pinned `ms-prod-1.93`. Fork's `channel = "stable"` override dropped. | Low. Upstream documents local builds run from repo root, outside this override. |
| 6 | `tools/wta/src/agent_check.rs` | b | Took upstream's `find_exe`/`refresh_path` (spawn_path + claude/codex special-casing) + re-applied fork Windows-only PATH guards. | Low. |
| 7 | `tools/wta/src/app.rs` | b + keep both | Kept upstream's `proposal_channels` field **and** fork's `#[cfg(feature="math-tools")] entropy_tracker` field + initializer. | Low. |
| 8 | `tools/wta/src/main.rs` | b | Took upstream's `main.rs` + re-applied fork feature-module declarations (`agent`, `griot_history`, `module_system`, `trending_harness`, `context_trigger`, `math_analysis`, `forecast`). Fork's Windows-only cfg gates on master/sessions CLI dropped (superseded). | **Medium.** Fork modules may not compile against upstream's changed APIs; CI is a placeholder (see below), so not build-verified. |
| 9 | `tools/wta/src/master/mod.rs` | b + c | Took upstream's full restructure (`MasterConfig`, `PipeSecurity`, 10k+ lines). Fork's 4 × `#[cfg(windows)]` master-mode gates dropped. | **Medium.** Upstream owns master mode natively (Windows-only by design); fork's Linux-master experiment not re-applied. |
| 10 | `tools/wta/src/protocol/acp/client.rs` | b + keep both | Kept upstream's imports (`session_mcp`, `app_contracts`) + fork's cross-platform `PipeStream` type alias (used by the retained connect block). | Low. |
| 11 | `tools/wta/src/protocol/acp/spawn.rs` | c | Took upstream; fork's `#[cfg(windows)]` PATH guard replaced by upstream's `configure_child_environment`. | Low. |
| 12 | `tools/wta/src/runtime_paths.rs` | b + keep both | Kept upstream's `pub(crate)` API; `current_package_family_name` gained `#[cfg(windows)]` + a non-Windows `None` stub; `resolve_root` non-Windows branch retained. | Low. |
| 13 | `tools/wta/src/ui/layout.rs` | b + c | Took upstream's `action_panel::plan` / `panel_layout` redesign. Fork's entropy-bar row wiring dropped from `render()`. | **Medium.** `entropy_bar`/`agent_disagreement` modules exist but are no longer rendered (dead code) pending re-integration against the new layout API. |
| 14 | `tools/wta/src/ui/mod.rs` | b + keep both | Upstream's re-export block + fork's gated `pub mod entropy_bar;` / `pub mod agent_disagreement;`. | Low. |

## Ensign features — survived vs superseded

**Survived (ours-only files, kept intact):**
- Ternary intelligence: `ternary_integration.rs` (`CommandPredictor`, `PatternAnalyzer`, `ConservationMonitor`).
- Math-tools: `math_analysis/*` (verification entropy, hodge error, command markov, spectral dashboard), `forecast/*`, `ui/entropy_bar.rs`, `ui/agent_disagreement.rs`.
- Griot history + skill detector: `griot_history/*`.
- Module system + context triggers: `module_system/*`, `context_trigger/*`.
- Agents-as-apps overlay renderer artifact: `src/renderer/agent_overlay.rs`.
- Docs: `INTEGRATION.md`, `AGENTS_AS_APPS.md`, `docs/FUTURE-INTEGRATION.md`, `AGENT.md`, `memory/JOURNAL.md`, plus `ARCHITECTURE.md`, `THREE_LAYER_ARCHITECTURE.md`, `TRIPARTITE-MAP.md`, `INDUCTION-ANALYSIS.md`, etc.
- CI workflow `.github/workflows/ci.yml` (placeholder: ubuntu-latest, no build).

**Superseded by upstream natives (documented, not re-applied):**
- **Agents-as-apps runtime wiring.** Upstream now has native agent-session/tab/pane integration (`agent_sessions.rs`, `agent_pane_origin.rs`, sidebar session UI, telemetry). Our `agent_overlay.rs` is a standalone prototype renderer (a Rust file in the C++ `src/renderer/` tree, not wired into any build); upstream's native path is the production equivalent. Prototype retained for reference.
- **Entropy-bar layout row** in `ui/layout.rs` — superseded by upstream's panel-layout redesign (see #13).
- **Windows-only `#[cfg]` gates** for master mode / named pipes / registry PATH in `master/mod.rs`, `main.rs`, `spawn.rs` — superseded by upstream's restructure. Partial gates were not re-applied where the surrounding module remains Windows-native; they added no real cross-platform value and risked masking upstream symbols.
- **`rust-toolchain.toml` "stable" override** — superseded by upstream's `ms-prod-1.93` pin (local builds bypass it by design).

**Carried but needs follow-up:**
- `entropy_bar` / `agent_disagreement` are declared and compiled but not rendered until re-wired to the new `panel_layout` API (#13).
- Fork feature modules are declared in `main.rs` but **not build-verified** against upstream's API drift; the fork CI is a no-op stub, so CI green does not imply a successful compile.

## Build-verification caveat

The fork's CI (`.github/workflows/ci.yml`) is a placeholder that runs no build.
No compile was attempted in this sync lane (no GPU/build constraint, and the
crate targets `x86_64-pc-windows-msvc`). A real `cargo check --features math-tools,griot-history,module-system,context-triggers,trending`
is the recommended next step before any further integration.
