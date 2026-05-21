# Critical-Zoomer Specification

**Version:** 0.1.0 (`critical_zoomer`)  
**Status:** Authoritative for goals, latency contract, architecture, algorithms (current and planned), and roadmap.  
**Install and controls:** [README.md](README.md).

This document uses plain language. Requirement IDs like `LR-1` or `AC-*` are not used. Standard Mandelbrot symbols (c, z, R = 2, etc.) are used where helpful.

---

## 1. Overview and goals

### 1.0 Core stress (telos)

The heart of the project is to meet **completeness** and **speed** **at once**. That pairing is a **problem** in the constructive sense: a **good stress**, a **request that needs help**—not something to shy away from.

| Pole | What it asks |
|------|----------------|
| **Completeness** | Every pixel **finished** (escaped with correct metrics, or proven periodic with approached period) or **explicitly not done yet** |
| **Speed** | Deliver that **as fast as the user cares to go** (zero input wait, deep zoom, retention, lookahead, perturbation, series approximation, graphics processor, …) |

The spec’s **telos** is to **answer both requests together**. Engineering (actors, cache, two-stage bailout, smallness, period detection, perturbation, perturperturbation experiments, series approximation, prefetch, graphics processor, platforms) serves that core stress.

**Language:** Use “stress,” “problem,” “request,” “needs help” for the completeness+speed pairing. Avoid “conflict” (goals fighting each other) and avoid calling the stress bad or the goals impossible. Interim tradeoffs (antenna bailout cap, naive period check) are **helps along the way**, not proof the stress cannot be met.

### 1.1 North star

**Mission:** Browse the Mandelbrot set **very deeply**, as fast as the user cares to go.

**Interaction contract** (normative target):

- Each scroll wheel tick is **one binary zoom step** (`zoom_pot` ± 1 → **2×** magnification change).
- A typical mouse stroke may produce **~10 ticks in quick succession** → **2^10 = 1024×** total zoom change in one gesture.
- The application must:
  1. **Apply every tick immediately** on the input path (viewport, preview, attention)—the user must **never wait** for a frame to finish before the view responds to the next tick.
  2. **Sustain rapid cumulative zoom** (1024× and beyond) without collapsing under cost per pixel at depth.
  3. **Refine quality asynchronously** after each tick (progressive fill), without blocking further input.

Responsiveness is **input-first**. Full-precision iteration for every pixel at extreme depth is incompatible with this goal; **perturbation** and **series approximation** are required end-state algorithms, not optional optimizations.

### 1.2 Compute / coloration separation

**Principle:** Mandelbrot **computation** (iteration until escape/period) is separate from **coloration** (mapping numeric results to screen colors). Computed facts are stored; colors are produced **on demand** from that cache.

**Why:** Instant coloring-script and animable changes without recomputing iterations; future prefetch targets **compute**, not pixels.

**Shipped pipeline:**

| Stage | Module | Artifact | Recomputable without iteration? |
|-------|--------|----------|--------------------------------|
| Iterate | `src/action/workshift.rs` | `CompletedPoint` | — |
| Derive metrics | `src/actor/escaper.rs` | `ScreenValue` in `ZoomerValuesScreen` | Only if bailout-derived fields change |
| Color | `src/actor/colorer.rs`, `src/action/color.rs` | `color()` → pixels | **Yes** — colorer re-runs on cached values when `Settings` updates |

Settings and `Animable` fields feed escaper and colorer independently of the screen worker.

**Caveats:**

- **Escaper is hybrid:** stage-2 bailout extension is derived iteration from stored escape state, not full recomputation from scratch.
- **Preview path** (`src/action/sampling.rs`): resamples last colored frame for pan/zoom—viewport warp of displayed pixels, not recomputation from `ZoomerValuesScreen` (required for instant input; optional future: warp from value field).

**Rules:** Coloring-script or animable change visible within one colorer frame (~8 ms class) without worker involvement. Value buffers are source of truth for non-preview imagery. Future compute modes must preserve the same value interface.

### 1.3 Speed via lookahead prefetch (roadmap)

Use **spare cycles** when the viewport is done or low priority to prefetch compute for where the user is **about to be**.

