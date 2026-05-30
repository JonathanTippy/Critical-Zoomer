# Critical-Zoomer Specification


NO EDITS TO THIS FILE BY AI ASSISTANTS WITHOUT EXPRESS APPROVAL

**Version:** 0.2.0 (`critical_zoomer`)  
**Status:** product definition (required state).  


**Operational principles**
This project should follow that NASA V methodology, using tracey to link design docs with tests.

---

## Document scope


This specification states the **required product**:
features, invariants, target architecture, and **approved design trade-offs**.


This file does **not** describe delivery status, debug tooling, test
methodology, crate layout, or actor-graph wiring.

Plain language throughout. Standard Mandelbrot symbols (c, z, R = 2) where
helpful.

**Traceability**

- Tracing Must follow the NASA V pattern: each component's design spec must link to the unit tests for that component, and for larger compositions, the design for that group of parts links to the tests which test it together. Thus, the spec here should link only to the end to end tests. As Egui isn't easily testable, the window must be thin/dumb and send its commands to the sampler actor which sends only rgb frames with additional metadata. This way, the entire graph is kept testable.

---

## 1. Overview and requirements

### 1.0 Problem statement


Critical-Zoomer must be **complete** and **fast** at the same time.

| Requirement | Meaning |
| ---- | ------- |
| **Complete** | Every pixel is truly done (escape or proven period) or clearly still in progress |
| **Fast** | No perceived wait on input; deep zoom stays practical |

That pairing drives the architecture: instant preview, retained work,
honest incomplete pixels, and **always-on** perturbation (§4.3) instead of
brute-force per-pixel iteration.

### 1.1 North star


**Mission:** Browse the Mandelbrot set **very deeply**, as fast as the user
cares to go.

**Methodology:** Use **steady_state actor pipelines** — an assembly-line
layout where early results show up fast and are **replaced by better ones**
as downstream actors catch up. No stage is allowed to fall so far behind
that it **chugs** or blocks input; the graph stays responsive end-to-end.

**Interaction contract:**

- Each scroll wheel motion is **one scroll bump**
  (`zoom_pot` ± 1 → **2×** magnification per bump).
- A typical gesture may produce **~10 scroll bumps** → **1024×** cumulative
  zoom.
- The application MUST:
  1. **Apply every scroll bump immediately** on the input path (viewport,
     preview) — no waiting for compute to finish before the next scroll
     bump.
  2. **Sustain rapid cumulative zoom** without cost-per-pixel collapse at
     any magnification.
  3. **Refine quality asynchronously** after each scroll bump without
     blocking further input.
- Zoom and pan are **mathematically exact** at the viewport (§4.2).
  Responsiveness does not trade away pointer anchoring or precision at any
  zoom level.

**Input-first:** Full per-pixel iteration at the viewport resolution is
incompatible with this requirement. **Perturbation** and **series approximation**
MUST run **always** (§4.3) — no “turn on perturbation” wall.

### 1.2 Compute / coloration separation


**Compute layer (retained):**  
The screen worker produces **`CompletedPoint`** results. The pipeline
**retains** them across settings changes.

**Derivation layer (downstream):**  
The **highlighter**, **escaper**, and **colorer** build what the user sees
from those retained **`CompletedPoint`**s — not by recomputing the worker
when only display or settings change.

When settings use **animables**, derived display (bailout, color) MUST run
at **60 Hz at 1080p** while compute refines in the background.

The **sampler** MUST refresh the on-screen preview from the last delivered
picture **immediately** on input while compute catches up (§3.2).

### 1.3 Lookahead prefetch


When the view is idle or low priority, the pipeline MUST prefetch ahead of
the user.

**Pan:** prefetch a ~1× neighborhood from pan velocity.

**Focus zoom:** prefetch a coin-sized patch from **eye gaze**.

Prefetch MUST cancel on view change.

Prefetch MUST stay within the **Memory Limit** (§1.5). Viewport work wins.

Under pressure, evict **retained off-screen work** before evicting
lookahead.

### 1.4 Two-stage escape and animated bailout


**Screen worker:**  
Iterate until **R > 2** (‖z‖² > 4). Publish `CompletedPoint::Escapes` or
`CompletedPoint::Repeats` per §1.6.

**Escaper:**  
Continue each escaped (c, z) **out to completion** at animatable bailout
radius **R_b**. Publish `ScreenValue::Outside` for outside pixels.

