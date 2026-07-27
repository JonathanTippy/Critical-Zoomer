# Phase 1.5 — strong period determination (2026-07-18)

## Quotes

> Add a phase 1.5
> Before orbit management can even begin, we must have strong and correct period determination.
> This is done via a multi-iteration test of not only equality within error epsilon, but also checking the derivative as well, as true twins behave similarly. (twin being z iterate which has the same iteration or equal via periodicity). This is fast  enough as usually the first test will almost always fail allowing continuation. Qestions?

> based on the previous simplified tortious & hare loop detection found in this repo, there will only be one candidate period.
> The candidate period will then be checked by doing the parity test: 20 iterations on the current z and 20 iterations on the saved one, all off to the side.
> Of course, also derivatives, but that doesn't change the shape.

> 1. thats fine
> 2. replace periodicitydetector
> 3. const in constants. 20 should be fine.
> btw, as we go, keep a running record of how often my answers align with your defaults (and whether just saying "defaults" would leave you with the exact same information, or I provided more detail in the act of answering), so that I can make a more informed decision in the future about working through it or just accepting the defaults.

> execute
> btw, one other quality bug, there are bands inside the 'in' areas due to out filament (differing period) highlighting: the periods are wrong, but we knew that much. You should know going forward this is what that looks like. Not much of a bug as fixing period determination will fix this.

## Locked (from answers)

1. Derivative = orbit multiplier along the iterate.
2. Replace workcore `PeriodicityDetector`.
3. Confirmation length = const in `constants.rs`; value **20**.
4. Structure: one candidate (tortoise/hare / checkpoint, every iteration) → off-to-side 20 twin confirm (z + derivative).

Implementation status / tests: `../phase-1.5-notes.md`. Align log: `../answer-vs-defaults.md`.
