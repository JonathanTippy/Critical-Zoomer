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
Window ──PointStencil──► WorkController ──Replace(WorkContext)──► ScreenWorker
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

That is the entire data path from user input to finished membership facts. Four kinds of message cross actor boundaries. One command type. One update type. One output type. The vocabulary is small enough that every actor can be reasoned about alone — and that smallness is not austerity, it is the load-bearing property. Every later failure mode (versioning conflicts, gate disagreements, stream reconciliation) requires more vocabulary than this.

---

## 1. Why three actors, and why these three

The split is exactly the split of *time horizons*:

- **The controller** answers “what screen is wanted *now*?” — a question with a single answer that changes at user speed.
- **The worker** answers “what seat should be iterated *this moment*?” — a question with one answer per bout.
- **The collector** answers “what do we know *so far*?” — a question whose answer accumulates and occasionally transforms wholesale.

Notice what is *not* an actor: scheduling policy. The controller does not tell the worker which seat to do next. It hands over a self-contained `WorkContext` — points, queues, mixmap, edge seeds — and steps out of the way. The worker’s scheduling state is *its own*, disposable, and invisible to everyone else. This is the first deep decision:

> **The schedule is ephemeral; the hoard is authoritative; and they are different objects held by different actors.**

Because the controller never shares mutable scheduling state with the worker, a pivot cannot produce a half-updated schedule. There is no negotiation, no two-phase commit, no “stop doing X, start doing Y” protocol. There is one command — `Replace` — which is the entire new world in a single message. The worker either has the old world or the new world; there is no third state. This is why pivots cannot fail structurally: there is nothing to desynchronize.

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

When a stencil changes, the controller builds the entire next world:

- **`points`** — a full-screen dense `Vec<Point>`, one entry per pixel, each seeded with `c = z = pixel_location`, iteration count 0, no loop checkpoint, smallness 100. The schedule and the working state are literally the same vector; there is no registry of "tasks" separate from "progress". A point's index *is* its seat. This is why work-skipping bugs (README: "fix work skipping") have no place to live: there is no task list that can disagree with the point state; the point state is all there is.
- **`random_map` (mixmap)** — a shuffled permutation of seat indices, regenerated when the resolution changes. Random order is a specific lesson: raster-order traversal of the Mandelbrot set creates visible banding, because neighboring pixels have correlated costs and correlated completion. A shuffled order spreads both easy and hard pixels uniformly across the screen, so partial progress *looks like* uniform refinement rather than crawling stripes. (An interlaced variant exists in the code as an alternative — same intent.)
- **`scredge_poses`** — the *shuffled perimeter* of the screen, computed at context build time. This is the scheduling face of the architecture's "smear/extrude" rule: the seats most likely to be newly exposed by motion are the edges, so the edges are seeded as work from birth, before any completion has occurred anywhere.
- **Four queues** — `scredge_poses`, `edge_queue`, `out_queue`, `in_queue`. All seeded empty except scredge. Queues are `VecDeque`s of `(position, difficulty-or-period)`.
- **`attention`** — the cursor pixel, updated asynchronously, used by the Random slot.
- **`zoomed`** — whether this frame is deeper than the last.
- **counters** — tokens, iterations, bouts, workshifts, percent.

Note the queue entries carry a **cost estimate** (iterations of the neighbor that spawned them, or period for interior floods). The current scheduling order does not sort by it — but it is captured at the source, for free, as a byproduct of having just iterated the neighbor. The design leaves hooks without paying for them.

### Why the context is built by the controller, not the worker

Because context construction is *work the worker should never do*. Building a full screen of seeded points, a mixmap, and an edge list is O(pixels) of pure allocation and arithmetic. If the worker built it, the first shift after every pivot would be consumed by construction — a visible pause at exactly the moment the user most wants responsiveness (README: "fix zoom while drag", "fix work cancellation"). The controller builds the next world *concurrently* with the worker finishing its current shift, and the swap is instant. This is the pipelining that makes pivots feel free: construction and execution overlap, and the handoff is one message.

One honest reservation: the construction is a single monolithic batch on the controller. It works — the controller has nothing else to do — but a small incremental generator could spread that load across the pivot window instead of doing it in one lump, an opportunity to reduce play even further. The *split* (builder vs runner) is the gold; the *batch size* of the build is a refinement left on the table.

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

Each bout picks one seat and calls `iterate_max_n_times(point, r²=4, epsilon, n=1000)`:

