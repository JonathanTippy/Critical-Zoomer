# New unit: Actor graph

No authoritative unit file. Non-authoritative. Capacities are behavior under Steady State (backpressure = bug).

## Assemblies (UD-ACT-1) — live

- **Headgroup:** single actor `window` (auth): UI + sample + shade + stencil out + tile ingest.
- **Workgroup (SteadyState names in `main.rs`):** tile scheduler, tile worker, intratile scheduler, reference worker, gpu uploader, tile publisher (+ shared tile-manager fn).

Legacy `work controller` / `screen worker` hosts removed. TileSession remains the sync workshift engine inside the tile worker; mutating outfill ops on the live path shuttle through the intratile actor (sync RPC + SS edge).

## Channels (UD-ACT-2) — live flows

| From → To | Payload | Nominal rate |
|-----------|---------|--------------|
| Headgroup → tile scheduler | Stencil (bundle slot 0) | 0–60/s |
| Headgroup → reference worker | Stencil (bundle slot 1; whole screen, never tiles) | 0–60/s |
| Headgroup → tile scheduler | attention | pointer |
| Headgroup → tile publisher | memory Settings | rare |
| Tile scheduler → tile worker | Retarget / SetAttention | coalesced |
| Reference worker → tile worker | ReferenceDelivery (UL-corner orbit; mag-change only) | on mag |
| Tile worker ⇄ intratile scheduler | IntratileRequest / Reply (SS + sync RPC) | workshift |
| Tile worker → GPU uploader | CPU `Tile<CalibratedAnswer>` | 0–100000/s (0 when GPU-native) |
| GPU uploader → Tile publisher | GPU-resident calibrated tile | 0–100000/s (0 when bypassed) |
| Tile worker → Tile publisher | Notify + GPU-resident calibrated handle (bypass) | per bout (cadence-capped) |
| Tile publisher → Headgroup | GPUTile answers | [20, 100000]/s incomplete; 0 complete (D-PUB-1) |
| Tile worker → publisher → Headgroup | Memory bump | rare |

Glitch path (tile worker only): fallback to const zero orbit — no reference rebase. Zero orbit has Z=0 every iteration so the glitch test cannot fire.

### GPU-native path (preferred)

Worker bout → write GPU calibrated → **notify publisher** → publisher shader → headgroup (D-PUB-6). Counter/**TPS** on **move-on done** (D-GPU-1); Phase 2 min-mag/small-time does not re-fire TPS. **Serial** tile default (D-GPU-6). Same-tile dense WIP (D-GPU-8). Boundary-trace / in-fill required (D-GPU-11).

### CPU path

Worker → uploader (CPU calibrated → GPU calibrated) → publisher (same bias shader) → headgroup.

## Capacities (UD-ACT-3) — live starting sizes

| Channel | Live capacity |
|---------|---------------|
| Stencil / attention / scheduler cmds | 64 |
| Upload / publish | 512 |
| Intratile SS | 64 |
| Memory bump / reference delivery | 8 |

Tighten toward assumed Steady State sizes once headed profiling is green.

## Wake rates (UD-ACT-4) — inferred

- Headgroup: frame-driven ≤60Hz.
- Publisher: worker notify after calibrated commits (D-PUB-5) + pace within [20, 100000] Hz while incomplete; idle when complete.
- Uploader: event or minimum wake to drain ([0, 100000]; 0 when bypassed).
- Reference: event on stencil mag change.
- Schedulers/worker: steady-state workshift cadence; pause off-screen WIP.

## Shutdown (UD-ACT-5) — assumed

Headgroup close → stop stencil stream → workers finish or cancel off-screen → drain publish → drop GPU resources via gpu context budget rules.
