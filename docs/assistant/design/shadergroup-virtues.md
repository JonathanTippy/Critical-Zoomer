# Shadergroup virtues (display path)

Status: **enshrined from 2026-08-11 interview** — keep these when changing
escaper / colorer. Charter: `docs/assistant/headgroup-charter.md` bucket 3
(explicit shade/display work).

## Colorer parity (binding — 2026-08-11 interview)

The colorer was written over a long grind and is **exactly how it should look**.
Performance upgrades (including aggressive rewrite or GPU) are allowed only as
an **honest rewrite**:

- **Feature parity** with the live CPU colorer — no cutting layers, settings, or
  behaviors.
- **Same results** for the same inputs (settings + escaper values) — no “close
  enough” visual substitutes.
- **No simplifications** dressed up as ports.
- **Every behavior guarded by tests** before/with the rewrite.

Prior GPU shade attempts that dropped features or changed the look are rejected
as a pattern. Prefer measuring (`shadergroup_fitness`) and then rewriting under
this bar — do not thin the product to win the clock.

## Color gear switch

Same idea as the screen-worker **manual gear** switch: settings expose

- **GPU** — honest f32 wgpu rewrite (`colorer/gpu/`) — **default** (2026-08-11)
- **OG** — CPU colorer (golden look; compare/rollback)

so the two can be compared and rolled back. The automatic PPS/kernel gearbox
must **not** auto-pick shade GPU. HUD stamps `color:OG|GPU|GPU→OG` (fallback
only when no usable device). Exact `Color32` parity is pinned by
`gpu_matches_og_*` tests.

GPU colorer keeps **persistent buffers** and may skip **redundant GPU uploads**
when the resident answers are already on device (mechanical sympathy). Actors
still always paint each content wake — upload skipping is not skip-send.
Animated layers / new packages force a fresh upload so silent skip cannot
freeze anim.

## Escape gear switch

Same pattern for the bailout tail:

- **OG** — CPU `escape_frame` (default)
- **GPU** — f32 continue R=2→radius only (`escaper/gpu/`); interiors pass-through

Manual only; never auto. HUD stamps `escape:OG|GPU|GPU→OG`. Resident answer
buffer; radius is a uniform on anim ticks. **Own wgpu device/queue**
(compartmentalized from the colorer — no shared `shade_ops`). Oracle: GPU
matches an f32 CPU twin; `big_time` matches OG under the same
`bailout_max_additional_iterations`.

**Actor API stays strong (2026-08-12):** keep escaper ↔ colorer as separate
actors with a real values channel between them. Do **not** fuse escape+color on
one device or collapse the stage boundary unless measurement proves every
within-actor option is exhausted. Preferred order when `esc:` is short:

1. Cheap host work on the escaper (package convert, packing)
2. Escaper-local GPU shipping (readback / upload) without changing the API
3. Escaper-local cadence (resident content-beat re-emit)
4. Cross-actor residency / fusion — last resort only

Live design talk: `docs/assistant/interviews/2026-08-11-shade-gpu-residency.md`.

**Stencil discipline (2026-08-12):** a `PointStencil` carries two `IntExp`
coordinates. Those integers get **large at design depth**, so any per-pixel
`clone` / touch is not a home-zoom nit — it is a deep-zoom time bomb. Rule:
touch the stencil **O(1) per actor loop / package** (read width, origin, space
once), never **O(pixels)**. Seat/row from index+width only after that.

## Bailout tail is intentionally tiny (principal)

The separate escaper exists because **continuing past ‖z‖²>4 is almost always
≤~10 iterations**, even when the animated bailout radius is huge: escaped
orbits *superpower*-escape. Default `bailout_max_additional_iterations = 10`
is policy matching that math, not a temporary cap waiting for “real” work.

Consequence for ports: **GPU escape cannot win by doing more FLOPs at large
radius.** The arithmetic budget is always tiny. Any GPU path that still ships
full-frame answers/values through the host will pay PCIe/map costs that dwarf
the kernel. Measured (2026-08-11, 854×480): dispatch ~0.05 ms; upload-path
pack+write ~5 ms; map_async readback ~4–5 ms; CPU OG escape ~5–6 ms Criterion.
Escape gear stays **OG default** until shipping is solved, not until radius is
large.

