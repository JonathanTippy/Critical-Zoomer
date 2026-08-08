# Phase 1.5 — implementation notes (assistant)

Authoritative PO quotes / locked answers: `he-said/phase-1.5-period.md`.

## Landed

- `PERIOD_CONFIRMATION_ITERATIONS = 20` in `constants.rs`
- `PeriodicityDetector`: power-of-two checkpoint → one candidate → off-to-side 20 twin confirm → ascending forward-close minimal period (no off-cycle `|λ|` gate)
- Wired through `ActivePoint.derivative` / naive iterate / tile session
- Tests: cardioid / bulb / boundary rings / membership screen / near-parabolic edge / exterior reject
- Out-filament: `get_loop_period` ignores `loop_period == 0` (Dummy)

## Technical notes

- First ε-hit vs a fixed checkpoint can still be a multiple while approaching (lag distance not monotonic); reduce by ascending `|f^d(z)-z|`.
- Do not gate that reduce on `|λ|<1` at the current off-cycle `z` (near-parabolic false reject).
- Interior μ≤0.95 grids alone miss boundary failures — keep near-edge samples.
- Exterior `small_time == 0` is valid; stray small-time edges are zero vs nonzero neighbors — see `bug-stack.md` (true bugs / B-STE-1) / `he-said/scheduler-and-edges.md`.
