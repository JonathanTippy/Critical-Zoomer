# Why the v0.0.9 workgroup stayed golden

Snapshot: commit `e6a0560` (“fix scroll bug”, 2026-06-22).
This document mines the live workgroup at that commit in extreme detail — every actor, every queue, every constant — and explains not just *what* it does but *why it has to be this way*. This design was not dead-reckoned: the README’s own ledger records a year of releases each named after a failure it had to kill — “fix the jank”, “fix work cancellation”, “fix work skipping”, “work saving when zooming in”, “work saving when moving”, “flood fill out points”, “fix work noncomplete”. Most mechanisms below are those scars, generalized — remove one and a named failure returns. But honesty matters more than worship: a few pieces were experiments that merely beat nothing, and §12 names them so nobody mistakes them for pillars.

The four behaviors this design guaranteed:

- **never got behind** a pivot,
- **never confused its work storage**,
- **never stalled**,
- **never showed stale work as current**.

---

## 0. The whole machine, to scale

Three Steady-State actors, each `SoloAct` on its own thread, connected by small channels:

```
Window ──PointStencil──► WorkController ──Replace(stencil)──► ScreenWorker
   │  (cap 50)               (cap 10)                                  │
   └──attention (i32,i32)──────(cap 50)────────────────────────────────┘
                                                    WorkUpdate  (cap 50)
                                                                         ▼
                                                                 WorkCollector
                                                                         │
                                                                   View<Answer>
                                                                         ▼
                                                          Escaper → Colorer → Window
```

Live `Replace` is stencil-only (`frame_info` + emission Instant) — not a seeded
`WorkContext` on the wire (`r[cz.craft.stencil-only-replace+2]`). The worker
builds the shell from the stencil; seat `c` is lazy at first start. The diagram
above used to say `Replace(WorkContext)`; that was the pre-laziness shape.

That is the entire data path from user input to finished membership facts. Four kinds of message cross actor boundaries. One command type. One update type. One output type. The vocabulary is small enough that every actor can be reasoned about alone — and that smallness is not austerity, it is the load-bearing property. Every later failure mode (versioning conflicts, gate disagreements, stream reconciliation) requires more vocabulary than this.

---

## 1. Why three actors, and why these three

The split is exactly the split of *time horizons*:

- **The controller** answers “what screen is wanted *now*?” — a question with a single answer that changes at user speed.
- **The worker** answers “what seat should be iterated *this moment*?” — a question with one answer per bout.
- **The collector** answers “what do we know *so far*?” — a question whose answer accumulates and occasionally transforms wholesale.

Notice what is *not* an actor: scheduling policy. The controller does not tell the worker which seat to do next. It hands over a **stencil** (wanted frame); the worker alone owns the live `WorkContext` — points, queues, mixmap, edge seeds — and steps itself. The worker’s scheduling state is *its own*, disposable, and invisible to everyone else. This is the first deep decision:

> **The schedule is ephemeral; the hoard is authoritative; and they are different objects held by different actors.**

Because the controller never shares mutable scheduling state with the worker, a pivot cannot produce a half-updated schedule. There is no negotiation, no two-phase commit, no “stop doing X, start doing Y” protocol. There is one command — `Replace` — which names the entire new world in a single small message. The worker either has the old world or the new world; there is no third state. This is why pivots cannot fail structurally: there is nothing to desynchronize.

---

## 2. The coalescing discipline: drain-to-newest, everywhere

Every inbound queue in the workgroup is drained the same way:

```
while avail_units(rx) > 1 { drop(try_take(rx)) }   // discard all but newest
let newest = try_take(rx)
```

- The **controller** drains stencils: only the newest `PointStencil` is ever turned into a context.
- The **worker** drains `Replace` commands: only the newest context is ever installed.
- The **worker** drains attention updates: only the newest cursor position biases scheduling.
- The **window** only sends a stencil when its sampling context is marked *updated* — it does not stream historical poses.

Why this has to be this way: **a queue of stencils is a queue of debt.** If the worker processed stencils in order, a fast pan would create a backlog of old frames, each demanding a full screen of work before the next. The system would fall behind by construction and could never catch up — every frame of user motion adds work faster than work retires it. Drain-to-newest makes the backlog physically impossible: intermediate poses are discarded *before* they become obligations. The only compute target that can ever exist is the tip of the user's motion.

