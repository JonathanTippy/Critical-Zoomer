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

- **OG** — CPU colorer (golden look; default)
- **GPU** — honest f32 wgpu rewrite (`colorer/gpu/`)

so the two can be compared and rolled back. The automatic PPS/kernel gearbox
must **not** auto-pick GPU color. HUD stamps `color:OG|GPU|GPU→OG` (fallback
only when no usable device). Exact `Color32` parity is pinned by
`gpu_matches_og_*` tests.

GPU colorer keeps **persistent buffers** and skips uploads when values/params
are clean (mechanical sympathy). Animated layers / new packages mark dirty
every wake so silent skip cannot freeze anim.

## Escape gear switch

Same pattern for the bailout tail:

- **OG** — CPU `escape_frame` (default)
- **GPU** — f32 continue R=2→radius only (`escaper/gpu/`); interiors pass-through

Manual only; never auto. HUD stamps `escape:OG|GPU|GPU→OG`. Resident answer
buffer; radius is a uniform on anim ticks. Shares the colorer wgpu device under
a short shade-ops lock. Oracle: GPU matches an f32 CPU twin; `big_time` matches
OG under the same `bailout_max_additional_iterations`.

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
- **Cadence (design lock):** shade path aims at **vsync** (~60 Hz default), not
  the current ~8 ms “as fast as periodic wake” forever. Workgroup *publish*
  aims ~20 Hz. See `pipeline-refresh-rates.md`.
- Criterion: `benches/shadergroup_fitness.rs` — escaper vs colorer at 1.0× /
  1.5× / 2.0× default pixel count (developer cliff ≈ 1.5×).

## Related

- `docs/assistant/collected-wisdom.md` — escaper 60 Hz @ 1080p target.
- Issue stack — high-res display lag; workgroup banding is a separate failure.
