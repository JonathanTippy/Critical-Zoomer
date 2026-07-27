# Scheduler boundary trace (PO)

> additional design gap issue to mark down: the scheduler should also follow all filaments and edges which may be rendered, not just the in/out edge. (The scheduler is meant to be boundary tracing so when it finds an in/out edge it should trace it, and also when it finds a period change edge, small time edge, or derivative magnitude angle edge (derivative magnitude angle is the new term to be added to Answers for in filament detection outside the workgroup)) (so out-fill + boundary trace. Originally it would split its time but I found that inefficient. It should do out-fill until it finds a real boundary, then trace all in/out edges, in filament edges, and out filament edges, then finish out-fill, then trace small-time edges.)
> Anyway, I shouldn't have to tell you the boundary. It should additionally trace the in filaments, out filaments

**Phases (intended):**

1. Out-fill until a real boundary is found
2. Trace all: in/out edges, in-filament edges, out-filament edges
3. Finish out-fill
4. Trace small-time edges

Also: period-change edges; future **derivative magnitude angle** on Answers (in-filament detection outside workgroup).

Tracking: `../issue-stack.md` (design gaps).