This is also why the channels are small (10–50). Small channels are not a resource limit; they are a **promise**: this machine consumes toward the tip. A large channel would be a landfill for stale frames.

The later machine kept the coalescing *function* (`coalesce_scheduler_commands`) but could not keep its *effect*, because the thing being replaced was no longer atomic — a retarget now had to flush lookahead, export lookback columns, carry unsent tiles, and preserve batches, each of which reintroduced the old world into the new one.

---

## 3. The WorkContext: a self-contained job site

When a stencil changes, the worker builds the next world from `frame_info` alone:

- **`points`** — a full-screen dense `Vec<Point>`, one entry per pixel. Seats start `initialized == false`; the first time the scheduler starts a seat, `ensure_started` materializes generator sample into `delta_c` (relative) or `c` (absolute), sets absolute `c`/`z` as appropriate, `dc = (1, 0)` (escape derivative). Vocabulary: absolute Mandelbrot `c`/`z`; reference `reference_c`/`reference_z`; perturbation `delta_c`/`delta_z` (`docs/assistant/tracey/depth-rules.md`). The schedule and the working state are still the same vector; there is no registry of "tasks" separate from "progress". A point's index *is* its seat. This is why work-skipping bugs (README: "fix work skipping") have no place to live: there is no task list that can disagree with the point state; the point state is all there is.
- **`c_generator`** — fail-closed objective→compute converter for this frame (`r[cz.depth.c-generator-fails-closed+1]`): O(1) IntExp probe at admission, stored `T` origin+space, blazing `get_c` multiply-add per seat; admits only when `T` keeps neighbors distinct **with ~10-bit render headroom** (absolute `c` or δc — the values actually iterated); relative anchor is `reference_c` when installed; rebuild on reference generation change. Also source of `pitch_epsilon`.
- **`random_map` (mixmap)** — a shuffled permutation of seat indices, regenerated when the resolution changes. Random order is a specific lesson: raster-order traversal of the Mandelbrot set creates visible banding, because neighboring pixels have correlated costs and correlated completion. A shuffled order spreads both easy and hard pixels uniformly across the screen, so partial progress *looks like* uniform refinement rather than crawling stripes. (An interlaced variant exists in the code as an alternative — same intent.)
- **`scredge_poses`** — the *shuffled perimeter* of the screen, computed at shell install. This is the scheduling face of the architecture's "smear/extrude" rule: the seats most likely to be newly exposed by motion are the edges, so the edges are seeded as work from birth, before any completion has occurred anywhere.
- **Four queues** — `scredge_poses`, `edge_queue`, `out_queue`, `in_queue`. All seeded empty except scredge. Queues are `VecDeque`s of `(position, difficulty-or-period)`.
- **`attention`** — `Option<(i32, i32)>` from the pointer channel; `None` when the mouse is off the fractal screen. Spiral anchor defaults to screen center.
- **`attention_anchor` / `attention_index`** — square-ring spiral state for slot 0.
- **`motion`** — `Zoomed` / `Panned` / `Neither`; decides whether slot 0 leads with attention or scredge.
- **counters** — tokens, iterations, bouts, workshifts, percent.

Note the queue entries carry a **cost estimate** (iterations of the neighbor that spawned them, or period for interior floods). The current scheduling order does not sort by it — but it is captured at the source, for free, as a byproduct of having just iterated the neighbor. The design leaves hooks without paying for them.

### Why the context is built by the worker from the stencil

Because shipping a fully seeded `WorkContext` (~410k points, tens of megabytes) through the
capacity-10 command channel was the pivot's dominant play cost. With a fail-closed
`CGenerator`, per-seat coordinates are trivially recomputable from `(loc, zoom, res)`, so
`Replace` carries only that stencil. The worker installs an O(1)-coordinate shell — generator,
mixmap, shuffled scredge, empty queues — and materializes each seat at first start, the same
way `delivered` guards the publish side. Construction spreads across the frame's natural start
pattern instead of landing as one lump on either actor, and steady-zoom pivots reuse the
previous points buffer so large reallocations are rare.

The virtue that remains is "pivots feel free". The old mechanism was controller-side pipelining
of a monolithic seed; the new mechanism is a near-zero-byte message plus lazy seat init. The
two-message pivot order (flush completions, then announce `frame_info`) is unchanged.

---

## 4. The workshift: time-boxing as the foundation of everything