**Sanctioned exception (§2.1):** On the main antenna, the escaper MAY hit
an iteration limit and publish `ScreenValue::Outside` with imperfect
metrics — not full completion at **R_b**.

Changing **R_b** in settings MUST rerun the escaper only — not the worker.

The escaper MUST sustain **60 Hz at 1080p** even in the worst case (every
pixel at maximum escaper iterations, including the main antenna).

### 1.5 Memory budget


The user sets a **Memory Limit** slider (`limit_mb`). Default **512 MB**;
maximum **1024 MB**.

**Minimum `limit_mb` depends only on screen size.** Recompute the slider
floor when:

- the app starts,
- the window is resized,
- the user changes **Memory Limit**.

Bump the floor so `limit_mb` cannot sit below what the current resolution
requires for on-screen viewport work.

On-screen viewport work is **never evicted** — the floor safeguards it.

The user MAY set `limit_mb` above the floor up to the maximum.

If the required minimum exceeds **1024 MB**, cap at 1 GB and enforce a
**screen-size bumper**: the window MUST NOT resize larger than fits within
the maximum.

Under pressure, evict **retained off-screen work**, then **lookahead** (§1.3).

### 1.6 Completion law


**Banned:** fake “done” via iteration caps; **effort limits**;
`CompletedPoint::Dummy` as final.

**Worker (CPU or GPU workshift):** Same rules. Escape at **R > 2** →
`CompletedPoint::Escapes`. Interior: **derivative-based period detection**
→ `CompletedPoint::Repeats`. GPU runs in **shifts** like the CPU worker —
no separate completion scheme.

**Escaper:** `ScreenValue::Outside` after bailout to **R_b** (§2.1 exception
on main antenna).

**Incomplete (unusual):** The screen is usually filled by **remap** of the
previous frame (§3.2). Explicit incomplete pixels (e.g. `ScreenValue::Idk`)
MUST appear only when lookahead and retained work are insufficient, or
during **first-frame** fill-in before any frame exists to remap.

### 1.7 Smallness and small time


Each pixel tracks **smallness** (smallest **magnitude of z** seen) and
**small_time** (iteration index at that minimum).

These live on `CompletedPoint` and carry into derived `ScreenValue` and
coloring (e.g. `PaintSmallTime`, `PaintSmallness`).

**Smallness gradient:** Follow smallness as a gradient to locate **minis**.
As smallness approaches **zero**, the point indicates a **minibrot**. The
highlighter MUST support **mini highlighting** — a primary product feature.

**Small-time edges:** Highlight small-time edges to show tree-like structure
in bulbs and cardioids.

**CPU-type workers** (one point at a time) MUST prioritize small-time edges
when scheduling compute. When edges enclose a
**petal** with one shared orbit period, fill the petal with that period
instead of iterating every interior pixel.

### 1.8 Filaments and highlighting


All of this is produced by the **highlighter** before the escaper (§1.2).
Marks are **set invariants** — they MUST NOT depend on animatable bailout
**R_b**. The colorer paints them; it does not detect them.

**In-filament:** Highlight pixels whose points lie **inside** the set.
Derived from neighbor escape-time (**big_time**) slope **sign changes**
(peaks). Bias toward a **thin line** (product choice).

**Out-filament:** Highlight pixels whose points lie **outside** the set.
Derived from **period changes** among interior neighbors. Bias **outward**
(toward higher period) (product choice).

**Node highlighting:** Mark **minibrots** too small to see (§1.7). Mark
**nodes** of bulbs and cardioids — the most stable point in that structure,
already settled, with no **fool's period**.

**Fool's period:** A point shows a temporary period that later settles;
settling can be arbitrarily slow. A black region shares one period, but
points nearer its edge take longer to reach the **node** (stable orbit).

Default coloring script includes these highlight layers.

### 1.9 Product summary


**Name:** Critical-Zoomer

**Primary requirement:** Deep Mandelbrot exploration at user speed.

**Platforms:** Linux (X11 + Wayland), Windows, macOS, web.

**One codebase, all platforms.**

*(Crate structure: [DELIVERY_SPEC.md](DELIVERY_SPEC.md) §1.)*

---

## 2. Approved design trade-offs


Deliberate compromises the product **accepts**. Not missing features —
those belong in [DELIVERY_SPEC.md](DELIVERY_SPEC.md).

