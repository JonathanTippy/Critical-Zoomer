# Interview: Shade GPU residency / data shipping

- **Date:** 2026-08-11 (night)
- **Status:** in progress — interview mode; do not forget parked items
- **Related:** `design/shadergroup-virtues.md` (bailout principal + shipping
  economics), `collected-wisdom.md` (two-stage escape), prior cadence interview
  `2026-08-11-actor-layout-frame-pacing.md`

**Summary (living):** Developer corrects false RCA framing — bailout tail is
**always** ≤~10 iters by Mandelbrot superpower escape, including large animated
radius. Live `esc:~15` then `esc:~9` / `col:~45–50` with `drop:` is real and
authoritative. Dual-device did **not** fix headed rates. Dummy-head RCA:
convert + GPU pack/readback, not channel full. Within-actor convert hoist +
same-walk prepack once printed dummy-head GPU esc ~60 Hz — **snapshot, not
headed, not a standing dummy-head rate** (pin GPU esc ≥40 on debug+opt-3
`cargo test`; unoptimized debug misses it; `--release` is not the house gate).
**Keep a strong escaper↔colorer actor API**; fusion parked unless unavoidable.
Stencil: O(1) per loop (two `IntExp`s grow at design depth).

---

## Parking lot

| Item | Status | Where |
|---|---|---|
| False “large radius amortizes GPU escape” claim | **rejected** — documented | virtues + wisdom |
| Bailout ≤~10 iters / superpower escape principal | **locked** | wisdom + virtues |
| GPU escape default stays OG until shipping solved | locked (prior) | escape gear |
| Why colorer GPU helps despite same round-trip pattern | **partial** — cost-ratio story is secondary to live rates | virtues § shipping |
| Live HUD `esc:~15` / `col:~50` beats Criterion | **locked** — real app rates authoritative | this interview |
| `drop:4` does not explain esc undercount | **locked** | this interview |
| Why `esc:~15`/`esc:~9` with GPU escape | **RCA (dummy-head)** — convert + GPU pack/readback, not channel full; dual-device did not fix headed | this interview |
| Separate wgpu devices for escape vs color | **applied 2026-08-12** — did not fix headed `esc:` | escaper/colorer gpu |
| Per-actor GPU compartmentalization | **goal** — prove by headed `esc:`/`col:`, not Criterion | this interview |
| Strong escaper↔colorer actor API | **locked** — fusion last resort | virtues + this interview |
| Stencil O(1) per loop, never O(pixels) | **locked** — two `IntExp`s grow at design depth | virtues + wisdom |
| Convert hoist + same-walk `gpu_answer_pack` | **applied** — one dummy-head snapshot ~60 Hz; standing pin is release GPU esc ≥40; headed not re-checked | escaper |
| Dual parallel CPU/GPU views for all actor linkages | **idea only** — developer wary (upload everywhere) | this interview |
| Fuse escape+color on device / keep values resident | **parked** unless within-actor options are exhausted | this interview |
| Workgroup answers already GPU-native → shade without host | undiscussed | naive-gpu / publish |
| Big plan / implementation | **wait** | developer |

---

### Developer

I do not agree on "large animated radius" — this is important to understand and
document. The principal behind having a separate bailout phase is that in almost
all cases, bailout takes literally not more than 10 iterations. This is because
the Mandelbrot function is superpower escaping when it does escape. GPU data
shipping is a real issue. I want you to interview me about my goals and preferred
design, but would prefer to avoid dictating too much which I don't actually know
from experience. One idea is to have two parallel views for all actor linkages,
but that just forces GPU upload problem everywhere. One question is why is the
colorer not slow? why doesnt it have the same issue?

### Assistant

Agreed and documented: separate bailout is justified by **always-short**
continue (superpower escape), not by radius size. GPU escape RCA reframed to
**shipping**. Colorer answer is in `shadergroup-virtues.md` (cost ratio + thin
RGBA out), restated below for the transcript.

**Why the colorer is not “the same failure” (facts):**

- Same *pattern*: upload → compute → host readback.
- Different *economics*: OG color was ~10× OG escape; GPU color still wins after
  paying shipping. Escape GPU tries to replace a ~5–6 ms CPU walk with ~5 ms of
  map/unpack alone → loses or ties.
- Colorer readback is **4 B/pixel** (RGBA). Escape GPU still ships **~32 B/pixel**
  values out (and ~48 B/pixel answers in on new packages).

**Correction (developer challenge 2026-08-11 night):** Criterion walls are
**not** the product truth. Developer reports live HUD **`esc:~15` vs `col:~50`**
with the real app — that stands. Isolated benches cannot overturn headed rates.

**HUD `esc:` semantics (important):**