The worker's loop body:

1. If idle (no context, or complete), wait up to 50ms or for a command.
2. Drain attention (newest) and commands (newest Replace).
3. Run one **workshift**.
4. Drain the completion buffer and, if non-empty, send a `WorkUpdate`.

And the workshift itself:

```
while elapsed() < 10 ms { one bout }
```

Why a clock and not a token budget? The code still carries token accounting (iteration cost 2, bout cost 4, point cost 150, budget 16M) — but the token condition in the shift loop is commented out and the loop condition is *wall time* alone. That is the honest history: token counting proved too hard to get right — estimating costs per iteration across wildly different pixel costs (a hard boundary pixel vs an exterior pixel differ by orders of magnitude) never converged — so the design settled on timeboxing and never looked back. **Time is the only quantity the user actually experiences**, so time is the quantity that is budgeted. The leftover token fields and the `spent_tokens_today` recomputation are dead weight, kept only as a fossil of the lesson; they should be deleted, not revived.

Why ~10ms? Because the worker must return to its loop head often enough that:

- a new `Replace` is observed within ~10ms of arrival (pivot latency),
- the completion buffer is drained into the collector at high frequency (publish freshness),
- a full completion buffer never backs up far (bounded loss).

A 100ms shift would triple the effective pivot latency. A 1ms shift would spend its time on bookkeeping. 10ms is the value that manual testing converged on — short enough to be imperceptible, long enough to amortize.

### The bout: one seat, bounded iterations, resumable

Each bout picks one seat and calls `iterate_max_n_times(point, r²=4, epsilon, BoutCap::STANDARD)`:

- Up to `MAX_BOUT` (1000) iterations per bout. The cap is a type, not a convention: `BoutCap`'s
  only constructor clamps, so an unbounded call literally cannot be expressed. The worker may
  never make an unbounded call — the 10 ms wall-clock check at the top of the bout loop is only
  valid if no call inside the loop can run away.
- The point's full state (`z`, cached squares, iteration count, loop checkpoint, smallness) lives in `points[index]` between bouts, so **pause and resume is free** — there is nothing to serialize, no coroutine, no stack to save. The architecture requirement "must be able to pause a particularly difficult point and continue it in the next workshift" is satisfied by the data layout itself. A point *is* its own continuation.
- Cached squares (`real_squared`, `imag_squared`, `real_imag`) make each iteration three multiplies plus the loop/bailout checks — and `update_point_results` also tracks the running minimum of |z|² and when it occurred. Smallness and small-time are collected *for free*, as a side effect of state the iteration already computes. The shadergroup's cosmetics (interior distance-ish shading, period animation) ride on data nobody paid extra to gather.

### Loop detection and period refinement

Inside-set detection uses the classic doubling checkpoint (`update_loop_check_points`: when iterations ≥ 2× checkpoint, save `(z, iterations)`), with a spatial epsilon derived from the actual pixel pitch: `epsilon = |c[0] - c[1]| / 256` — the distance between neighboring pixels, divided by 256. This is a subtle and lovely choice: the loop tolerance scales with the *screen's own resolution in complex space*, so "near enough to be periodic" means "near relative to what the user can see", not an absolute magic number. Zoom changes, epsilon changes, correctness tracks. (The `/256` factor is still a hand-picked number, though — the non-arbitrary form of this tolerance is a design ideal, §13.)

The restored code originally refined repeats with a 100000-step timewarp and tighter-epsilon
re-search. That provisional stage has now been replaced by the published atom-domain candidate
→ Newton attractor → cycle-multiplier test (`verified_period`): the final period/interiority
answer uses the derivative and exact unit-circle boundary, while pitch epsilon remains only the
cheap provisional loop trigger. See §12's resolved note.

---

## 5. The scheduling core: five-way rotation between frontier queues

This is the heart of the design, and the most imitated-least-understood piece. Each bout picks a `(position, step)` by `workshifts % 5`:

| Shift mod 5 | Priority order | Character |
|---|---|---|
| 0 | **Attention** when Zoomed/Neither; **Scredge** when Panned | foveation vs smear |
| 1 | edge → out → scredge → in | boundary & exterior |
| 2 | **out** → edge → scredge → in | exterior flood favored |
| 3 | edge → out → scredge → in | boundary & exterior |
| 4 | **scredge** → edge → out → in | perimeter |