### 2.1 Main antenna deep zoom (escaper)


**Region:** Main antenna — near the **R > 2** escape boundary, including
approach toward **(−2, 0)**.

**Accepted:** The escaper MAY use a **maximum iteration limit** and publish
`ScreenValue::Outside` with **imperfect** bailout metrics. Full extension
to **R_b** is not required in this region.

Checkerboard or hard incompleteness would be stricter; **speed and responsive
escaper animation** take priority. This trade-off MAY be revisited.

**Not accepted:** Blocking scroll or sampler snappy control; skipping worker
escape at **R > 2**; fake finished worker states; using this cap outside the
main antenna.

---

## 3. User-facing behavior


**Main window:** **800×480** at the default home view; separate settings
surface; user-resizable within memory rules (§1.5).

### 3.1 Interaction controls


**Pan:** Primary (left) drag; viewport position MUST use the §4.2 model.

r[viewport.retained_drag_start]  
On primary-button down, store `objective_drag_start` and
`screenspace_drag_start`. **Not releasing** is required: while the button
stays down, the user MAY zoom out, pan, and zoom back in. Zooming back MUST
be able to return to the stored **drag-start** objective location.

**Zoom:** Scroll wheel — one **scroll bump** per step (`zoom_pot` ± 1 → **2×**).

**Zoom anchor:** Pointer at bump time; `center_screenspace_pos` is the render
pixel under the cursor (internal origin §3.3).

r[viewport.pointer_anchored_zoom]  
The Mandelbrot point at `center_screenspace_pos` **before** a scroll bump
MUST map to that **same** pixel **after** the bump, with **cell-center**
half-pixel correction on each `zoom_pot` step.

**Snappy controls:** The preview path updates on every scroll bump and drag
without waiting for merged compute (§1.2, §3.2).

