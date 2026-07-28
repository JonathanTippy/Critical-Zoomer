# New unit: Actor graph

No authoritative unit file. Non-authoritative. Capacities are behavior under Steady State (backpressure = bug).

## Assemblies (UD-ACT-1) — inferred

- **Headgroup:** single actor (auth): UI + sample + shade + stencil out + tile ingest.
- **Workgroup:** tile scheduler, tile worker, intratile scheduler, reference worker, gpu uploader, tile publisher (+ shared tile-manager fn).

Code may still have legacy `workgroup` / `shadergroup` paths — **code-diverge**; target graph is architecture + auth workgroup.

## Channels (UD-ACT-2) — inferred flows

| From → To | Payload | Nominal rate |
|-----------|---------|--------------|
| Headgroup → Workgroup (scheduler) | Stencil | 0–60/s |
| Tile scheduler → Tile worker | Tile address | bursty |
| Tile worker ⇄ Intratile scheduler | Seats / phase jobs / veto | internal |
| Reference worker → Tile worker | Reference orbit | on mag change |
| Tile worker → GPU uploader | CPU tile (if not GPU-native) | 0–1000/s |
| GPU uploader → Tile publisher | GPU tile | 0–1000/s |
| Tile publisher → Headgroup | GPUTile answers | ≤1000/s flat (D-PUB-1) |
| Tile manager (via publisher) → Headgroup | Memory bump | rare |

Bypass: GPU-native worker output skips uploader, still hits publisher.

## Capacities (UD-ACT-3) — assumed

Sized so sustained product rates never block under correct scheduling:

| Channel | Assumed capacity |
|---------|------------------|
| Stencil | 2 (latest-wins coalescing preferred) |
| Tile address | 64 |
| Upload queue | 16 |
| Publish → headgroup | 32 |
| Memory bump | 4 |

Replace if profiling shows structural backup (that is a bug, not a reason to inflate).

## Wake rates (UD-ACT-4) — inferred

- Headgroup: frame-driven ≤60Hz.
- Publisher: event + pace to respect 1000/s ceiling (no min floor).
- Uploader: event or minimum wake to drain.
- Reference: event on mag change.
- Schedulers/worker: steady-state workshift cadence; pause off-screen WIP.

## Shutdown (UD-ACT-5) — assumed

Headgroup close → stop stencil stream → workers finish or cancel off-screen → drain publish → drop GPU resources via gpu context budget rules.