| Mode | Scale | Trigger | Purpose |
|------|-------|---------|---------|
| **Viewport pan lookahead** | ~1× neighborhood | Pan velocity | Edge regions warm when pan completes |
| **Focus zoom lookahead** | Coin-sized patch | **Eye gaze** (required) | ~50× effective speedup for examined region |

Mouse attention (`screen_worker`) helps flood-fill priority but **lags gaze** for zoom-intent prefetch.

**Idle:** When complete or quiescent, token budget shifts to prefetch (lower than active viewport, cancellable on view change).

### 1.4 Two-stage escape and animated bailout (shipped)

| Stage | Where | Threshold | Output |
|-------|-------|-----------|--------|
| **1 — Standard escape** | Screen worker | **R = 2** (`\|z\|^2 > 4`) | `CompletedPoint::Escapes { escape_time, escape_location, … }` |
| **2 — Bailout extension** | Escaper | Animatable **bailout radius** R_b | `ScreenValue::Outside { big_time, … }` |

Worker iterates only to R = 2; escaper continues from stored (c, z) to R_b. Animated bailout updates outside coloring without worker rerun.

**Known limitation — main antenna / (-2, 0):** Many pixels sit on the R ≈ 2 boundary; stage 2 crawls → animated bailout loses “superpower” speed. `bailout_max_additional_iterations` is an interim cap (section 1.6).

### 1.5 Retain completed work + memory budget

Retain finished compute as aggressively as the memory budget allows.

- **Window resizing:** Freely allowed; pixel maps vary with resolution.
- **Computed-work cache:** One setting, **100 MB – 1 GB** (inclusive). **Default: 512 MB** (`computed_work_cache_mb`).
- **Term:** `ResultsPackage`, remapped tiles, `ZoomerValuesScreen` pins, lookahead—not actor stacks (`main.rs` 200 MiB graph thread) or UI textures.

**Shipped today:** ~2-frame implicit retention; no enforced budget.

**Rules:** User limit ∈ [100, 1024] MB; eviction under limit; prefer `sample_old_values` on resize; regenerate colors from `ScreenValue`; viewport wins over lookahead under pressure.

### 1.6 Completion law

**Banned in finished app (worker / period path):**

- `max_period` and any worker cap that declares periodic/complete without correct algorithm (`determine_period` today—**violation**)
- Mandelbrot **max iteration count** that publishes finished interior/escape without proof
- `CompletedPoint::Dummy` as final published pixel state

**Allowed:**

- **Per-workshift effort limits** — pixel stays incomplete, returns next pass
- **Cancel stale in-flight work** on viewport change
- **`bailout_max_additional_iterations`** (escaper stage 2) — intentional antenna workaround until dedicated strategy

**Normative completion:**

- **Escape:** z² > R² at R = 2 (worker); stage 2 toward R_b subject to escaper cap
- **Inside / periodic:** period detection certifies repeat → `Repeats { period, smallness, small_time, … }`

| Location | Status |
|----------|--------|
| `workshift.rs` `determine_period` `max_period = 100000` | **Violation** — fix in period detection phase |
| `settings.rs` + `escaper.rs` `bailout_max_additional_iterations` | **Allowed workaround** |

### 1.7 Periodicity detection (required)

**Shipped (interim only):** Single loop checkpoint; `determine_period` with hard `max_period` cap—not compliant with completion law.

**Required (finished app):** **Derivative-based periodicity detection** (or equivalent rigorous method). Must decide escape **or** repeat. On repeat: yield **approached period** for escaper/colorer and filaments.

**Roadmap:** **Period detection phase** — after input-first, before classical perturbation.

### 1.8 Smallness and small time (shipped)

Per-point **smallness** (smallest \|z\|² seen) and **small_time** (iteration at minimum), stored on `CompletedPoint` / `ScreenValue`.

| Layer | Effect |
|-------|--------|
| `PaintSmallTime` | Bubbles outside; pinwheel inside |
| `HighlightSmallTimeEdges` | Tree structure inside |
| `PaintSmallness` | Minibrot valleys |

First-class value-field features; preserved under compute/color separation.

### 1.9 Graphics processor (roadmap)

Largest throughput win may be graphics-processor fill of iteration fields—not a drop-in replacement for steady_state actors. See section 3.6.

### 1.10 Filaments (shipped)

