# Edge trace vs flood (RCA note)

**Old** (`workshift.rs` `queue_incomplete_neighbors_of_edge`): after an in/out edge pair, enqueue only the **8 geometrically chosen** seats that continue the contour; `push_front` so tracing stays local.

**Live** (`tile_session.rs` `queue_edge_neighbors`): enqueue **all axis neighbors of both** edge seats (`push_back`). That is a thick near-boundary flood of hard seats, not a 1-pixel boundary walk — dominates Edge-first scheduling and looks like an “extremely slow edge phase.”

Incomplete-point rotate on Edge is secondary; flooding is the primary structural mismatch.