## GPU shipping economics — why colorer wins and escaper does not

Both GPU colorer and GPU escaper currently round-trip through the host
(upload → compute → `map_async` readback). They do **not** have different
shipping *laws*; they have different **cost ratios**:

| stage | CPU baseline (1.0× default) | GPU payload shape | Why GPU helps / hurts |
|---|---|---|---|
| **Escaper** | ~5–6 ms | fat in (~48 B/px answers) + fat out (~32 B/px values) for ~≤10 iters + `atan2` | Shipping ≈ whole CPU job → GPU ties or loses |
| **Colorer** | ~43–55 ms OG (multi-layer script) | fat in (~32 B/px values) + **thin out (4 B/px RGBA)** | Replaces a *much* larger CPU walk; readback is 8× smaller than escape out → net win (~13 ms class after always-refresh) |

So the colorer is not “immune” to GPU data shipping — it still pays upload and
readback — but its **CPU alternative was the problem child** (~10× escaper),
and its **output is display-thin**. The escaper’s CPU alternative was already
cheap, and its output is still a full values frame the next stage re-consumes.

**Live rates override benches (2026-08-12):** headed HUD with GPU escape showed
roughly **`esc:~15` then `esc:~9` / `col:~45–50`**. Dual-device did not fix that.
A dummy-head convert-hoist pass once printed GPU **esc ~60 Hz**. That was a
**snapshot**, not a standing rate: dummy-head pin is GPU esc ≥40
(`tests/pipeline_cadence.rs`) on **debug + opt-level 3** (house `cargo test`);
`--release` is not the gate (overflow checks off); **unoptimized** debug misses
the floor; **headed HUD was not re-checked**. Do not cite ~60 Hz as current
truth. Shipping/host
round-trip (map wait) remains.

Stacking GPU escape + GPU color today makes the middle worse: escape readback
into `ZoomerValuesScreen`, then colorer re-packs nearly the same layout as
`GpuPixel` and uploads again. Dual parallel CPU/GPU channel views (one idea
under interview) would push that upload problem onto every linkage unless
residency is real end-to-end.

Live design talk: `docs/assistant/interviews/2026-08-11-shade-gpu-residency.md`.

## Single path

There is **one** shade pipeline body:

1. Workgroup publishes answers → collector package.
2. **Escaper** walks the full frame once per wake (`escape_frame`) — neighbor
   cues + continue-to-bailout.
3. **Colorer** walks the full frame once per wake (`color`) — layer script.

Animated bailout (and animated color params) are **not** a second path. The
same functions run; only the numbers flowing through change. That separation —
workgroup computes membership/escape-to-4, shadergroup continues and paints —
is load-bearing for comprehensibility and maintainability. Do not fork
“static bailout” vs “animated bailout” code paths.

## Steady-state on this path

- Small channels; drain toward the tip.
- When the shade path falls behind, escaper/colorer **drop intermediate
  full-frame packages** (count on HUD `drop:` / `ViewHud.packages_dropped`) and
  keep the newest. Persistent drops mean the shade path is too slow for the
  pixel count — fix cost, do not grow channels into a landfill.
- **Cadence (live 2026-08-13):** collector, escaper, and colorer wake on
  `Settings::resolved_content_period()` (Automatic = head `auto_vsync_hz`, not
  hardcoded 60). Head present is still bare `request_repaint` (CPU open). See
  `pipeline-refresh-rates.md`.
- Criterion: `benches/shadergroup_fitness.rs` — escaper vs colorer at 1.0× /
  1.5× / 2.0× default pixel count (developer cliff ≈ 1.5×).

## Related

- `docs/assistant/collected-wisdom.md` — escaper 60 Hz @ 1080p target.
- Issue stack — high-res display lag; workgroup banding is a separate failure.
