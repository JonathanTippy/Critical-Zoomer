# E2E-test matrix (assistant-owned, non-authoritative)

Phase gate: every row `green` after a fresh suite run on current tree.
Status: `green` | `in-progress` | `blocked-impl`

| Id | Headed verifies | Status |
|----|-----------------|--------|
| cz.e2e.harness-stack+1 | `scripts/harness_selftest.sh` | green |
| cz.e2e.controls-bindings+1 | `scripts/e2e_controls.sh` | green |
| cz.e2e.controls-no-jump+1 | `scripts/e2e_controls.sh` | green |
| cz.e2e.perf-home-fill+1 | `scripts/e2e_performance.sh` | green |
| cz.e2e.perf-zoom-simple+1 | `scripts/e2e_performance.sh` | green |
| cz.e2e.perf-zoom-hard+1 | `scripts/e2e_performance.sh` | green |
| cz.e2e.visual-oracle+1 | `e2e_oracle` + `scripts/e2e_visual.sh` | green |
| cz.e2e.visual-assistant-review+1 | Read staged PNGs under `/tmp/cz_e2e_visual_review` | green |
| cz.perf.home-10000tps-gpu+1 | headed/release GPU home TPS (cross-link standards) | blocked-impl |
| cz.math.perturbation-naive-oracle+1 | unit ≥3 loci (headed optional) | green |

Orchestrator: `scripts/e2e_suite.sh` (taskset center-half CPUs).

## Evidence (2026-07-31 post-rebase; refreshed 2026-08-01)

- Fresh `scripts/e2e_suite.sh` on current tip: **E2E OK** (harness + oracle + controls + performance + visual).
- Controls / performance / visual pillars: **E2E OK**.
- Home ctl: keyword `home` → `MoveTo` HOME_POSITION UL + `SetZoom` (not SetPos-as-center).
- UI home button aligned to same HOME_POSITION framing.
- Controls zoom/scroll bases use exterior `goto 1.5 0 -1` so center zooms stay structured.
- Simple-zoom gate judges structured frame after 100ms intent (not Xvfb wall clock).
- Assistant review: `vis_home_final.png` shows classic Mandelbrot framing with escape banding; tps 0 after fill is expected; fps ~66.

Phase gate: **green**. Next: QC (V2V ≥ B).

Auth tile_publisher ≥30/s wording still pending (non-blocking; D-PUB-1).