| Kind | Detection | Depends on |
|------|-------------|------------|
| **In-filaments** | Neighbor escape times (`big_time`) — slope sign changes | Correct outside iteration through stage 2 |
| **Out-filaments** | Neighbor loop periods increase across stencil | **Correct approached period** (section 1.7) |

Value-field features; `HighlightInFilaments`, `HighlightOutFilaments` in default script.

### 1.11 Product summary

| Field | Content |
|-------|---------|
| **Name** | Critical-Zoomer |
| **Primary goal** | Deep Mandelbrot exploration at user-driven speed with **zero perceived input latency** |
| **Platform (today)** | Linux desktop, X11-oriented (`window.rs`) |
| **Platform (required)** | One codebase: Linux X11 + Wayland, Windows, macOS, web |
| **Non-goals** | Per-OS forks; general fractal suite; cloud-only rendering |

**Current vs target:**

| Capability | Shipped (0.0.x) | Target |
|------------|-----------------|--------|
| Viewport math | `IntExp` + `zoom_pot` | Same; reference for series approximation |
| Per-pixel iteration | `f64` direct | Perturbation + series approximation |
| Scroll response | Preview + cancel stale work | Instant every tick |
| Compute vs color | `ZoomerValuesScreen` + on-demand `color()` | Preserve through deep-zoom algorithms |
| Work retention | ~2-frame implicit | 100 MB–1 GB cache; free resize |
| Completion law | `max_period` debt | Period detection phase |
| Graphics processor | CPU actors only | Option B + GPU workshift (3.6) |
| Platforms | Linux X11 | X11, Wayland, Windows, Mac, web |

---

## 2. User-facing behavior

**Main window** (800×480 default): pan (drag), zoom (scroll), home (`HOME_POSITION`), settings viewport, free resize.

**Preview path:** On viewport change before worker catches up, `sampling.rs` resamples last `ZoomerScreen` — **required** for north star.

**Idk display:** `ScreenValue::Idk` (from `CompletedPoint::Dummy`) is drawn in the **colorer only** as a Minecraft missing-texture checkerboard: `#FF00FF` and `#000000` on **64×64** screen-space tiles (`idk_checkerboard_rgb` in `constants.rs`). Before the first colored frame, the **window** shows flat purple (`WINDOW_IDK_RGB`, 128, 0, 128); pan preview holes without cached coverage use the same flat purple (no lookahead today). Known-computed pixels use the coloring script only.

### 2.1 Input and zoom requirements

| Requirement | Definition |
|-------------|------------|
| **Instant input** | Viewport and preview update within the same UI frame as each scroll/drag event |
| **Latest viewport wins** | Discard stale worker frames; latest viewport always wins |
| **Progressive refinement** | Pixels may be low-res briefly; must not block input |
| **Rapid deep-zoom gestures** | ~10× 2× ticks (1024×): preview stays interactive |

**Normative:** One scroll tick = **2×** zoom (`pot` ± 1). Verify/implement consistently on all platforms.

### 2.2 Screen-space coordinate convention (shipped internal, target UX)

**Shipped (internal):** Pixel indices, queue positions, attention tuples, and screenspace offsets in the compute and sampling pipeline use an **upper-left origin**: x increases right, y increases down (same as framebuffer row-major indexing and egui layout coordinates). Objective Mandelbrot position (`IntExp` pair + `zoom_pot`) is derived from that frame—see `src/action/sampling.rs`, `src/action/workshift.rs`, `index_from_pos` in `src/action/utils.rs`.

**Not shown today:** Coordinate readouts and editable position fields are not in the UI yet, so the corner origin is invisible to users.

**Planned (exploration / navigation UX):** Whenever coordinates are **presented** (status line, settings, copy/paste, path tracking) or **ingested** (jump-to-position, `SetPos`, focus commands), use **screen center as origin** (0, 0 at center; positive x right, positive y up or down per chosen UI convention—document the axis choice when implemented). **Minimal surprise:** users expect “where am I looking” relative to the middle of the view, not the top-left pixel.

**Implementation rule (normative):**

