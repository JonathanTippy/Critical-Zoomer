# Shadergroup virtues (display path)

Status: **enshrined from 2026-08-11 interview** — keep these when changing
escaper / colorer. Charter: `docs/assistant/headgroup-charter.md` bucket 3
(explicit shade/display work).

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
- Criterion: `benches/shadergroup_fitness.rs` — escaper vs colorer at 1.0× /
  1.5× / 2.0× default pixel count (developer cliff ≈ 1.5×).

## Related

- `docs/assistant/collected-wisdom.md` — escaper 60 Hz @ 1080p target.
- Issue stack — high-res display lag; workgroup banding is a separate failure.