**Incomplete display:** Unfinished pixels use a **64×64** checkerboard
(**#FF00FF** / **#000000**). Before the first full paint, the window shows
solid purple (**128, 0, 128**).

### 3.2 Never stale, never final


Frames are always moving through the pipeline. Nothing is “stale” and
nothing is treated as a **final** frame to discard.

The **preview path** MUST satisfy **instant feedback** at **60 Hz or better
at 1080p**. Merged compute plays catch-up at its own speed
without blocking scroll bumps or drags.

~10 scroll bumps in one gesture MUST stay interactive on the sampler path.

The workgroup MUST NOT fall far behind. After a scroll bump or move, the user
MUST see a work-collector remap **almost instantly** — there is no good
excuse for a long visible lag before merged compute reflects the new
viewport.

### 3.3 Screen-space coordinates


**Internal:** Upper-left origin; x right, y down — buffers, queues, and grid
indices use this frame.

**User-facing:** Screen **center** as origin for display and typed coordinates.
Convert only at the UI boundary using the §4.2 viewport model.

Pan and zoom MUST NOT use a low-precision floating frame for navigation —
use the infinite-precision viewport model (§4.2) only.

---

## 4. Mathematical model


### 4.1 Mandelbrot


z₀ = 0, z_{n+1} = z_n² + c.

**Halting:**

1. **Escape:** magnitude of **z** greater than **2**.
2. **Interior:** **attracting cycle** detected (correct algorithm TBD).

### 4.2 Viewport coordinates and zoom


Viewport **location** MUST be held at **infinite precision** — **exact or
bust**. If the representation cannot place the requested center exactly, the
product MUST NOT silently round to a wrong point.

**Zoom** MUST use **binary exponent** steps so magnification can go
arbitrarily deep without running out of precision in the viewport model.

### 4.3 Deep-zoom algorithms


The product MUST **always** iterate via **perturbation** and **series
approximation** — never a mode that waits until “deep enough” to turn
perturbation on. Other explorers hit a **perturbation wall**; this app does
not.

That requires a **smart reference-orbit manager** (algorithm **TBD**) that
keeps a valid reference for the current and upcoming viewports. The
implementation MAY use **multiple threads** maintaining **lookahead**
reference orbits.

**Perturbation (split orbit):** Write the screen iterate as **z** and the
reference orbit as **Z**. For one step, with pixel parameter **c** about the
reference **C**:

```
z_{n+1} = z_n² + c
Z_{n+1} + z_{n+1} = (Z_n + z_n)² + C + c
```

Using **Z_{n+1} = Z_n² + C** and cancelling:

```
z_{n+1} = 2 Z_n z_n + z_n² + c
```

The screen location is **Z_n + z_n** (reference plus deviation).

**Series approximation (exact skip, not “fuzzy bounds”):** Early in the walk,
**z_n** stays so small that **z_n²** changes the iterate **not at all** —
it is absorbed next to **2 Z_n z_n** and **c** (squaring a tiny deviation
drives it toward zero). While that holds, the same algebra simplifies to:

```
z_{n+1} = 2 Z_n z_n + c
```

From **z_0 = 0**:

```
z_1 = c
z_2 = 2 Z_1 c + c
z_3 = 2 Z_2 (2 Z_1 c + c) + c = c (2 Z_2 (2 Z_1 + 1) + 1)
```

**Invariant:** **c** factors out. The coefficients built only from **Z** can
be **precomputed once** with the reference orbit and stored beside it. Each
screen point becomes a **search**: find the last step where absorption still
holds, then run **full perturbation** (with **z_n²**) from there. That turns
bulk iteration into lookup plus a short perturbation tail — not an approximate
skip.

### 4.4 Perturperturbation (research)


**Classical perturbation** (§4.3) keeps a fixed reference orbit and perturbs
each screen point around it. **Perturperturbation** is different: the
**reference orbit itself** is perturbed.

**Core hypothesis:** Interior points **approach the node** of the bulb or
cardioid they belong to — the point where **smallness** (smallest magnitude
of **z**) goes to **zero**. If that holds, a **secondary perturbed reference
orbit** can gather **corrections** on the way to its node and **repair**
error inherited from its **parent** reference orbit.

**If it works:** The app is no longer limited to “pick a reference orbit
precise enough and hope.” It becomes a **progressive zoomer** — in principle
able to zoom **without a fixed depth ceiling** — provided the viewport is
managed with **viewport relativity** (the **0.0.2** prototype did this).

**Current product choice:** **Viewport relativity is off for now.** The bet
is that the infinite-precision viewport model (§4.2) stays **instant** for
depths a human is likely to reach at natural zoom speed. If that bet fails
(viewport math stops feeling instant), **viewport relativity** MUST be
restored.

**Status:** Research parallel to §4.3 — not shipping. If adopted, same
completion and retention rules as production perturbation (§1.6, §1.2).

---

## 5. Coloring and settings


How retained compute becomes RGB (§1.2). The **colorer** paints from the
**value field** (`ScreenValue` per pixel, including highlighter marks from
§1.7–§1.8). The **settings** panel edits the **coloring script** and global
parameters; changes take effect per §5.4–§5.5.

### 5.1 Coloring script


The product MUST support a user-editable **coloring script**: an **ordered
list of layers** applied sequentially to build each frame’s RGB.

Each layer MUST read from the current **value field** (escape time, period,
smallness, small-time, and highlight marks). Layers MUST **composite** over
the result of prior layers (opacity and inside/outside rules per layer type).

The user MUST be able to **reorder** layers (order changes the image). The
user MUST be able to **select** a layer to edit its parameters.

The coloring layers must support separate internal and external opacities
so they can be applied to the inside or outside of the set or both.

### 5.2 Default script


The shipped default script MUST include layers that exercise the core value
field and highlights:

- Outside **escape-time** coloring (with optional animated shading).
- **Small-time** and **smallness** painting (§1.7).
- **In-filament**, **out-filament**, **node**, and **small-time edge**
  highlights (§1.8).

Until the colorer runs, incomplete pixels use the checkerboard and bootstrap
colors defined in §3.1.

### 5.3 Animatable parameters


Global and per-layer parameters MAY be **animatable** (time-varying). The
most important is **bailout radius** **R_b** (§1.4): animating it MUST
rerun the **escaper** only, not the screen worker.

When any animatable drives visible change, derivation MUST sustain **60 Hz at
1080p** (§1.2) — coloring and bailout animation stay live while compute
refines in the background.

### 5.4 Settings-only updates


**All** settings changes MUST **not** force a full recomputation of retained
`CompletedPoint` data. This allows immediate settings application at full
framerate.

The **colorer** MUST repaint from the cached value field. Changes that alter
**R_b** or other escaper-derived fields MUST rerun the **escaper** (then
colorer) from retained escape state — still without worker rerun (§1.2).

### 5.5 Settings UI


A dedicated **settings** surface (separate from the main fractal window)
MUST expose:

- **Memory Limit** slider (§1.5).
- **Bailout radius** **R_b** — the only escaper control exposed in settings
  (§1.4).
- Coloring-script **layer list** with reorder and per-layer editors.

Settings changes MUST apply immediately everywhere they affect behavior
(§5.4).

Layout, default layer table, settings wire, and UI implementation →
[DELIVERY_SPEC.md](DELIVERY_SPEC.md) §2.

---

## 6. Quality attributes


Non-functional requirements that cut across the spec. Verification detail lives in
[TESTING_SPEC.md](TESTING_SPEC.md).

**Input and viewport:** The user MUST perceive **no wait** between scroll
bumps on the sampler path (§3.1, §3.2). A gesture of **~10 scroll bumps**
(~**1024×** cumulative zoom) MUST remain interactive without UI stall.
Viewport position and pointer-anchored zoom MUST stay **exact** at the model
in §4.2; deep zoom MUST keep refining via **always-on** perturbation and
series approximation (§4.3) without breaking responsiveness.

**Derivation framerate:** Snappy preview and settings-driven repaint MUST
sustain **60 Hz or better at 1080p** where §1.2 and §3.2 require it
(sampler, animatable bailout, coloring).

**Compute throughput:** The workgroup MUST keep up closely enough that
collector remap after a scroll bump or pan is **almost instant** (§3.2) —
no multi-second hole where the merged frame ignores the current viewport.

**Correctness:** Finished pixels MUST satisfy §1.6 (escape or rigorous
period, honest incompletes only). The **main antenna** escaper trade-off
(§2.1) is the sole sanctioned relaxation of full **R_b** completion.

**Core pairing:** The product MUST optimize **completeness and speed
together** (§1.0) — neither “fast but lying” nor “correct but unusably slow.”

---

## 7. Acceptance criteria


Product-level outcomes used for release judgment. Verification detail lives in
[TESTING_SPEC.md](TESTING_SPEC.md) — not duplicated here.

### 7.1 Input and viewport


1. Each **scroll bump** MUST update the sampler preview **within one sampler
   frame** (§3.2).
2. **Rapid zoom (baseline):** **Ten** consecutive **2×** scroll bumps in
   **~200 ms** total (**~20 ms** per bump on average) MUST leave the UI
   interactive; the sampler MUST apply **every** bump without backlog (§6).
3. **Sustained and hyperfast scroll:** The same MUST hold for **long runs**
   of bumps and for input **much faster** than the baseline — including
   “spinny” wheel bursts **~10× to ~100×** faster than §7.1.2. That is a
   valid use case (e.g. zooming back in along a **retained drag anchor** after
   zooming out, §3.1). The product MUST **not miss bumps** or stall
   visibly; sampler and window MUST stay aligned bump-to-bump.
4. After **N** scroll bumps, the pointer-anchored pixel still maps to the same
   objective **c** (within documented zoom-out rounding only) (§3.1).
5. **Perturbation** and **series approximation** refine at depth without
   breaking 1–4 (§4.3).

### 7.2 Compute and color


1. Coloring-script **reorder** or **animatable** toggle → visible change at
   **derivation framerate** (colorer, and escaper when **R_b** changes) per
   §5.3–§5.4.
2. When **only** colors change, the retained **value field** is unchanged
   (§5.4).
3. **Bailout** animation stays interactive away from the **R = 2** boundary,
   except the **§2.1 main antenna** trade-off.

### 7.3 Completion and memory


1. No false periodic completion: interior finish MUST use the **correct
   period-detection algorithm** (§1.6), not caps or heuristics that declare
   done early.
2. Finished pixels are **`Repeats`** or **`Escapes`** with **smallness** and
   **small_time** — not **`Dummy`** as final state (§1.6).
3. **Memory Limit** slider, floor from screen size, 1 GB cap, and resize
   bumper behave as §1.5.

### 7.4 Platforms


1. **Desktop** (Linux X11/Wayland, Windows, macOS): full behavior in §1–§6.
2. **Web:** same UI affordances as desktop on the shared codebase (§1.9);
   platform-specific gaps are documented in [DELIVERY_SPEC.md](DELIVERY_SPEC.md),
   not excused in product criteria here.

---
