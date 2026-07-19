# Workcore tiles-only / when to cut over (2026-07-18)

## Quotes

> Work no longer seems to be done one tile at a time. What is the design you implemented? I imagined tiles being completed one at a time, but the schduler seems to be working on the whole screen now. it sweeps across.

> The ideal design is that the workcore actually only ever sees tiles. Admittedly, the intra-workgroup design for this was not done.
> The workgroup recieves a stencil and outputs a tile: the tile output is simple enough, it should be done via an integrator which takes single points and emits the whole tile at the workgroup publish rate.
> The input side will need a "tile scheduler" which breaks up the stencil into tiles and tells the workcore what to do based on what the current collection of tiles looks like. This way, the workcore doesn't even have to consider the screen at all.
> Once again, not sure where in the plan this fits. If the shadergroup handles tiles as input, you can go ahead with this. Otherwise, it has to wait.

Status / plan fit: `../tile-status.md`.
