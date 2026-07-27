Apply the phase order and each phase follows the tenacity principle and completes its job.
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

tracing algorithm: apply basic boundary tracing, depth first so paths are explored more quickly rather than slowly growing like a mold.

in-fill under unknown period: spread whatever period will be sent. doesnt matter as long as its the same so it won't cause a false in filament.
