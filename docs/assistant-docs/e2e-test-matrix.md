# E2E-test matrix (assistant-owned, non-authoritative)

Phase gate: every row `green` after a fresh suite run on current tree.
Status: `green` | `in-progress` | `blocked-impl`

| Id | Headed verifies | Status |
|----|-----------------|--------|
| cz.e2e.harness-stack+1 | `scripts/harness_selftest.sh` | green |
| cz.e2e.controls-bindings+1 | `scripts/e2e_controls.sh` | green |
| cz.e2e.controls-no-jump+1 | `scripts/e2e_controls.sh` | green |
| cz.e2e.perf-home-fill+1 | `scripts/e2e_performance.sh` | blocked-impl |
| cz.e2e.perf-zoom-simple+1 | `scripts/e2e_performance.sh` | blocked-impl |
| cz.e2e.perf-zoom-hard+1 | `scripts/e2e_performance.sh` | green when reached |
| cz.e2e.visual-oracle+1 | `e2e_oracle` + `scripts/e2e_visual.sh` | in-progress |
| cz.e2e.visual-assistant-review+1 | Read staged PNGs under `/tmp/cz_e2e_visual_review` | in-progress |

Orchestrator: `scripts/e2e_suite.sh` (taskset center-half CPUs).

## Evidence (2026-07-31)

- Integration: all-green (`docs/assistant-docs/integration-test-matrix.md`); `assembly_tests` 20/20.
- Local workspace tip: home fill fails (~5–6s, gray holes / empty left). Release binary must be rebuilt with `CARGO_TARGET_DIR` unset (sandbox redirect).
- `origin/grok-probation` (`fast`): home fill **pass** (~2.5s, 0 holes). Simple-zoom 100ms gate **fails twice** (~650ms wall; capture not a full answered view). Hard zoom pace OK.
- Workspace is **3 commits behind** origin (`5458b74`, `20231d1`, `7ff5ef5 fast`) while carrying local unit-test WIP on pre-rename `workgroup_new` paths.

## Blocker

Jonathan: how to sync onto origin tip before more e2e/QC (stash+ff, rebase, or discard local WIP)? Loop stopped — do not continue on the stale tip.

Auth tile_publisher ≥30/s wording still pending (non-blocking; D-PUB-1).