- **Keep internal math corner-based**—do not re-root worker buffers, order maps, or flood-fill indices.
- **Convert only at the UI boundary** when translating between user-facing center coordinates and internal corner-based pixels or `IntExp` objective space. Use the same `IntExp` + `zoom_pot` + `PIXELS_PER_UNIT_POT` machinery as zoom/pan so deep-zoom precision is preserved (no parallel `f64` screen frame).
- Scroll zoom at cursor already passes a screenspace point into `Zoom { center_screenspace_pos }`; when center-origin UI ships, document whether that field stays internal-corner-based (egui-native) with conversion at ingest, or is redefined to center-origin in the command enum—either is acceptable if conversion is explicit and tested.

**Roadmap:** **Exploration features** phase (and any navigation UI before then)—center-origin display and editing; internal representation unchanged.

---

## 3. Mathematical model

### 3.1 Mandelbrot (baseline)

z₀ = 0, z_{n+1} = z_n² + c.

Outcomes: `Escapes` or `Repeats` per `CompletedPoint`—only valid terminal states (section 1.6).

### 3.2 Coordinates (shipped)

**Objective (Mandelbrot c):**

- `IntExp` navigation (`src/action/utils.rs`) — arbitrary-precision real and imaginary parts with binary exponent.
- `zoom_pot`: binary zoom steps; `PIXELS_PER_UNIT_POT = 9` links screenspace scale to objective units.

**Screenspace (internal):**

- **Origin: upper-left** of the image (pixel (0, 0) is top-left; y down). Used for indexing, work queues, and command math today.
- **User-facing origin: screen center** when coordinates are shown or typed — see section 2.2; conversion at UI boundary only, still via `IntExp`.

**Conversion sketch (center ↔ corner, for UI):** Let resolution be (W, H). Internal pixel offset from top-left (px, py) corresponds to center-relative screenspace (sx, sy) with sx = px − W/2, sy = py − H/2 (integer half-width/half-height per chosen rounding rule). Map (sx, sy) to Δc in objective space using current `zoom_pot` and `PIXELS_PER_UNIT_POT` the same way pan/zoom already shifts `context.location.pos` in `sampling.rs`.

### 3.3 Deep-zoom algorithms (roadmap)

**Perturbation theory:** Reference orbit Z_n at c₀; per-pixel δ with D_{n+1} = 2 Z_n D_n + D_n² + δc. Recompute reference on viewport jump.

**Series approximation:** Truncated Taylor in c − c₀ to skip iterations where error bounds < pixel spacing. Stack: series bulk skip + perturbation for remaining pixels + occasional high-precision refresh.

**Mandatory for north star:** Direct iteration cost ∝ iterations × pixels; both algorithms required at depth.

**Gap:** `workshift.rs` uses `WorkContext<f64>` only today.

### 3.4 Perturperturbation (research)

Experimental: **reference orbit itself** perturbed; hypothesis that interior dynamics stabilize error. **Research track** parallel to classical perturbation—not a ship commitment. Must satisfy completion law and value-buffer rules if adopted.

### 3.5 Platforms

**Stack:** egui + eframe + winit (0.32.x); single actor/window architecture.

| Target | Requirement |
|--------|-------------|
| Linux | X11 and Wayland |
| Windows | Native via eframe/winit |
| macOS | Native via eframe/winit |
| Web | Browser build; graph runtime + egui threading may need adaptation |

One crate; target cfgs and feature flags—not duplicate apps. Same feature set on all surfaces where OS policy allows.

### 3.6 Graphics processor architecture

**Today (CPU):** steady_state actors; screen worker sparse/order-mapped iterate; escaper filaments + bailout; colorer instant recolor; pixel order maps.

**Standard GPU fractal app:** One program per pixel; iteration in shader; colors baked in—loses section 1.2 and filaments unless rebuilt.

| Option | Summary | Fits core stress |
|--------|---------|------------------|
| **A — Actors only** | Improve worker, messaging; GPU deferred | Strong |
| **B — Hybrid: GPU iterate, CPU rest** | GPU textures for escape time, period estimate, smallness, small_time; escaper + colorer on CPU | **Preferred** |
| **C — GPU iterate + GPU color** | Two shader stages | Medium |
| **D — Full GPU pipeline** | Typical explorers; actors UI-only | Weak for product identity |
| **E — Tile hybrid** | GPU fills tiles; order map picks tile | Strong |

**Near term:** Option A + sections 4.2–4.3.  
**First GPU experiment:** **B with graphics-processor workshift** (3.6.2); optional dirty tiles (3.6.1).  
**Technology sketch:** Rust wgpu; compute shader → float textures; CPU readback for escaper; egui texture display.