- Escaper stamps `escape_emitted_at` only on **successful** `try_send` of a
  values screen (design: emission Instant, successful put only).
- Colorer does **not** invent escape emits when it re-paints: it forwards the
  stamp from the latest values screen, then `clear_emission_stamps()` so the
  same Instant is not re-counted. So `col:` can be ~50 while `esc:` is ~15
  without the HUD “lying” about escape — those are two stages’ emit rates.
- Window `esc:` therefore tracks escaper puts **that survive** colorer→window
  coalesce. With empty `drop:` and no silent overwrite before first forward,
  **`esc:` is the escaper’s actual emission rate**. If intermediate frames are
  drain-dropped, `esc:` undercounts true escaper puts (same class of caveat as
  other stage rates observed at the head).

Parking: live `esc:`/`col:` remain authoritative for shade-GPU shipping work;
do not argue from bench-only escape&lt;color walls. Separately confirm HUD
integrity — see developer follow-up on whether `esc:` equals escaper puts.

**Interview questions (goals first; mechanisms only if you already lean):**

1. **Success feel.** When shade is “right” on GPU, what do you want to *notice*
   as a user — bailout anim at vsync with no hitch, lower CPU, headed FPS floor,
   something else? Rank if you can; “I don’t know yet” is fine.

2. **Boundary you care about most.** Is the painful seam primarily
   **workgroup→shade** (answers crossing), **escape→color** (values crossing),
   **color→window** (pixels crossing), or “any full-frame host bounce”?

3. **What must stay true.** Of these, which are non-negotiable vs flexible?
   - workgroup still publishes answers the CPU can reason about / test
   - escaper and colorer remain separate actors / separate gears
   - animated bailout never re-runs the worker
   - colorer parity bar (exact look)
   - HUD / manual OG rollback gears

4. **Dual parallel views.** You floated CPU+GPU linkages and also worried it
   spreads upload. Without picking a design: is the *goal* behind that idea
   closer to (a) “each stage can pick CPU or GPU without blocking the other,”
   (b) “avoid one global device lock / one queue,” (c) “keep an always-working
   CPU reference path,” or (d) something else?

5. **Experience you trust.** Have you ever had a shade/display path that felt
   good on GPU in this project or another — and what made it feel good (no
   readback, fused paint, smaller payloads, lower wake rate)? If none, we stay
   on goals only.

6. **Failure you refuse.** What would count as a bad “fix”? (Examples to
   accept/reject, not a menu: dual buffers that desync, upload on every link,
   forcing naive-GPU workgroup before shade can be fast, thinning colorer
   features, etc.)

Awaiting your answers; parking lot will absorb anything you lock.

---

### Developer

Not true in practice. In the app, the escaper runs ~15fps while the colorer
runs ~50fps. Real app rates are all that matter.

### Assistant

Accepted. Criterion-based “escape GPU still faster than color GPU” is **withdrawn**
as product evidence. Live `esc:~15` / `col:~50` is the bar. Documented above:
those HUD counters are two stages’ emit rates (escape puts vs colorer puts);
colorer can re-emit resident values faster than escape produces. Interview
questions 1–6 still open when you want to continue on goals.

---

### Developer

Its highly important that the HUD values are correct. Is the HUD not displaying
the actual emission rate of the escaper?

### Assistant

**Mostly yes — `esc:` is the escaper emission rate**, not a colorer artifact.

- Stamp is set only on escaper **successful** `try_send` (`escape_emitted_at`).
- Colorer forwards that Instant on the next colored frame and clears it so
  re-paints do **not** re-count the same escape put.
- Window RateCounter records that Instant → `esc:` ≈ escaper puts/sec that
  reach the head.

So `esc:~15` means the escaper is only successfully putting about 15/s (e.g.
GPU escape body blocking the actor), while `col:~50` means the colorer is
successfully putting about 50/s. Both can be correct at once.

**Integrity caveat (not “re-color inflation”):** because counting happens at the
window, drain-to-newest between escaper→colorer or colorer→window can
**undercount** escaper puts that never arrive stamped. Check `drop:` when
auditing. ### Developer

drop is 4

### Assistant

`drop:4` is **cumulative** shade coalesce (`packages_dropped`), not a rate.
Four drops over the session cannot explain `esc:~15` vs `col:~50` — at most a
handful of missing escape stamps total, not a ~35 Hz hole in the 1s window.
So with `drop:4`, treat **`esc:~15` as the real escaper emission rate**; the
coalesce-undercount caveat is idle here. Persistent/`growing` `drop:` would be
the “shade too slow for pixel count” smell (`r[cz.craft.shade-coalesce-drop-count]`).

---

### Developer

Ok, why is the escaper rate 15fps?

### Assistant

