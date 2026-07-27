# Period determination phase (PO)

> Period determination isnt correct, it still shows bands at deeper level minibrots. Mark down this new bug (well, return of a thought-fixed bug).
> This isn't a surprise, correct period detection is demanding.
> Add period determination back in. It adds complexity, but unfortunately it is a necessary division.
> During regular iteration, the code should use the simplest check it can without worrying about period, but it must never produce false periodicity. it must be certain. To output false periodicity would be to violate tenacity.
> Once boundary tracing and out fill are complete, the worker should have another phase where it determines the periods of the in-edge.
> Somehow, the out filament rendering must be able to avoid an ugly boundary between in but period unknown and in with known period.

**Clarifies** earlier “`determine_period` antipattern” (`period-small-time.md`): bolting a heavy fix onto every mid-iterate repeat was desperation. A **dedicated period-determination phase** after boundary + out-fill is required.

**Rules:**

1. **Regular iterate:** simplest membership / certainty check only; **never** emit a wrong period (tenacity). Prefer “Inside, period unknown” over a false period.
2. **After** boundary trace + out-fill: phase that determines periods along the **in-edge**.
3. **Out-filament paint:** must not show an ugly seam between period-unknown Inside and period-known Inside.

Tracking: `../issue-stack.md` (B-PER-2, D-PER-1), `../period-and-small-time.md`.
