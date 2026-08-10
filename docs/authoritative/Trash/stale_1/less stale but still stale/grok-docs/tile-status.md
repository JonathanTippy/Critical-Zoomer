# Tile quality / scheduling / storage — status (assistant)

Quotes: `he-said/tile-quality-and-homothety.md`, `he-said/tile-workcore-boundary.md`.

## Status

- **Boundary tracing:** present in `tile_session`, but queues are **screen-wide** (not one-tile-complete). Not the ideal — see workcore quotes.
- **Black lines:** mitigated via boundary tracing + timed publish + harness mean check; revisit with B-SCH-*.
- **Spiral from cursor:** later.
- **Stack tile:** active `Tile<Answer>` uses `[Option<T>; TILE_SEAT_COUNT]`.
- **Screen-children:** temporary OK for now.
- **Desired:** equal-zoom tiles share one homothety; **at most 8 homotheties** total.
- **Later:** three-path policy — progressive × foveated × lookahead.
- **Ideal workcore:** tiles only; tile scheduler + integrator — **wait for Phase 4**.
