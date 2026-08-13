# RCA: i64 naive four-quadrant grey (2026-08-13)

Headed (developer screenshots, not a tokenizer paraphrase):

- `mag 2^43  -0.1761779392230477 + 1.0870336335448237i`
- `mag 2^43  -0.2067325560057166 + 1.1075689870974698i`
- `mag 2^44  -0.104628673854 + 0.959133336119i`

Picture: **four axis-aligned quadrants of flat grey**, one slightly lighter.
Sharp horizontal + vertical cuts. Not Mandelbrot. Not one uniform field.

HUD: `stack:i64` `mode:naive` `gear:F64` (OG stamp — read `stack:`) `ipp:0`
pps high. `HEADED_I64_GREY_*` is the first locus.

**Not fixed.** Shade is a pipe (v0.1 interview): what the screen worker
answers is the picture.

---

## Retracted

1. **OG-black was i64.** Mag-~38 black was OG naive **f64**. Assistant
   regression. `gear:F64` is the kernel stamp.
2. **Collector `to_f64(c)`.** `c` is not a render input and must not be
   emitted (`docs/assistant/design/work-update.md`). Publish is f32 `z` +
   f64 scalars. That cannot draw four screen rectangles.
3. **“Flat grey” as the symptom.** The screenshots are a **2×2**.

---

## Scope: screen worker only

Manual Naive `DirectKernel` on `CopyIntExp<1>`. Absolute C-generator
(`relative_ok: false`). Completions are Escapes / Repeats / Dummy; those
are what shade paints (escape time / smallness / interior).

This view does **not** cross `c = 0`. The whole patch is Re < 0, Im > 0
(width ~ `854 × 2^-(43+9)`). Screen quadrants are **not** the complex-plane
axes. They are **pixel-index** axes.

---

## Mechanism

`CGenerator::get_c` is `origin + space * seat` (and `origin − space * row`)
in `CopyIntExp<1>`. Admit is bit-*count* (64 ≥ ~54 at mag 43, |c|~1) and
passes. `get_c` then **adds** a `2^-52` pitch onto an origin near 1.

CopyIntExp add **squeeze-aligns to the coarser exp** and right-shifts the
finer mantissa. If origin’s stored exp is coarser than pitch, the pixel
offset shifts off the one-word tape. Only the **high bits of seat/row**
survive.

One surviving bit of seat → a vertical cut near `2^k` columns (e.g. 256 or
512), not the window midline. One surviving bit of row → a horizontal cut
near `2^k` rows (e.g. 256 of 480 ≈ lower ~47%). **Four constant-`c` tiles.**
Each tile is one orbit / one smallness → one grey; one tile a hair lighter.

That is precision-wall **failure shape C** (admit honest, then a later drop)
and the interview’s **rectangular low-res** look, at the limit: the whole
window is a 2×2.

`loop_check` / false period can still make a tile interior. Secondary.
Provisional scredge does not draw a centered 2×2.

`og_copy_intexp1_headed_mag_43_not_all_interior` uses `TEST_SCREEN_RES`
(16×17). A 16-wide grid can still show more than one escape time while
854×480 is four cells. That pin does not see this picture.

---

## What a fix has to do (not done)

`get_c` on `CopyIntExp<1>` must keep neighbor `c` distinct for every seat
and row at headed res, at this mag. Admit count is not that proof. Product
change is developer-driven.

Do not re-break OG f64 naive. Do not put `c` on WorkUpdate to “fix” grey.
