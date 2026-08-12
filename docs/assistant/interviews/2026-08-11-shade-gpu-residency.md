# Interview: Shade GPU residency / data shipping

- **Date:** 2026-08-11 (night)
- **Status:** in progress — interview mode; do not forget parked items
- **Related:** `design/shadergroup-virtues.md` (bailout principal + shipping
  economics), `collected-wisdom.md` (two-stage escape), prior cadence interview
  `2026-08-11-actor-layout-frame-pacing.md`

**Summary (living):** Developer corrects false RCA framing — bailout tail is
**always** ≤~10 iters by Mandelbrot superpower escape, including large animated
radius. GPU escape slowness is **data shipping** plus, in-app with GPU color,
**shared `shade_ops` / shared shade device**. Live `esc:~15`/`col:~50` with
`drop:4` is real. **Goal:** each GPU-using actor compartmentalizes its GPU
relationship so actors do not fight for GPU time; prove any change with live
rates. Dual-device is one experiment, not a declared fix. **No code** until
greenlit / goals settle.

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
| Why `esc:~15` with GPU escape | **RCA** — `shade_ops` lock shared with GPU color | this interview |
| Separate wgpu devices for escape vs color | **applied 2026-08-12** — measure live `esc:`/`col:` | escaper/colorer gpu |
| Per-actor GPU compartmentalization | **goal + first step** — escaper≠colorer device; prove by measure | this interview |
| Dual parallel CPU/GPU views for all actor linkages | **idea only** — developer wary (upload everywhere) | this interview |
| Fuse escape+color on device / keep values resident | undiscussed | — |
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
