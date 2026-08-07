# Scheduler, edges, small-time (PO quotes)

> Seems to be a similar bug in small time edges causing bands in that as well
> Also, the worker appears to leave bands of incomplete (black?) work, perhaps due to channel size. Since they are regularly shaped, it seems to be some data or scheduling issue like that.
> Also, the worker appears to skip the screen-edge phase. It is important and was left out of docs but was in the prototype. its necessary so that parts in the set but not connected to other parts (due to current screen position, of course, they are all objectivly connected) can all be caught, because they all must connect at some point to the outer parts, so walking the entire screen edge is in fact necessary as a first step for out-bucket-fill method.
> Also, the worker appears to get stuck and not post any progress when zooming in more to the series of bulbes to the left of the main cardioid. Maybe its just slow, not sure, check it out.

> Not correct, points outside r=2 actually have a small time of 0.
> also, im failing to reproduce the stuck issue so maybe ignore that for now
> however, as soon as the edges have all been walked, the scheduler should send hints to fill in the in areas with a result of inside and a period propogating the period of the edge of the area
> the points should stlil be done as they will have their own min magnitudes, but the easy win should be taken as well.

> its not a discontinuity its just a 0.
> I'm throwing a lot of stuff at you, ensure you maintain your files and a stack of known bugs / todo in your docs as you go.

> also reorzainze and stop putting everything in he-said. Anything that isn't largely quotes just goes in groc-docs.

Tracking / status: `../bug-stack.md`.