- Up to 1000 iterations per bout.
- The point's full state (`z`, cached squares, iteration count, loop checkpoint, smallness) lives in `points[index]` between bouts, so **pause and resume is free** — there is nothing to serialize, no coroutine, no stack to save. The architecture requirement "must be able to pause a particularly difficult point and continue it in the next workshift" is satisfied by the data layout itself. A point *is* its own continuation.
- Cached squares (`real_squared`, `imag_squared`, `real_imag`) make each iteration three multiplies plus the loop/bailout checks — and `update_point_results` also tracks the running minimum of |z|² and when it occurred. Smallness and small-time are collected *for free*, as a side effect of state the iteration already computes. The shadergroup's cosmetics (interior distance-ish shading, period animation) ride on data nobody paid extra to gather.

### Loop detection and period refinement

Inside-set detection uses the classic doubling checkpoint (`update_loop_check_points`: when iterations ≥ 2× checkpoint, save `(z, iterations)`), with a spatial epsilon derived from the actual pixel pitch: `epsilon = |c[0] - c[1]| / 256` — the distance between neighboring pixels, divided by 256. This is a subtle and lovely choice: the loop tolerance scales with the *screen's own resolution in complex space*, so "near enough to be periodic" means "near relative to what the user can see", not an absolute magic number. Zoom changes, epsilon changes, correctness tracks.

When a point repeats, `determine_period` tries to refine the period: `timewarp_n_iterations(..., 100000)` bulk-iterates in unrolled blocks of 4096, then steps forward one iteration at a time with an epsilon eight times tighter, looking for the true period within a 100000-step bound. Honest verdict on this whole stage: it was never that good. The timewarp never yielded great speed, the refinement step's results were never fully trusted either, and the cheap alternative (`period = iterations − loop_checkpoint`) sits commented out directly above the call, nearly as good in visible results. The newer period-determination theory — the one that includes the derivative — is more effective than anything in this file, though no fully working implementation of it has been seen yet. Treat the entire `determine_period` apparatus as a placeholder awaiting that theory; it is a cleanup candidate, not a virtue. (See §12.)

---

## 5. The scheduling core: five-way rotation between frontier queues

This is the heart of the design, and the most imitated-least-understood piece. Each bout picks a `(position, step)` by `workshifts % 5`:

| Shift mod 5 | Priority order | Character |
|---|---|---|
| 0 (first shift only) | scredge → edge → out → in | screen perimeter first |
| 0 (later) | edge → out → scredge → in | boundary & exterior |
| 1 | edge → out → scredge → in | boundary & exterior |
| 2 | **out** → edge → scredge → in | exterior flood favored |
| 3 | edge → out → scredge → in | boundary & exterior |
| 4 | **Random**: attention ± 50 jitter | cursor neighborhood |

Read what this rotation *is*: **timeslicing between scheduling queues**. Each of the five shifts grants the CPU to a different scheduling class, in a fixed round-robin. No class monopolizes; no class starves; the classes interleave at 10ms granularity, so over any 50ms window all five have run.

Why five classes and not a single priority queue? Because each queue encodes a *different theory of what matters most*, and the theories disagree:

