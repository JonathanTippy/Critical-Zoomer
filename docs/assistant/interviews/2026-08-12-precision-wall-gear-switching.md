# Interview: Precision wall and gear switching

- **Date:** 2026-08-11 → 2026-08-12
- **Topic:** Precision wall, C-generator admission, gear vs type, transition blockiness, glitch pause

**Summary:** Developer clarified vocabulary and failure modes. A **gear** is a seat-worker / compute kernel, not a numeric type; each gear picks the cheapest type the C-generator admits. Gearbox ranks **fastest expected PPS among gears that admit**. Transition **rectangular blockiness** (when a deeper option exists) is suspected **false shallow admit** from missing ~**10 bits** of admit headroom beyond neighbor distinguishability. That margin belongs in the C-generator admit test on both absolute-`c` and δc paths. **Black** wrong-interior in naive is a separate periodicity-detection tangent. Refs are **never discarded for glitches**; only glitching seats pause until a better ref exists. “No close ref → precision wall” is extremely unlikely in practice (types span ~20 mags); glitch-with-no-safe-ref is the real stuck case. Perturbation admit: choose nearest ref to the screen, then δc stencil, then margin test, then type.

---

### Vocabulary (locked)

| Term | Meaning |
|------|---------|
| **Gear** | Seat worker / compute kernel path (e.g. naive DirectKernel, PerturbationKernel, Naive GPU). HUD may still blur this. |
| **Type** | Numeric representation inside a gear (f32/f64/ScaledF64/FloatExp, …). |
| **C-generator** | Admission gate for the **values actually iterated** (absolute `c` when naive; `delta_c` when perturbed). |
| **Gearbox** | Among gears that admit the current stencil, pick **max expected PPS**; within a gear, smallest type that admits. |

### Failure modes

1. **Rectangular blockiness at transitions** — not enough precision in use. Design: should not happen when a deeper admitting gear/type exists; temporary rescale of old work can look blocky and is unrelated. Suspected bug: **shallow false admit** (missing ~10-bit margin) so PPS race never sees a deeper legal gear.
2. **Gear “kaput”** — distinct from blocks; still open as a second failure shape.
3. **Black wrong-interior** — not a C-gen false pass (that would look blocky). Naive path: likely **false periodicity**; treated as a tangent to the precision/gear work. An earlier periodicity black path was already fixed elsewhere.

### C-generator margin

- Distinguish-neighbors alone is not enough for correct render.
- Need ~**10 extra bits** at shallow depth (polar / Mandelbrot dynamics near |z|≈1); theoretically may approach zero deeper; **leave at 10** is fine.
- Same margin on absolute and δc admit paths.
- Lives **in the C-generator admit test**.

### Perturbation admit order

1. Candidate ref = nearest kept ref to the screen (center/view).
2. Build δc stencil for that ref.
3. Admit with distinguish + 10-bit margin.
4. Smallest type that passes.

Closer ref can make the same stencil admit on a cheaper type — uncommon, must be handled correctly.

### Precision wall vs glitch

- **Precision wall** (no gear+type admits): theoretically possible if no close-enough ref for δc, but **extremely unlikely** — refs are already worked near this mag; types span ≥~20 magnifications. Not a priority worry.
- **Glitch**: may have no non-glitching ref for some seats. Those seats **pause** until a better ref exists. Refs are **liberally / greedily saved** and **must never be discarded because they glitched**. Only glitching points are affected, not whole-gear reject.

### Decision rule (one sentence)

**Among gears that admit (C-gen with margin), lock the highest expected PPS; each admitting gear uses its smallest admitted type.**

Paraphrase: `docs/assistant/paraphrase-authoritative/c-generator-admit-margin.md`.
Design: `docs/assistant/design/gearbox.md`, depth-design CGenerator section, `reference-reuse.md`.
Live stack: `docs/assistant/issue-stack.md`.