#### 3.6.1 Done pixels and wasted GPU cycles

Naïve full-frame dispatch spends launches on finished pixels. Mitigations (at least one required for GPU phase):

1. **Done / stale bitmask** in value texture — early return in shader.
2. **Dirty regions / tiles** — dispatch only changed rectangles.
3. **Sparse incomplete list** — CPU-maintained active set per launch.
4. **Scoped readback** — escaper reads only dirty regions.

**Warp divergence** (different iteration counts in one warp) is separate from done-pixel waste; bounded passes help both.

#### 3.6.2 Graphics-processor workshift

GPU mirrors CPU workshift: **repeated bounded passes**, then **refangle** (rebuild) the active point set.

| CPU workshift (target) | GPU workshift |
|------------------------|---------------|
| Batch budget or ~10 ms today | **Max iterations per launch** per active point |
| Incomplete → next pass | Same |
| Complete → leave frontier | Removed from next launch; written to texture |
| Order map picks who | Refangle: compact buffer / indirect dispatch = incompletes only |

**Per-pass loop:**

1. Upload active set (coordinates, partial state in texture/SSBO).
2. Dispatch: at most N iterations; early exit at R = 2 or GPU terminal condition.
3. Fence; read back only if escaper needs batch (tile-scoped).
4. Refangle on CPU: drop finished; add invalidated; merge attention / flood-fill / order map.
5. Repeat until quiescent, preempted by input, or viewport replace.

**Why:** Cooperative with input (section 2.1); avoids one long divergent kernel; **done points get off** between passes—not mid-warp pause, but not rescheduled.

Tiles optional for locality; **sparse active list required**.

**GPU phase acceptance:** Document max-iterations policy; measure active set vs full frame; escaper/colorer unchanged on CPU for stage 1.

---

## 4. Runtime architecture

### 4.1 Graph runtime thread vs compute

**Misnomer today:** `main.rs` thread `"worker-thread"` with 200 MiB stack does **not** run Mandelbrot math.

**Actual role:** `graph.start()` / `block_until_stopped` — babysits steady_state actor graph.

**Compute:** **`screen worker`** (`screen_worker.rs`) calls `workshift()`.

**200 MiB stack:** Graph runtime thread **owns all child actor states on its stack** (`with_default_actor_stack_size`). Enables **limping through panics**—steady_state signature. Not because coordinator bookkeeping is large, and not because this thread iterates Mandelbrot.

**SPEC terminology:** **Graph runtime thread** (or **actor graph thread**). Never “compute worker thread.”

### 4.2 Workshift design

**Shipped issues:**

| Current | Issue |
|---------|--------|
| Full-screen `Vec<Point>` | Huge footprint; most pixels untouched per slice |
| ~10 ms workshift loop | No command channel poll until return |
| `WorkUpdate` with `Vec` batches | Heap allocation per send |

**Target:** Smaller working set (queues, frontier only); cooperative scheduling with **command/attention poll** between batches; order map + sparse point store as backbone.

**Pixel order maps (shipped):** `mixmap` / `random_map` in `work_controller.rs`. **Target:** first-class artifact—array or lazy generator; optional strategy function (spiral, Morton, gaze-weighted).

### 4.3 Channel messages