**Because GPU escape and GPU color share one exclusive `shade_ops` lock on the
same wgpu device, and colorer already spends most of that budget.**

Mechanism:

1. Escaper stamps/`esc:` only after a successful put; body is
   `escape_with_gear` → GPU `escape_frame` which takes `shade_ops` then
   upload/compute/`map_async` Wait.
2. Colorer (default GPU) does the same lock for every paint (~14–22 ms at
   default res in process).
3. `col:~50` ⇒ colorer alone wants on the order of **50 × ~15–20 ms ≈ 0.75–1.0 s
   of lock time per second**.
4. Leftover lock time for escape ≈ a few hundred ms/s. At ~15–25 ms per GPU
   escape (upload path / contended readback at default res), that leftover is
   about **~15 puts/s** — matching HUD `esc:~15`.
5. Confirmed not the main story: `drop:4`, Criterion-alone walls, or “needs large
   radius.” Pixel scale still hurts (720p upload already ~48 ms isolated), but
   **15 vs 50 at default-sized home is explained by lock multiplexing with the
   colorer**, not by escape arithmetic.

So the escaper is not “mysteriously 15 Hz”; it is **queued behind a ~50 Hz GPU
colorer on one serialized shade device**. OG escape avoids `shade_ops` and can
keep content cadence; GPU escape cannot while colorer owns the same lock.

---

### Developer

Well that seems like a simple fix, just make sure that the device is different,
right?

### Assistant

**Partly — different devices remove the `shade_ops` serialization, but they are
not obviously a complete/simple product fix.**

What separate devices *would* buy:

- Escaper and colorer no longer need one mutex around submit+`map_async`.
- Each can progress on its own wgpu device/queue without waiting on the other’s
  poll Wait.

What they would *not* automatically buy:

- Still two full host round-trips (escape values out, color pixels in/out) —
  the shipping problem remains.
- Same **physical** GPU: driver still multiplexes two clients; PCIe/readback
  can contend even with two devices. Esc might rise above 15 without reaching
  content cadence.
- Duplicate VRAM sessions/buffers; init/teardown complexity.
- Already parked as product risk: **dual wgpu / stops-on-GPU-color**
  (`actor-layout` interview / issue-stack).

So: “different device” is a fair **experiment** to test whether the mutex was
the live limiter, not a settled design. A deeper fix is still residency/fusion
so escape output never returns to the host for color to re-upload. No code until
you say to try the dual-device experiment (or prefer another goal from Q1–6).

---

### Developer

More generally, I think that each actor that uses the GPU should really
compartmentalize its relationship with the GPU so that they're not fighting for
time. What you're describing is all of them using the same GPU resources, which
like, yeah, duh. But I agree that nothing is a fix until we actually see whether
it fixes it.

### Assistant

**Locked as a goal (not an impl):** each GPU-using actor should
**compartmentalize** its GPU relationship so actors are not fighting each other
for GPU time. Shared shade device + `shade_ops` is the opposite of that.
Physical GPU multiplexing (“yeah duh”) remains; the design aim is isolation at
the actor/API boundary. **No claim of fixed until live `esc:`/`col:` (and
related HUD) show it.**

Mechanism left open on purpose — per-actor device/queue/instance are candidates
to try, not prescriptions. Residency/fusion can still matter inside an actor’s
compartment. Experiment when you greenlight; measure first.

---

### Developer

Apply the change

### Assistant

Applied: `GpuEscaper` creates its own wgpu device/queue (`escaper_gpu`); no longer
uses `GpuColorer::shared_device` or cross-actor `shade_ops`. Colorer keeps
`paint_lock` only on its device. Shared `wgpu_init_lock` for adapter init races
under libtest. **Not declared fixed** until you re-check headed `esc:`/`col:`
with escape gear GPU.

Lab probe (release, 854×480, 40 parallel wakes): esc ~95 Hz and color ~48 Hz
**concurrent** on own devices — consistent with removing the 15 Hz lock
multiplex; still confirm in the real app.

---

### Developer

fps:55 pub:8 esc:9 col:45 … escape:GPU — not any better. Escaper 100% CPU at
9fps. Want dummy-head full-graph cadence steady-state.

### Assistant

**Agreed** — Criterion/dual-device probes missed the live pipe. Implemented
`assemblies/pipeline.rs` + `headgroup/dummy_cadence.rs`: real graph, dummy head
records HUD RateCounters. Release: OG cadence green (col~52, esc~22); GPU escape
pin **red** (mean esc~15 ≪ 40) matching the ghost class. See
`steady_state_pipeline_cadence_*` and `testing.md`.

---

### Developer

Good, profile that test and RCA the escaper rate.

### Assistant

