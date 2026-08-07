# Period and small_time (PO)

> determine_period is an antipattern, I added it in desperation to figure out period issues. Mark it down that its an antipattern, and make period and small time just a matter of course.

**Narrow antipattern:** heavy mid-loop `determine_period` on every repeat.

**Superseded in part by** `period-determination-phase.md`: a **dedicated** period-determination phase after boundary + out-fill is required; regular iterate must only make **certain** claims (tenacity).

Tracking: `../period-and-small-time.md`, `../issue-stack.md` (B-PER-2, D-PER-1).
