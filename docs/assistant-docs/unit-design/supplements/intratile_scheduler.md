# Supplement: intratile_scheduler.md

Pairs with authoritative `docs/design/intratile_scheduler.md`. Non-authoritative.

## Phase preference (UD-ITS-1) — inferred

Auth order (highest preference first when choosing work):

1. fill out  
2. edge  
3. scredge  
4. period edge  
5. flood in  
6. in  

Jobs as in auth. Tracing: depth-first boundary tracing.

## Preemption (UD-ITS-2) — D-SCH-3

A higher-preference phase **immediately** suspends a lower-preference mid-job. Suspended job state must be retained so it can resume when it again becomes the preferred work (tenacity; no discard of progress for preference alone).

## Period-unknown in-fill (UD-ITS-3) — inferred

Auth: under unknown period, spread whatever period will be sent — same across the fill so it will not create a false in-filament. `period == 0` remains “unknown” for shade (see period issues in issue-stack); flood-in after resolve uses propagated known period.

## Interaction with tile worker spiral (UD-ITS-4) — inferred

Default tile-worker schedule is spiral-in from outer edge unless this scheduler has provided more specific seats/queues. Scheduler vetoes / redirects; worker still owns iteration mechanics.