**Instrumented** escaper RCA (`enable_escape_rca` / measure-window reset at first
color): wakes, convert ns, escape-body ns, send ok/full/blocked, packages taken.
Dummy-head prints the snapshot next to HUD rates.

**Release profile (unsandboxed, `--test-threads=1`, 854×480, 5 s measure):**

| gear | pub | esc | col | wake/s | body ms/wake | convert ms/pkg | send_full | packages≈send_ok |
|------|-----|-----|-----|--------|--------------|----------------|-----------|------------------|
| GPU  | 16  | 18  | 77  | 17.4   | **20.9**     | **~36**        | **0**     | 87≈88            |
| OG   | 21  | 23  | 80  | 26.6   | **7.7**      | **~34**        | **0**     | 115→133          |

`CZ_PROFILE_CPU` busy probes (body only): colorer ~50% of wall; escaper body
~15–20%; convert is *outside* the busy probe and still ~3.1 s of the 5 s GPU
measure (~62% wall).

#### RCA (escaper rate)

1. **Not channel backpressure.** `send_full=0`, `send_blocked=0`. Stamps equal
   successful sends.
2. **Convert dominates.** MandelbrotResult→CompletedPoint on each package take
   costs ~34–36 ms at live window res (per-pixel `stencil.clone().seat_and_row`).
   Same cost OG and GPU.
3. **GPU body is secondary but real.** Escape body ~21 ms GPU vs ~8 ms OG
   (submit+`poll(Wait)`+unmap path). Combined with convert:
   - GPU ceiling ≈ 1/(36+21) ms ≈ **17 Hz** (matches mean esc 17.8)
   - OG ceiling ≈ 1/(34+8) ms ≈ **24 Hz** (matches mean esc 26.6 with a few
     resident re-emits)
4. **Why esc≈pub, not esc≈col.** Under GPU, nearly every wake takes a new
   workgroup package (`packages_taken≈send_ok`); convert+body leave no budget
   for content-beat resident re-escape. Colorer keeps `col~80` by re-emitting
   resident values without that convert tax. Live `esc:~9` is the same saturation
   class under heavier headed load — dual-device did not remove convert or the
   blocking GPU readback.
5. **Ruled out as primary:** Criterion FLOPs, shared `shade_ops`, stamp
   visibility without send.

**Next levers (ordered after developer preference):** (1) cheapen/hoist convert;
(2) escaper-local GPU shipping (async/non-blocking readback); (3) escaper-local
resident content-beat re-emit; (4) cross-actor fuse/residency **only if
unavoidable**. Measure again with the same RCA print — not Criterion alone.

---

### Developer

Residency is tempting but I prefer to maintain a strong API between the actors
unless we find that it's really unavoidable. So we should start with other
options.

### Assistant

**Locked:** keep a strong escaper↔colorer actor API (values channel / stage
boundary). Cross-actor fuse or end-to-end device residency is **parked** until
within-actor options are exhausted. Lever order matches virtues. Starting with
(1) cheap host convert.

**Convert pass (same session):** hoist seat/row (`results_package_from_answers_view`,
no per-pixel `stencil.clone`). Release cadence re-measure:

| gear | mean esc before → after | convert ms/pkg | body ms/wake | notes |
|------|-------------------------|----------------|--------------|-------|
| GPU  | 17.8 → **32.6** (still <40) | ~36 → **~13** | ~21 → ~18 | body now primary |
| OG   | 26.6 → **65.8** | ~34 → **~11** | ~8 | content-class; send≫packages |

Actor API unchanged. Next within-actor: GPU escape shipping (~18 ms body).
---

### Developer

Very good, and in general, stencils should only be touched N per loop not N per
pixel. …and obv i say that because it has two intexp values, which can get quite
large and will at design depth. proceed with next escaper opt.

### Assistant

**Locked (stencil discipline):** O(1) stencil touch per loop/package — two
`IntExp` coords grow large at design depth (virtues + collected-wisdom).

**GPU shipping (lever 2):** convert fills `gpu_answer_pack` in the same pixel
walk; escape body `write_buffer`s the prepack (no second CompletedPoint→GpuAnswer
walk). Scratch pack retained as fallback. Actor API unchanged.

**Re-measure (release dummy-head, not headed):** GPU mean esc **32.6 → 60.2**;
body **~18 → ~7.1 ms/wake**; send_ok 303 vs packages_taken 150 (resident
re-emit back). OG still ~64 Hz. Dummy-head cadence tests currently pass.
**Not declared fixed** — headed `esc:` with `escape:GPU` was not re-checked.

**Session close:** stencil discipline and strong actor API locked. Fusion
parked. Next within-actor if headed is still short: map/`poll(Wait)` shipping,
not collapsing the actor API.
