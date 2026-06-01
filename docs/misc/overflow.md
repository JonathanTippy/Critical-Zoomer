The app must use the Perturbation algorithm.

The app must use a correct periodicity detection algorithm.

There must be only one answer per view.

Across viewport transforms, work still in frame must still be hoarded:
for moving, this is very simple with tiling and offset mapping.
For zooming in, its optional since the savings would be tiny. 
It suffices to remap the prevous frame and write the new work on top, and accept the very small inefficiency. 
When zooming out, if memory limits allow, the old work must be restored and not redone.


The app must use series approximation.

The app must use boundary tracing.

The app must save its work.

The app must have lookahead.

The app must leverage the GPU

The app must use perturbation and series approximation from the start.

animables:
This must be done by timestamp and speed calculation, not by sending frequent settings updates.

The app must lookahead as the memory limit allows.

The app must use perturbation with screenspace factor for zooming deeper than the f64 limit of ~2^-1024



The historical hoard must be evicted first when memory is tight, and then the lookahead hoard.
However, current and lookahead reference orbits must never be evicted and must contribute to the minimum.

The viewport position management must be able to continue handling well at such a depth; 
viewport position calculations must always be done in less than 1ms.