Read what this rotation *is*: **timeslicing between scheduling queues**. Each of the five shifts grants the CPU to a different scheduling class, in a fixed round-robin. No class monopolizes; no class starves; the classes interleave at 10ms granularity, so over any 50ms window all five have run. Slot 0 is the one exception that adapts: after a zoom (or neither), attention leads for direct navigation; after a pan, scredge leads so the smearing screen border resolves first.

The scheduler is deliberately separated from the numerical kernel it runs
(`r[cz.craft.kernel-seam+1]`). `SeatKernel` owns only three operations:
materialize one seat, run one `BoutCap`-bounded numerical bout, and map a
finished seat to its answer. Production dispatches **`DirectKernel`** (naive) or
**`PerturbationKernel`** (pert) per view (`r[cz.perf.pps-selected-kernel+1]`).
`DirectKernel` is also the parity oracle in tests. The slot rotation, queues, attention
backpressure, and wall-clock law remain outside the kernel. Swapping arithmetic
does not rewrite any of the empirically proven scheduling machinery.

Why five classes and not a single priority queue? Because each queue encodes a *different theory of what matters most*, and the theories disagree:

- **scredge** says: the newly exposed screen edge matters most (motion continuity).
- **edge** says: the boundary between inside and outside matters most (that's where the image's information content lives — the set's filigree).
- **out** says: flood outward from completed escapes (large exterior regions finish fast; clears cheap space quickly).
- **in** says: flood outward from completed repeats (interior regions likewise).
- **attention** says: what the user is looking at matters most.

A single priority order would enshrine one theory and starve the others: edge-only leaves big exterior regions visibly unfinished; out-only wastes time on boring space while the interesting boundary crawls; attention-only turns the rest of the screen into background. The rotation is the compromise — and the specific ordering (attention first after zoom / neither, scredge first after pan, out promoted on shift 2, scredge also owning slot 4, attention getting exactly one shift in five when it leads) is the empirically-tuned truce between the theories. It is a scheduler of schedulers, and it is six lines of `match`.

Notice the first-shift exception on attention fallthrough: when the spiral yields nothing on shift 0 of a brand-new context, scredge leads. A fresh frame (after a move) still needs its edges proven early — the architecture's rule that extruded/smear regions be redone first. Once that first-shift scredge burst drains, scredge lives in slot 4 for the rest of the context's life.

### Queue dynamics: frontiers that generate themselves

The queues are not pre-computed; they are **discovered by completion**:

- When a point escapes, `queue_incomplete_neighbors` pushes its undelivered 4-neighbors into `out_queue`, tagged with the escapee's iteration count.
- When a point repeats, `queue_incomplete_neighbors_in` pushes undelivered 4-neighbors into `in_queue`, tagged with its period.
- When a completed point has a completed neighbor of a *different kind* (or different period), `point_is_edge` fires, and `queue_incomplete_neighbors_of_edge` pushes up to 8 neighbors of the boundary pair into `edge_queue` — with `push_front`, i.e. edge work is inserted at the head of its queue. Edges jump their own line.

This is a flood fill that steers itself toward the interesting parts of the plane. The frontier expands from proven ground; the boundary between regions gets priority within its class; difficulty/period metadata travels with every entry. The scheduler never scans the whole screen — work *announces itself* at the moment the information that justifies it comes into existence. That is why the design has no global re-prioritization pass, no sort, no heap: local events maintain the queues, and the round-robin selects among them.

### The hard-seat rules: how tenacity avoids stalls

When a bout does *not* finish its seat:

- **Out**: pop from front, push to back — rotate. A brutal escape pixel (the neck/butt regions, where iterations run to the bound repeatedly) yields the floor to every other out-queued seat before its next turn. One hard seat can never block the frontier behind it.
- **In**: deliberately *not* rotated (the rotate is commented out) — a slow repeat keeps being probed. The asymmetry is intentional: an unfinished interior point's neighborhood benefits from persistent probing (period detection wants iteration depth), whereas an unfinished escape's neighbors are usually equally slow, so rotation wins there. These opposite treatments of the two queues are the kind of detail that only survives because it was tested by hand against real images.
- **Scredge**: pushes a **provisional answer** — a `Repeats` with the loop-check delta as period and the running smallness/small-time — into the completion staging Vec, and moves on. This is the most audacious line in the file: an unfinished screen-edge pixel is published as a *best-effort guess* so the collector's package keeps filling at the motion boundary. The guess is honest about its evidence (period is "how long since my last checkpoint", smallness is real data), it is bounded in impact (edge seats only, and the seat remains undelivered so later shifts still try to finish it — provisional data never blocks true completion), and it eliminates the "blank frontier at the leading edge during pans" failure without inventing a special display state. The architecture's "active temporal dynamic resolution", realized as one `push_delivery`.
- **Collector channel full**: the worker **waits** (`wait_vacant`) until the
  collector drains — it does not clear `delivered` or reopen Dummy holes.
  Throughput yields to the collector bottleneck. On shutdown interrupt only,
  unsent answers are restaged with `delivered` left true
  (`restage_unsent_batch`). Staging is a growable per-shift `Vec` (fixed-cap
  `Stec` removed 2026-08-11). `r[cz.craft.wait-on-channel-full+1]`

### Attention: the user's gaze as a fifth queue

Slot 0 runs an **attention spiral**: a square-ring walk outward from the live
cursor (`Option<(i32,i32)>` on the attention channel). `Some` restarts the spiral
at that seat; `None` (pointer off the fractal screen) anchors at screen center.
Tenacity is *state*, not call depth: the seat under work is held in
`attention_current`, and each attention bout is bounded by `BoutCap` like every
other phase — the worker may never make an unbounded call. When the held seat
completes (or is found delivered), the hold releases and the spiral advances,
skipping delivered and off-screen seats, so the fovea fills deterministically
instead of re-picking finished seats. When the spiral is exhausted, the slot
falls through to the ordinary queues — with scredge preferred on the context's
first shift. `r[cz.craft.attention-spiral+1]` `r[cz.craft.bout-cap+1]`

---

## 6. The pivot handshake: the two-message ordering

When a `Replace` arrives and an old context exists:

1. `work_update(old_ctx)` — drain the old context's completion buffer.
2. If non-empty: send `WorkUpdate { frame_info: None, completed_points }`.
3. Install the new context.
4. Send `WorkUpdate { frame_info: Some(new_frame), completed_points: vec![] }`.

The **order is the invariant**. Completions from the old frame are flushed *before* the new frame is announced, because seat indices are only meaningful relative to a frame: a completion's `index` addresses the package that matches the frame under which it was computed. The collector therefore processes:

- updates with `frame_info: None` → write seats into the **current** package (the frame both parties agree is current),
- updates with `frame_info: Some` → **remap** the package to the new frame.

Writes never cross a remap. Old seats never land in a remapped package at wrong positions; new announcements never arrive while old seats are still in flight. Two messages and an ordering constraint replace any versioning, tagging, or reconciliation scheme. This is the exact mechanism that makes the storage rules unbreakable: the protocol makes the wrong sequence *unrepresentable*.

`work_update` drains the staging Vec via `pop` — LIFO. Recent completions are sent first. During a pivot (the moment of maximum motion), the freshest work lands first, and it tends to be the edge/frontier work that matters most for the new frame.

---

## 7. The collector: one package, two mutations

The collector's entire state is `completed_work: Option<ResultsPackage>` (plus a dormant `surrounding_work` sketch). A `ResultsPackage` is:

- `results: Vec<CompletedPoint>` — dense, one per seat, `Dummy` for never-written,
- `screen_res`, `location` — the frame it belongs to.

The two mutations:

1. **Write seats** (`frame_info: None`): `results[seat] = completed` for each delivered completion, then publish.
2. **Remap** (`frame_info: Some`): `sample_old_values(old_package, new_location, new_res)` builds a whole new package by sampling the old one through the relative-pixel transform — the *same* `transform_relative_location_i32` / `index_from_relative_location` helpers the headgroup's RGB sampler uses. The architecture demanded "the code used for transforming old work must be the same code used in the Headgroup to sample rgb values" — and it literally is, imported from the same module. Work and color can never disagree about where a pixel went.

The remap is **clamped** at edges (`index_from_relative_location` clamps to bounds): seats whose source fell outside the old frame inherit the nearest edge value. That clamp *is* the smear. The architecture's "extrude the pixels at the edge of the screen" is not a special renderer path — it is the natural behavior of a clamped nearest-neighbor resample, applied to the hoard itself. Elegant because free: the storage remap and the motion-fill are the same operation.

### Why dense, and why whole-package publishes

Every publish is a **complete `View<Answer>`**: the full package, mapped through the `CompletedPoint → Answer` conversion, with the package's true frame in the stencil. Downstream (escaper, colorer, window) never merges deltas, never reconciles versions, never asks "is this tile newer than that tile?" There is one world; here it is, in full.

The cost — re-sending a full screen — is the price of eliminating an entire class of bugs: partial-stream disagreement. The DAT-era failures (NORES floods, WIP holes, missing lookback columns) are all partial-stream diseases. v0.0.9 chose the bandwidth and bought the correctness. And the `Dummy → Inside{period:0}` conversion means unpublished seats render as plain interior — the set's home frame is mostly interior, so an unfinished screen looks like the set with fuzzy exterior detail arriving, which is exactly the architecture's "best-res available" fill order expressed as a data default.

**Implementation note (2026-08-12):** “whole snapshot” is a **contract** (one complete world per put), not a requirement to `clone` the private package every content beat. Frozen `Arc` / ping-pong / handback-pool snapshots are compatible; shared-mutable buffers are not. Live costs and options: `collector-publish-bottleneck.md`.

### The remap handles all three motion cases with one code path

- **Move**: relative position in pixels shifts the sampling origin. Overlap preserved, new edge clamped-smeared, worker redoes smear + frontier.
- **Zoom out**: relative zoom positive → each new seat samples an interior point of the old frame → the old frame shrinks into the middle, edges smear. New annulus is scheduled.
- **Zoom in**: relative zoom negative → sampling spreads → old pixels magnify (nearest-neighbor → honest big square pixels, matching "user sees what they saw, magnified"), center detail refills.

One function, three behaviors, no case analysis. The mathematics of the shared transform *is* the motion policy.

---

## 8. Workshift density + content-beat publish

Look at the timing architecture from the outside:

- The worker's shift clock is ~10ms.
- After *every* shift, non-empty completions (or iteration deltas) are sent as
  `WorkUpdate`s (`total_workshifts % 1 == 0` — every shift, no gating).
- The collector swaps those into a single resident package and **publishes to
  shade on the content beat** (`Settings::resolved_content_period()`, Automatic
  = egui/OS vsync Hz). Escaper and colorer share that period.
- Every collector publish is still a full `View` snapshot of work-so-far.

Worker → collector cadence remains emergent from the shift clock. Collector →
shade is intentionally timer-paced so incomplete large frames still refresh at
vsync even when shifts are sparse. Head present may use vsync or
uncapped-to-max-FPS independently.

And the worker's idle path — undelivered seats keep it chaining shifts
with no sleep; once every seat is delivered it sleeps on the 50ms/command wait
and skips `workshift` — means the
machine is exactly as busy as the screen is unfinished. Load is proportional to
ignorance.

---

## 9. Why each virtue follows from the structure, not from vigilance

### Never gets behind — because backlog is unrepresentable
Drain-to-newest on every input + single-context worker + controller-side context construction. Old frames cannot accumulate as obligations; the schedule is replaced atomically; the next world is built concurrently. Getting behind would require the machine to remember intermediate frames, and nothing in the machine remembers intermediate frames.

### Never confuses storage — because there is one store with two mutations
One dense package; write-seat and remap-whole; pivot ordering makes write-after-remap-into-wrong-frame unrepresentable; remap uses the display's own transform code. Confusion requires competing versions or crossed mutations; both are structurally absent.

### Never stalls — because the unit of work is one seat and the unit of control is 10ms
Wall-clock shifts, bounded bouts, resumable points, hard-seat rotation, provisional edge answers, completion-buffer backpressure-as-requeue, and five-way queue timeslicing. A stall requires something that can own the actor: there is nothing bigger than a bout to own it, and nothing that can starve a queue for more than four shifts.

### Never regresses the picture — because publishes are whole snapshots of the single truth
Every output is the complete current package after the latest update, stamped with the true frame. Stale work as current requires a second stream or a delta protocol; neither exists.

---

## 10. The craftsmanship inventory

Details that are easy to miss and were clearly earned (each bound to its code site by a Tracey rule in `docs/assistant/tracey/craftsmanship-rules.md`):

- **Epsilon from pixel pitch** — loop tolerance tied to visible resolution, not a constant. The `/256` factor is still arbitrary, though; the non-arbitrary form is a design ideal (§13). `r[cz.craft.epsilon-pixel-pitch+1]`
- **Cached products in `Point`** — iteration arithmetic minimized; smallness collected as a free side effect. `r[cz.craft.cached-products+1]`
- **LIFO completion drain** — freshest work publishes first (ordering is the virtue; the stack structure underneath is §12 material). `r[cz.craft.lifo-drain+1]`
- **Edge neighbors pushed to queue front** — boundaries jump their own line. `r[cz.craft.edge-push-front+1]`
- **Difficulty/period carried in queue entries** — cost metadata captured free at the source. `r[cz.craft.cost-metadata+1]`
- **Shuffle-per-resolution mixmap** — anti-banding randomized traversal, rebuilt exactly when it must be. `r[cz.craft.mixmap-shuffle+1]`
- **Scredge first on shift-0 fallthrough** — motion edges proven at frame birth when attention yields nothing. `r[cz.craft.scredge-first-shift0+1]`
- **Attention spiral first** — foveated square-ring walk owns slot 0; tenacity held in `attention_current`, bouts capped. `r[cz.craft.attention-spiral+1]`
- **Pan/zoom slot 0** — attention leads on Zoomed/Neither; scredge leads on the first shift of a pan. `r[cz.craft.pan-zoom-slot0+1]`
- **Bout cap** — no unbounded call; every bout bounded by `BoutCap`/`MAX_BOUT` (type-enforced). `r[cz.craft.bout-cap+1]`
- **Screen-space derivative edges** — `is_in_filament` extrapolates the escape field; flat and ±1 raw neighborhoods stay dark. `r[cz.craft.screen-space-derivative-edges+1]`
- **Period derivative test** — verified periods via atom-domain candidate → Newton → multiplier. `r[cz.craft.period-derivative-test+1]`
- **Out rotates, In doesn't** — asymmetric treatment of slow escapes vs slow repeats. `r[cz.craft.out-rotates-in-stays+1]`
- **Provisional answers never mark delivered** — guesses never block truth (type-enforced: only `Delivery::Final` may set `delivered`, via `push_delivery`). `r[cz.craft.provisional-not-delivered+1]`
- **Wait-on-channel-full** — when the collector is behind, the worker calms down
  and waits; `delivered` stays set (no Dummy reopen). Type-enforced staging via
  `push_delivery`; restage only on shutdown interrupt. `r[cz.craft.wait-on-channel-full+1]`
- **Clamped remap as smear** — motion-fill and storage-remap are one operation. `r[cz.craft.clamped-remap-smear+1]`
- **Stencil-only Replace, lazy seat init** — no seeded context crosses the channel; seats materialize at first start; the single live target is structural (`LiveTarget` pairs context + `frame_info`). `r[cz.craft.stencil-only-replace+2]`
- **Small channels** — the machine promises to consume toward the tip. `r[cz.craft.small-channels+1]`
- **Wall-clock as law** — budget what the user feels (token accounting is vestigial, §12). `r[cz.craft.wall-clock-law+1]`
- **Publish cadence emergent** — no timer to tune. `r[cz.craft.emergent-cadence+1]`
- **Load proportional to ignorance** — busy exactly while incomplete. `r[cz.craft.load-proportional-ignorance+1]`

Two protocol-level disciplines are likewise bound: drain-to-newest coalescing (`r[cz.craft.drain-to-newest+1]`, §2) and the pivot two-message ordering (`r[cz.craft.pivot-two-message-order+1]`, §6). The shared remap transform (§7) carries `r[cz.craft.shared-remap-transform+1]`.

Each is a line or two. Together they are the difference between a machine that was designed and a machine that was *finished*.

---

## 11. The takeaway, sharpened

v0.0.9's superiority is not any single mechanism — later designs copied the coalescing, the time-boxing, the queues in name. It is that the mechanisms here are **closed over one screen**:

- one live target, so nothing can be behind;
- one package with two mutations, so nothing can be confused;
- one-seat bouts inside 10ms shifts, so nothing can stall;
- whole-package publishes, so nothing can be stale.

Every added capability after v0.0.9 (tiles, mags, batches, orbits, GPU) re-opened one of these closures and then had to re-seal it with gates, carries, versions, and restores — each seal a place for a new bug. The golden design's lesson is not the list of mechanisms; it is that the mechanisms are cheap *because the contract is singular*. Keep "the current truth" a single object, and pivotability, storage sanity, stall-freedom, and freshness are defaults. Distribute it, and they become permanent projects.

### GPU port note (forward-looking; does not amend the inventory)

The Naive GPU design (`naive-gpu-design.md`) keeps these closures intact: one live
view, whole-truth publishes, pivot order, provisional ≠ final, and
`BoutCap`/wall-clock interruptibility. Parallelism widens the **bout** (many seats
per bounded wave) behind the kernel seam; it does not multiply live targets or
replace the host scheduling queues with a second authority. FLOP→IPS tracking is a
performance bar on that kernel, not a license to reopen the tile-era seals.
Skipping flood-fill neighbor discovery on GPU finals (or mopping holes with a
late CPU `DirectKernel` phase) reopens the “work skipping / noncomplete” class
of failures; that is not an allowed GPU optimization
(`r[cz.craft.gpu-host-queue-discovery+1]`).

---

## 12. What was only better than nothing (the honest 10%)

The closures above are the gold. These five remaining pieces are not — they were experiments that shipped because they beat the alternative of nothing, and each has a known better shape. None of the four guarantees depends on any of them; clean them up without fear, but keep the *need* each one was feeding.

- **Attention spiral.** The user's gaze gets a scheduling class that walks outward from the
  cursor (or screen center when the pointer is off-screen), skipping finished seats, and holds
  the current seat in `attention_current` until it completes.
  `r[cz.craft.attention-spiral+1]`
- **~~The `Stec` fixed array stack~~ (resolved 2026-08-11).** Deleted. Completions stage in a growable per-shift `Vec` drained LIFO into the collector channel; channel-full undelivers the batch.
- **The completion staging Vec ("publish queue").** Still a second queue in front of the `WorkUpdate` channel. Its remaining jobs are per-shift batching and LIFO drain; if those stop earning their keep, drain straight into the channel.
- **Monolithic WorkContext construction.** Controller-side seeding is **done** (stencil-only Replace + lazy `ensure_started`). The remaining O(pixels) lump is **worker shell install**: same-res pan still `resize_with` placeholder `Point`s (~13 ms @ 854×480, ~76 ms @ 1080p in the 2026-08-12 probe). Amortize or generation-invalidate without bringing a seeded context back onto the channel. See `collector-publish-bottleneck.md`.
- **Token accounting.** Too hard to get right, and the code already knows it: the token budget in the shift-loop condition is commented out; wall-clock is the only law. The surviving token fields and recomputation are a fossil. Delete them.

The pattern in all five: the *policy* each served (foveation, boundedness, batching, overlapped construction, budgeting) was right, and the *mechanism* was the first thing that worked. That is exactly what "culmination of manual testing" means — the failures were killed for real, and a few of the weapons were provisional.

### Resolved since restoration

- **Period refinement.** The timewarp and tighter-epsilon re-search were replaced by the
  atom-domain → Newton → derivative pipeline (`period_partials` + `verified_period_from`):
  candidates are record-minimum steps tried ascending (the last-record-only form verified
  multiples of the true period), Newton starts from the orbit's tail iterate (the published
  `F^p(0,c)` start is unreliable near parabolic necks), and a converged root is reduced to its
  minimal period before the exact `|b| ≤ 1` multiplier test. Unverifiable repeats publish
  period 0 (unknown), which never lights a period edge. Oracles: main cardioid = period 1,
  period-2 bulb = period 2, and the neck at −0.75 classifies correctly at ±2^-k for k up to 40.
  Full-frame time: 12.30 s (timewarp) → about 293 ms, same 10,302,563 counted iterations.

---

## 13. Design ideals

Ideals, not defects: the preferred path toward elegance — solutions that are not arbitrary, that just work because they are the correct numbers. They may or may not be practical; they are recorded so that future work aims at them rather than at another tuned constant.

- **Non-arbitrary tolerances.** The pitch-derived epsilon (`|c[0] − c[1]| / 256`) beats a constant because it tracks visible resolution — but the `/256` factor is still a hand-picked number, which just moves the arbitrariness from "what epsilon" to "what fraction of a pixel". The ideal is a loop tolerance that falls out of the correct numbers with no free parameter at all. Until such a form exists, the pitch-derived version stays.