- **scredge** says: the newly exposed screen edge matters most (motion continuity).
- **edge** says: the boundary between inside and outside matters most (that's where the image's information content lives — the set's filigree).
- **out** says: flood outward from completed escapes (large exterior regions finish fast; clears cheap space quickly).
- **in** says: flood outward from completed repeats (interior regions likewise).
- **random/attention** says: what the user is looking at matters most.

A single priority order would enshrine one theory and starve the others: edge-only leaves big exterior regions visibly unfinished; out-only wastes time on boring space while the interesting boundary crawls; attention-only turns the rest of the screen into background. The rotation is the compromise — and the specific ordering (edge usually first, out promoted on shift 2, scredge demoted after the initial burst, attention getting exactly one shift in five) is the empirically-tuned truce between the theories. It is a scheduler of schedulers, and it is six lines of `match`.

Notice the first-shift exception: on shift 0 of a brand-new context, scredge leads. A fresh frame (after a move) needs its edges proven first — the architecture's rule that extruded/smear regions be redone first, embodied as "the perimeter is the initial frontier". Once scredge drains, it demotes behind edge and out for the rest of the context's life.

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
- **Scredge**: pushes a **provisional answer** — a `Repeats` with the loop-check delta as period and the running smallness/small-time — into the completion buffer, and moves on. This is the most audacious line in the file: an unfinished screen-edge pixel is published as a *best-effort guess* so the collector's package keeps filling at the motion boundary. The guess is honest about its evidence (period is "how long since my last checkpoint", smallness is real data), it is bounded in impact (edge seats only, and the seat remains undelivered so later shifts still try to finish it — provisional data never blocks true completion), and it eliminates the "blank frontier at the leading edge during pans" failure without inventing a special display state. The architecture's "active temporal dynamic resolution", realized as one `try_push`.
- **Completion buffer full** (`Stec`, 100k entries): the point is **undelivered** (`point.delivered = false`) and the shift breaks. The seat is not lost; it will be re-attempted and re-delivered when the drain has capacity. Backpressure degrades into a re-queue, never into a dropped answer. The *policy* is gold. The *structure* is not: `Stec` is a fixed inline array stack — `[T; 100000]` lives inside the context object — and a bounded stack can overflow, which is exactly the failure this branch exists to absorb. A `Vec` with allocation discipline (reserve once, reuse across contexts) is the better structure: same boundedness policy, no ceiling baked into the type. (See §12.)

### Attention: the user's gaze as a fifth queue

The Random step jitters ±50 pixels around the attention position (clamped to screen), effectively sampling the neighborhood of the cursor. The *idea* is right and cheap: the user is usually looking near the cursor or drag origin; giving that neighborhood one shift in five is a foveation that required no eye-tracking and no special data path — just an `(i32,i32)` on a channel, drained to newest. And if the jitter would fall off-screen, it clamps to the exact attention seat — the center of interest is always reachable.

The *implementation* was an experiment that beat nothing but was not great: the random walk has no memory of delivered seats, so it keeps re-picking points that are already done and burning bouts on the no-op. A delivered-aware sampler (or a small frontier seeded from the attention seat, like the other queues) would keep the virtue without the waste. Keep "gaze is a queue"; replace "random" with something that knows what it has already seen. (See §12.)

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

`work_update` drains via `try_pop` — LIFO. Recent completions are sent first. During a pivot (the moment of maximum motion), the freshest work lands first, and it tends to be the edge/frontier work that matters most for the new frame. Even the drain order is tuned — though note the LIFO order is a free side effect of the `Stec` stack structure, which §12 argues should become a `Vec`; pop-from-end on a `Vec` preserves the same freshness order, so the virtue survives the cleanup.

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

### The remap handles all three motion cases with one code path

- **Move**: relative position in pixels shifts the sampling origin. Overlap preserved, new edge clamped-smeared, worker redoes smear + frontier.
- **Zoom out**: relative zoom positive → each new seat samples an interior point of the old frame → the old frame shrinks into the middle, edges smear. New annulus is scheduled.
- **Zoom in**: relative zoom negative → sampling spreads → old pixels magnify (nearest-neighbor → honest big square pixels, matching "user sees what they saw, magnified"), center detail refills.

One function, three behaviors, no case analysis. The mathematics of the shared transform *is* the motion policy.

---

## 8. The 50ms pulse: how publish cadence emerges instead of being scheduled

Look at the timing architecture from the outside:

- The worker's shift clock is ~10ms.
- After *every* shift, non-empty completions are sent (`total_workshifts % 1 == 0` — every shift, no gating).
- The collector wakes on new data or its own 50ms periodic timer.
- Every seat-write publishes a full View.

The architecture's rule — "send hoarded work on transform, or every 50ms while incomplete; always have new work at that interval" — is satisfied **without a scheduler**: the shift loop naturally produces completions several times per 50ms window under any non-trivial frame, and the natural remap-on-frame-info covers the transform case. The cadence is an emergent property of the shift clock plus per-shift drain. There is no timer to tune, no "publish every N" constant to get wrong, no burst-then-starve behavior. The design found the cadence inside the work rhythm rather than bolting it on.

And the worker's idle path — `percent_completed < 100` keeps it chaining shifts with no sleep; complete means it sleeps on the 50ms/command wait — means the machine is exactly as busy as the screen is unfinished. Load is proportional to ignorance. There is no polling, no spin, no wasted cycles on a finished frame.

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

Details that are easy to miss and were clearly earned:

- **Epsilon from pixel pitch** — loop tolerance tied to visible resolution, not a constant.
- **Cached products in `Point`** — iteration arithmetic minimized; smallness collected as a free side effect.
- **LIFO completion drain** — freshest work publishes first (ordering is the virtue; the stack structure underneath is §12 material).
- **Edge neighbors pushed to queue front** — boundaries jump their own line.
- **Difficulty/period carried in queue entries** — cost metadata captured free at the source.
- **Shuffle-per-resolution mixmap** — anti-banding randomized traversal, rebuilt exactly when it must be.
- **Scredge first only on shift 0** — motion edges proven at frame birth, then demoted.
- **Out rotates, In doesn't** — asymmetric treatment of slow escapes vs slow repeats.
- **Provisional answers never mark delivered** — guesses never block truth.
- **Undeliver-and-break on full buffer** — backpressure degrades to re-queue, never to loss (policy gold; fixed-array structure replaceable, §12).
- **Clamped remap as smear** — motion-fill and storage-remap are one operation.
- **Controller builds, worker runs** — pivot construction overlaps current execution.
- **Small channels** — the machine promises to consume toward the tip.
- **Wall-clock as law** — budget what the user feels (token accounting is vestigial, §12).
- **Publish cadence emergent** — no timer to tune.
- **Load proportional to ignorance** — busy exactly while incomplete.

Each is a line or two. Together they are the difference between a machine that was designed and a machine that was *finished*.

---

## 11. The takeaway, sharpened

v0.0.9's superiority is not any single mechanism — later designs copied the coalescing, the time-boxing, the queues in name. It is that the mechanisms here are **closed over one screen**:

- one live target, so nothing can be behind;
- one package with two mutations, so nothing can be confused;
- one-seat bouts inside 10ms shifts, so nothing can stall;
- whole-package publishes, so nothing can be stale.

Every added capability after v0.0.9 (tiles, mags, batches, orbits, GPU) re-opened one of these closures and then had to re-seal it with gates, carries, versions, and restores — each seal a place for a new bug. The golden design's lesson is not the list of mechanisms; it is that the mechanisms are cheap *because the contract is singular*. Keep "the current truth" a single object, and pivotability, storage sanity, stall-freedom, and freshness are defaults. Distribute it, and they become permanent projects.

---

## 12. What was only better than nothing (the honest 10%)

The closures above are the gold. These six pieces are not — they were experiments that shipped because they beat the alternative of nothing, and each has a known better shape. None of the four guarantees depends on any of them; clean them up without fear, but keep the *need* each one was feeding.

- **Random attention walk.** The idea — the user's gaze gets a scheduling class — is sound foveation for free. The implementation re-picks already-delivered seats because the random walk has no memory, wasting bouts on no-ops. Better shape: a delivered-aware sampler, or seed a small ordinary frontier from the attention seat and let the existing queue dynamics do the work.
- **The `Stec` fixed array stack.** A stack is a fine discipline, but a `[T; 100000]` inline in the context object bakes a ceiling into the type and can overflow — the undeliver-and-break branch exists precisely to absorb that. A `Vec`, with allocations kept in mind (reserve once, reuse across contexts), gives the same boundedness policy and the same pop-from-end freshness order without the hard cap or the inline bulk.
- **The completion staging buffer ("publish queue").** It is a second queue sitting in front of a queue — the `WorkUpdate` channel already stages and bounds messages. Its only distinct contributions are per-shift batching and the LIFO drain order, and both might be had from the channel directly. Possibly redundant; not obviously wrong. If it stays, it should stay *because* batching and freshness-order are demonstrably earning it, not by default.
- **Monolithic WorkContext construction.** Building the next world in one O(pixels) lump on the controller is correct but crude. A small incremental generator could spread construction across the pivot window — one more turn of the play-reduction ratchet. The builder/runner split is gold; the batch size is the unfinished part.
- **Timewarp *and* the period-refinement stage.** Wired up (every repeat completion calls `determine_period`), but the stage was never that good: the timewarp never yielded great speed, the tighter-epsilon re-search never earned full trust, and the one-line `iterations − checkpoint` period sits commented out directly above it, nearly as good in practice. The more effective period-determination theory is the recent one that includes the derivative — but no fully working implementation of it exists yet anywhere in the codebase. Keep "interior points get a period"; replace the whole apparatus when the derivative-based theory lands.
- **Token accounting.** Too hard to get right, and the code already knows it: the token budget in the shift-loop condition is commented out; wall-clock is the only law. The surviving token fields and recomputation are a fossil. Delete them.

The pattern in all six: the *policy* each served (foveation, boundedness, batching, overlapped construction, period precision, budgeting) was right, and the *mechanism* was the first thing that worked. That is exactly what "culmination of manual testing" means — the failures were killed for real, and a few of the weapons were provisional.
