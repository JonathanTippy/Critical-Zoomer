# Scheduler phase order (2026-07-18)

> I don't fucking understand, this is simple. If you apply the phase order and each phase follows the tenacity principle and completed its job, this issue literally cannot occur.
>
> order of phase preference:
> fill out
> edge
> scredge
> period edge
> flood in
> in
>
> jobs:
> fill out: flood fill out points, fed by scredge
> edge: follow edges, fed by scredge
> scredge: complete screen edges
> period edges: follow period edges, fed by edge and scredge
> flood in: instantly bucket fill in-areas of equal period, fed by all edges
> in: complete in points, fed by all edges

Tracking: live `tile_session.rs`.
