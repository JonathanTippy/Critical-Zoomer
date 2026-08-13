# Quality doctrine (binding)

Hard rules for Critical-Zoomer after the post–v0.0.9 quality slip. Violating these
is a fail regardless of feature gain. Cross-check with V2V (S-scale) and
`docs/assistant/testing.md`.

## Never soften a failing truth

**Forbidden** in live code, tests, Tracey rules, or assistant docs:

- `#[ignore]` on a test that guards a product or craftsmanship invariant
- Labeling a failing bar **rejected**, **soft-skip**, **soft floor**, **parked**,
  or **temporarily OK**
- Lowering an assert so a broken path goes green
- `eprintln!("skipped")` + early `return` for a hard Tracey / standards bar
  (GPU absent may skip *GPU-only* probes; CPU baselines must still run)

When a test fails: **fix the code**, then keep the hard assert. If the design
changed deliberately, update the Tracey rule, the virtues/design doc, and the
pinned test **together** in one change (`AGENTS.md` change protocol).

Historical Criterion rows that once said **REJECTED** stay in
`benchmarks.md` only as diagnostic history under an explicit “not a baseline”
section — they are not a license to ship slow paths.

## FIX NOW — Criterion regressions

Any workgroup/headgroup edit that moves an **accepted** Criterion median more
than ~20% worse than the current accepted baseline in `benchmarks.md` is a
**FIX NOW** regression: do not checkpoint as done, do not continue feature
grind, do not rewrite the baseline to hide it. Investigate and restore, or
update the baseline in the **same** commit only when the change intentionally
and correctly alters performance (documented why).

## v0.0.9 naive f64 baseline

Release **v0.0.9** (`e6a0560`) is the golden **CPU naive DirectKernel** feel and
scheduling semantics. Post-revert work lives in the workgroup; it must not
quietly make shallow naive f64 slower or dishonest.

Guards:

- Steady-state DirectKernel home IPS floor (`steady_state_screen_worker_home_ips_cpu_direct`)
- Criterion `time_to_full_frame_direct_oracle` / accepted DirectKernel rows
- Home iteration budget identity where pinned (same counted Mandelbrot work)
- `mode:naive` remains reachable; PPS-selected kernel must not force pert on
  shallow legal views

## Oracle gear (tests only)

`crate::gearbox::oracle` provides an **Oracle** compute gear: absolute
FloatExp (“slidy”) naive iteration for **answer quality checks**. It is never
a production app path. Production gears are compared against it; they do not
replace it.

## Soft-skip history (do not repeat)

DAT 2026-08-01 failed after QC scored B while a GPU TPS bar was soft-skipped
(`docs/assistant/Trash/dat-2026-08-01.md`). Soft-skip looked like hygiene and
was a NASA-V style process failure. **No soft-skip on hard bars.**

## Operational suggestions (from the quality slip)

1. Prefer types over comments for invariants (`BoutCap`, `Delivery`, …).
2. Run `scripts/full_check.sh` (unit then integration then e2e, then Criterion
   + `tracey query validate`) before claiming a unit green — not a hand-picked
   subset. Tracey missing is a **fail**, not a skip.
3. Keep bacon jobs pointing at **live** scripts under `scripts/`; do not leave
   coverage/mutants paths broken.
4. Workgroup-only charter: colorer/headgroup edits beyond location bar + HUD
   require an explicit design note; otherwise revert.
5. Checkpoint on the feature branch; never use ignore/soften to make a
   checkpoint look green.
