# Interview: Shade GPU residency / data shipping

- **Date:** 2026-08-11 (night)
- **Status:** in progress — interview mode; do not forget parked items
- **Related:** `design/shadergroup-virtues.md` (bailout principal + shipping
  economics), `collected-wisdom.md` (two-stage escape), prior cadence interview
  `2026-08-11-actor-layout-frame-pacing.md`

**Summary (living):** Developer corrects false RCA framing — bailout tail is
**always** ≤~10 iters by Mandelbrot superpower escape, including large animated
radius. GPU escape slowness is **data shipping**, not missing FLOPs. Colorer GPU
wins because CPU color was huge and RGBA readback is thin — not because shipping
is free. Open: preferred residency / actor-linkage design without forcing a
dual-view upload tax everywhere. **No code** until goals settle.

---

## Parking lot

| Item | Status | Where |
|---|---|---|
| False “large radius amortizes GPU escape” claim | **rejected** — documented | virtues + wisdom |
| Bailout ≤~10 iters / superpower escape principal | **locked** | wisdom + virtues |
| GPU escape default stays OG until shipping solved | locked (prior) | escape gear |
| Why colorer GPU helps despite same round-trip pattern | **answered in docs** | virtues § shipping |
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
