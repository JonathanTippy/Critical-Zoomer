# Collector / publish bottleneck (2026-08-12)

Status: **diagnosed; not implemented.** Conversation lock for workgroup
throughput under motion and at ≥ default resolution. Does not amend
`workgroup-virtues.md` closures; it names where the live tree pays O(pixels)
while still obeying them. Context closed 2026-08-12 with **no chosen
implementation** and **no product fix**.

## What HUD was showing

Headed `ctrl:` / `pub:` ~15 at `DEFAULT_WINDOW_RES` (854×480), collapsing
toward 0 at 1920×1080.

- `pub:` is collector content-beat publish stamps that reach the window.
- `ctrl:` is controller Replace emission Instants that **ride** a later
  successful collector→shade publish (`pending_controller_emitted_at` → View HUD).

So `ctrl:` is **not** the work-controller loop rate. The controller is not
back-pressured by the collector channel in the sense of a slow Replace sender;
the stamp only advances when the dense publish path delivers. Under load the
two rates couple and both fall.

RateCounters prune events whose emission Instant is older than 1s at paint
time — severe publish latency can drive displayed rates to 0 even when some
puts still succeed.

## Controller audit (stencil-only is real)

Live `WorkerCommand::Replace` carries only `frame_info` + `emitted_at`
(~80 B). Admission is O(1) (`admit_generator` ≪ 0.1 ms). Seat `c` materializes
at `ensure_started` / `get_c`. The full-grid `get_points` path is test/parity
only.

The virtues diagram historically said `Replace(WorkContext)`; the live
contract is stencil-only Replace (`r[cz.craft.stencil-only-replace+2]`). Lazy C
generation is not the excuse for low `ctrl:` — the cost moved off the command
channel.

## Release microbench (2026-08-12)

Wall times on the profile machine (release, center CPUs), synthetic full
packages / shells:

| Path | 854×480 | 1920×1080 |
|---|---|---|
| Controller admit | ~0.0001 ms | ~0.0001 ms |
| Worker `from_stencil` cold (mixmap + shell) | ~79 ms | ~490 ms |
| Worker same-res pan reuse (still `resize_with` placeholders) | ~13 ms | ~76 ms |
| Collector `sample_old_values` | ~24 ms | ~97 ms |
| Collector publish (`clone` + `view_from_package`) | ~9–10 ms | ~151 ms (~6.6 Hz ceiling alone) |

Scale: `Point<f64>` ≈ 344 B → ~141 MB / ~713 MB seat buffers;
`CompletedPoint` package ~30 MB / ~150 MB; densified `View` Answers ~20 MB /
~100 MB.

`CZ_PROFILE_CPU` `collector_ms` only wraps the content-beat publish block — it
**does not** include `absorb_work_update` / remap.

## Three O(pixels) lumps (integrity-preserving targets)

1. **Publish clone + full Answer rebuild** every content beat  
   (`completed_work.clone()` + `view_from_package` map-all).
2. **Dense remap** on every `frame_info` (`sample_old_values`), including
   intermediate stencils when the collector is behind.
3. **Worker shell reinstall** — same-res Replace still
   `points.clear(); resize_with(..., placeholder_point)` even though C is lazy.

Shadergroup fullscreen cost is separate (see `shadergroup_fitness` /
Known issues) and compounds the same HUD collapse.

## Design options discussed (not yet chosen)

### A. Frozen snapshot publish (Arc or pool) — favored first step

Whole-snapshot publish means downstream sees **one complete world**, not
“memcpy the package every beat.”

- Collector keeps a **private** unique dense package (and optionally a private
  dense `Answer` buffer updated on write-seat / remap).
- Content beat publishes an **immutable** snapshot: `Arc<View>` (or Arc of
  answer storage + small header), or a ping-pong buffer move.
- Downstream only reads; never `Arc<Mutex<…>>` shared mutable.

Handback channel / object pool: explicit recycle so steady 60 Hz avoids
allocator churn. **`Arc` avoids a backchannel for lifetime** (last drop frees);
it is not automatically a freelist unless `try_unwrap` feeds a pool.

This does **not** by itself fix remap or shell install.

### B. Tip-only dense remap — not a free win

Worker already drain-to-newest on `Replace` **commands**. Collector
**absorbs all** `WorkUpdate`s and remaps on every `frame_info` (charter /
`collector-absorbs-all`).

When behind, the channel can be
`frame_A, seats_A…, frame_B, seats_B…, frame_C, seats_C…` and the collector
pays full remap for A and B before C — wasted for display.

Skipping to the tip is not “just discard stale stencils”:

- Remap **lineage** is central: intermediate seat writes are carried forward
  by dense `sample_old_values` smear.
- Tip-only without a seat policy either mis-indexes A/B completions into C or
  drops work that the remap chain would have preserved.

So tip-only dense remap needs an explicit orphan policy; it is not an
obvious one-liner without revisiting absorb-all.

### C. Sparse integrator (lineage without full-frame hops)

Targets “getting behind from mixed work + stencils”:

| Layer | Role |
|---|---|
| Worker | one `LiveTarget` / `WorkContext`; seat indices local to live stencil |
| Integrator | sparse hoard; on stencil change, reproject **present** points only with the shared transform |
| Publisher | densify sparse → full `View` on content beat (Arc/pool) |

Integrity constraints if pursued:

- Shade still receives **whole snapshots** only (no delta/`dirty` protocol
  downstream; `dirty`/`clean` tokens remain banned in `src`/`benches`).
- Integrator is results continuity, **not** a second scheduler.
- Pivot order preserved: flush completions for the old stencil, then announce
  the new one; sparse lineage applies on announce.
- One live compute target unchanged.

Cost: a second frame-aware store. Benefit: remap scales with outstanding
results × pivots, not pixels × pivots; densify stays O(pixels) **once per
content beat**, not once per backlog hop.

### D. Same-res shell reuse (worker)

Keep `Vec<Point>` capacity across same-res Replace; reset by generation /
field clear instead of rewriting 344 B × N placeholders; mixmap only on res
change. Still stencil-only Replace and lazy `ensure_started`.

## Standing invariants (do not “fix” by breaking)

From `workgroup-virtues.md`: one live target, one truth for what we know so
far, shared remap transform with the headgroup sampler, small interruptible
bouts, whole-snapshot publishes, fixed pivot order, no competing versions.

Preferred direction from this conversation: **A** for publish cost; **C** if
remap lineage under backlog must stay correct without O(pixels) per hop;
**D** for pan Replace feel. Dense tip-only (**B**) only with a written orphan
seat policy that the developer accepts.
