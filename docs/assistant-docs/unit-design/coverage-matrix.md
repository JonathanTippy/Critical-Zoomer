# Unit-design coverage matrix

Maps architecture statements and requirements to: authoritative unit doc, assistant supplement/new design, or explicit hole.

Status key: **covered** (auth), **closed-here** (assistant), **hole** (explicit), **code-diverge** (code ≠ arch, noted only).

## Architecture → units

| Architecture item | Auth unit doc | Assistant | Status |
|-------------------|---------------|-----------|--------|
| Stencil (homothety + resolution + desire) | (none; arch prose only) | `new/stencil.md` | closed-here |
| Tile 64×64, shared mag homothety, ≤8 mags | tile_manager, dyadic, headgroup | supplements + `new/tile_and_answer.md` | closed-here |
| CPU tile vs GPU tile; headgroup GPU-only hoard | architecture + gpu_uploader | `new/tile_and_answer.md` | closed-here |
| Tile manager pure fn; protect screen+lookahead; bump | tile_manager | `supplements/tile_manager.md` | closed-here |
| Headgroup: ingest tiles, sample, shade ≤60fps | headgroup, shaders | supplements | closed-here |
| Workgroup pipeline + sub-actors | workgroup | `supplements/workgroup.md` + `new/actor_graph.md` | closed-here |
| Ranges / calibrated honesty before publisher | tile_worker, tile_publisher | supplements | closed-here |
| Nores when no proximate | tile_publisher, architecture | `supplements/tile_publisher.md` | closed-here |
| Reference orbits background | reference_worker | supplement | closed-here |
| IntExp basis for locations | headgroup, homothety | `new/number_stack.md` | closed-here |
| Requirements allocation table | (phase boundary) | n/a | covered at arch |

## Requirements → units

| Requirement area | Auth | Assistant | Status |
|------------------|------|-----------|--------|
| Form factor / distribution | (out of unit lane) | — | hole (product/ops, not unit) |
| System policy / memory limit | tile_manager | `supplements/tile_manager.md` | closed-here |
| Control scheme + mechanics | headgroup (thin) | `new/window_controls.md` | closed-here |
| Display scheme / window / viewport | headgroup | `new/window_controls.md` | closed-here |
| Cosmetic options / coloring script | shaders (thin) | `new/settings_and_coloring.md` | closed-here |
| Seamless (no max-iter, perturb always, GPU preferred, ref bg, foveation) | tile_worker, tile_scheduler, reference_worker | supplements | closed-here |
| Deep / gears / precision | tile_worker | `supplements/tile_worker.md` + number_stack | closed-here |
| Tenacious / nores not flat black | tile_publisher, shaders | supplements + settings | closed-here |
| Hoarding / one answer per point | tile_manager, publisher | supplements | closed-here |
| Fast / natural zoom 2× | headgroup, window_controls | closed-here |
| Calibrated / ranges → answers | tile_publisher | supplement | closed-here |
| Shade tags (in-fil, out-fil, node, STE, escape, layers) | shaders | `supplements/shaders.md` | closed-here |
| E2E harness tags | (test phase) | — | hole until unit-test phase |

## Auth design files → supplements

| Auth file | Supplement | Gap severity before this pass |
|-----------|------------|-------------------------------|
| dyadic.md | supplements/dyadic.md | medium (sampling algorithm underspecified) |
| gpu_uploader.md | supplements/gpu_uploader.md | low |
| headgroup.md | supplements/headgroup.md | medium |
| homothety.md | supplements/homothety.md | low (validity only) |
| intratile_scheduler.md | supplements/intratile_scheduler.md | medium (preempt, phase jobs) |
| period_detector.md | supplements/period_detector.md | high (open choices) |
| reference_worker.md | supplements/reference_worker.md | medium |
| shaders.md | supplements/shaders.md | high (thresholds, compositing) |
| tile_manager.md | supplements/tile_manager.md | medium |
| tile_publisher.md | supplements/tile_publisher.md | high (rate, bias) |
| tile_scheduler.md | supplements/tile_scheduler.md | medium |
| tile_worker.md | supplements/tile_worker.md | high (series approx, gears) |
| workgroup.md | supplements/workgroup.md | medium (graph) |

## Units with no auth design file

| Unit | Assistant file |
|------|----------------|
| Stencil message | `new/stencil.md` |
| Tile / Answer / CalibratedAnswer / GPU encoding | `new/tile_and_answer.md` |
| Number stack (IntExp, StackedIntExp, FloatExp) | `new/number_stack.md` |
| Settings + coloring script model | `new/settings_and_coloring.md` |
| Window controls detail | `new/window_controls.md` |
| Actor graph (channels, rates, capacities) | `new/actor_graph.md` |
| GPU context + budget | `new/gpu_context_and_budget.md` |

## Explicit holes / code notes (not closed here)

| Item | Note |
|------|------|
| Form-factor packaging (Flatpak/deb) | Outside unit design |
| GPU tile publisher restore | GPU shader required; cadence [20, 100000] Hz (D-PUB-1); CPU `publish_seat` interim for single-seat until GPU tile path used |
| `shadergroup` assembly vs shaders-in-headgroup | **closed** — actor assembly removed; live shade is `gpu_display` wgsl |
| Parallel `workgroup` vs `workgroup` | **closed** — live SteadyState names match auth: tile scheduler, tile worker, intratile scheduler, reference worker, gpu uploader, tile publisher |
| Exact EWMA half-life / α for mag velocity | Assumed in tile_scheduler supplement |
| Exact N for A-REF-MAX-N | Assumed = 3 |
| In-fil / node numeric thresholds | Assumed placeholders A-SHADE-* |