**Reference:** [steady-state-performant](https://github.com/JonathanTippy/steady-state-performant) — `peek_slice` / `poke_slice`, small fixed messages, cache-friendly slices.

**Today (misaligned):** `WorkUpdate` with `Vec`, `WorkerCommand::Replace` with full `WorkContext`, `ResultsPackage` vectors on hot path.

**Target (steady-state messaging phase):** One completed point (or tiny struct) per send; viewport metadata + incremental indices on control path; full frames only in computed-work cache.

### 4.4 Shipped actor pipeline

```
window → work controller → screen worker → work collector → escaper → colorer → window
         Settings ──────────────────────────────→ escaper, colorer
```

| Actor | Role |
|-------|------|
| screen worker → work collector | Compute (`CompletedPoint`) |
| escaper | Stage-2 bailout, derive `ScreenValue`, filaments |
| colorer | `color()` → screen pixels |

**Future actors:** Reference orbit; series approximation planner; prefetch scheduler (lookahead).

---

## 5. Computation and scheduling (current)

**Shipped foundation:** ~10 ms workshift, token budget, flood-fill queues, attention, work reuse, cancel stale in-flight only, `iterate_max_n_times` per bout—incomplete points stay incomplete (not banned caps).

**Input-first phase:** Workshift redesign (4.2); messaging alignment (4.3); knowledge leverage (`partial_knowledge.rs`), movement.

---

## 6. Coloring and settings

**Shipped:** Coloring script (ordered layers), `Animable` parameters, drag-and-drop reorder (`egui_dnd`), `DEFAULT_COLORING_SCRIPT` (7 layers).

**Behavior:** Colorer holds `ZoomerValuesScreen`; on wake runs `color()`; settings-only updates repaint from same values.

**Future:** New instructions in `color.rs` only; deep-zoom modes populate same `ScreenValue` semantics.

---

## 7. Dependencies and build

See [README.md](README.md): Rust, build-essential, m4.

**Build profile (normative):** Use **`--release`** for normal runs, manual testing, profiling, and acceptance checks. The actor pipeline and Mandelbrot workshift are CPU-bound; debug builds are far slower and distort startup time, frame pacing, and graph telemetry (for example long purple placeholder frames and idle-looking actors). Reserve debug builds (`cargo build`, `cargo run` without `--release`) for debugging with a debugger or extra checks.

| Intent | Command |
|--------|---------|
| Run the app | `cargo run --release` |
| Build only | `cargo build --release` |
| Debug / lldb | `cargo build` then `cargo run` (or `RUSTFLAGS` as needed) |

`rebuild.sh` already watches sources and runs `cargo build --release`.

**Dependencies:** clap, eframe/egui 0.32, rug, steady_state 0.2.9, winit, rand.

**Future (note only):** Extended precision beyond `rug` for reference orbits—deferred.

---

## 8. Quality attributes

| Attribute | Target |
|-----------|--------|
| **Input latency** | 0 ms perceived wait between scroll ticks |
| **Zoom gesture** | 10× 2× ticks without UI stall |
| **Depth** | Beyond `f64` c via perturbation + series approximation |
| **Correctness** | Escape or proven period (section 1.6) |
| **Core stress** | Completeness + speed together |

**Shipped prerequisites (0.0.2–0.0.6):** Drag+zoom stability, resize, home, work saving, flood fill, settings.

---

## 9. Roadmap

Phases ordered by dependency.

| Phase | Scope | Status |
|-------|-------|--------|
| **Interactive foundation** | Window, actors, workshift, work reuse, flood fill, settings, `IntExp` | **Done** (0.0.1–0.0.6) |
| **Input-first zoom and cache** | Instant input; 100 MB–1 GB cache; workshift redesign; graph runtime naming | **In progress** |
| **Steady-state messaging** | Point-sized messages; slice APIs; no full-screen `Vec` on hot path | **Planned** |
| **Period detection** | Derivative-based prover; remove `max_period` | **Planned** |
| **Graphics processor** | Option B; GPU workshift + refangle; CPU escaper/colorer | **Planned** |
| **Classical perturbation** | Reference orbit + δ iteration | **Planned** |
| **Perturperturbation (research)** | Experimental perturbed reference | **Research** |
| **Series approximation** | Taylor orders, error bounds, region skipping | **Planned** |
| **Cross-platform** | Linux X11+Wayland, Windows, macOS, web | **Planned** |
| **Deep-zoom validation** | 10-tick 1024× benchmarks; correctness vs reference | **Planned** |
| **Exploration features** | Point path tracking, Julia overlay; **center-origin** coordinate display/edit (section 2.2) | **Planned** |
| **Lookahead prefetch** | Pan + gaze coin-patch; eye tracking | **Planned** |
| **Antenna bailout (open)** | Replace escaper iteration cap when design chosen | **Undecided** |

**Known limitations:** Antenna zoom; naive period check until period detection phase; implicit two-frame cache until memory budget ships.

**Optional:** `partial_knowledge.rs` may inform knowledge leverage—not a substitute for perturbation + series approximation.

---

## 10. Acceptance criteria

### Shipped smoke test (today)

1. `cargo run --release` on Linux/X11
2. Drag pan; scroll zoom at cursor
3. Home; resize; settings reorder
4. Partial refinement visible while moving

### Deep zoom and input

1. Single scroll tick updates preview in the same interaction frame.
2. Ten rapid 2× ticks (1024×): UI remains interactive.
3. At depth, perturbation and series approximation refine without breaking 1–2.
4. Idle view converges within documented error bounds vs high-precision reference.

### Compute and color

1. Reorder script or toggle animable → visible change without worker stall.
2. Value field unchanged when only colors change.
3. Bailout animation interactive away from R = 2 boundary.
4. Documented exception: antenna deep zoom may slow bailout animation.

### Completion law

1. No worker `max_period`-style completion without proof (after period detection phase).
2. Finished pixels use `Repeats` or `Escapes` with smallness + small_time—not `Dummy` as final.
3. Known periods stable at depth after period detection phase.

### Memory

1. Cache limit 100 MB–1 GB; resize safe; eviction respects limit.

### Graphics processor phase

1. Document throughput goal vs CPU baseline (aspirational large gain on fill-bound frames).

### Cross-platform phase

1. Build/run documented for Linux (X11, Wayland), Windows, macOS, web from one repo.
2. Pan, zoom, settings, recolor on each target.

### Lookahead prefetch phase

1. Prefetch uses spare cycles without blocking instant input.
2. Gaze-directed zoom measurably faster than off-gaze control.

---

## Appendix A — Unimplemented commands and CLI

### ZoomerCommand variants

Defined in `src/actor/window.rs`; handled in `src/action/sampling.rs`:

| Command | UI wired | Behavior today |
|---------|----------|----------------|
| `Zoom { pot, center_screenspace_pos }` | Yes (scroll) | Full viewport update |
| `Move { pixels_x, pixels_y }` | Yes (drag, keys) | Pan |
| `MoveTo { x, y }` | Partial | Sets position |
| `SetZoom { pot }` | No | Sets `zoom_pot` |
| `SetFocus { pixel_x, pixel_y }` | No | **No-op** (empty match arm) |
| `SetPos { real, imag }` | No | **No-op** — planned with center-origin navigation UX (section 2.2) |
| `TrackPoint { … }` | No | **No-op** — path tracking roadmap |
| `UntrackPoint { point_id }` | No | **No-op** |
| `UntrackAllPoints` | No | **No-op** |

Julia overlay and path tracking depend on exploration features phase.

### CLI (`src/arg.rs`)

| Flag | Default | Wired |
|------|---------|-------|
| `-r` / `--rate` | 2 ms | Parsed via `MainArg`; passed to graph build — **not used** for runtime pacing in current `main.rs` |
| `-b` / `--beats` | 30000 | Parsed — **not used** for shutdown loop |

Reserved for future telemetry or test harnesses.

---

## Appendix B — Legacy / non-roadmap modules

Present in tree but not part of the active actor pipeline:

| Path | Notes |
|------|-------|
| `src/action/collect.rs` | Legacy collection helpers |
| `src/action/streaming.rs` | Legacy streaming experiment |
| `src/action/do_work.rs` | Legacy work driver |

Do not extend these for new features without explicit migration plan. Docker/lambda scripts under repo (if any) are deployment experiments, not product architecture.

---

## Appendix C — Mechanically sympathetic actor comms

Study [steady-state-performant](https://github.com/JonathanTippy/steady-state-performant) for:

- Small messages per channel slot
- `peek_slice` / `poke_slice` in-place processing
- Generator/worker slice fill patterns

Goal for **steady-state messaging phase:** hot path matches L3-friendly access, not heap `Vec` per message.

---

## Appendix D — Code anchors

**Zoom command:** `src/actor/window.rs` — `Zoom { pot, center_screenspace_pos }`.

**Preview:** `src/action/sampling.rs` — `sample()` on viewport commands.

**Workshift:** `src/action/workshift.rs` — `workshift()`, stage-1 escape at R = 2.

**Color:** `src/actor/colorer.rs` — `color(v, &mut settings)` on cached values.

**Stage-2 bailout:** `src/actor/escaper.rs` — extend from escape state to `big_time`.

**Graph thread:** `src/main.rs` — spawn `"worker-thread"`, `STACK_SIZE`, `graph.start()`.
