# Product Tracey rules (assistant-owned mapping of requirements)

Derived from `docs/authoritative/requirements.md` Central Differentiators / Display. Not authoritative text.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). Rules below fall into three
> classes, marked per rule:
> - **STANDS** — product rule the v0.0.9 design embodies; verify against restored symbols.
> - **GAP** — product rule with no implementation at v0.0.9 (the feature arrived later);
>   scheduling its port is a product decision.
> - **SUSPENDED** — rule only makes sense for machinery (tiles, references, GPU compute,
>   multi-mag hoards) that the revert removed; it returns with the GPU/depth port described in
>   `docs/assistant/design/design-target.md` and must then follow `docs/assistant/design/workgroup-virtues.md`.
> All checkboxes earned on the tile machine were cleared; nothing below is currently verified
> against the restored tree.

r[cz.seamless.perturbation-always-on+1]

**Normative summary.** Perturbation must always be on; there is no user toggle.

**Acceptance criteria.**
- **SUSPENDED.** v0.0.9 iterates f64 directly; there is no perturbation path. Returns with the
  depth port. When it returns: perturbation on the only path, no user-facing off switch.

r[cz.seamless.gpu-preferred+1]

**Normative summary.** GPU acceleration must always be on (no user toggle); when a device
exists it is preferred over CPU for tile work.

**Acceptance criteria.**
- **SUSPENDED.** v0.0.9's GPU use is display-side only (shadergroup escaper/colorer); iteration
  is CPU. Returns with the GPU compute port — as views-not-tiles, per the design target.

r[cz.seamless.reference-background+1]

**Normative summary.** Reference orbits are computed in the background without a progress bar or blocking the UI.

**Acceptance criteria.**
- **SUSPENDED.** No reference orbits at v0.0.9. Returns with depth work; the binding constraint
  that survives: reference computation must never block or gate the UI.

r[cz.seamless.foveated-mag-velocity+1]

**Normative summary.** Work order follows the user's focus: foveated begin-near-attention;
zoom-out prefers edge/low-res fill. Foveation is from the mouse.

**Acceptance criteria.**
- [ ] STANDS (foveation half): the attention position biases scheduling — at v0.0.9 the Random
  slot (one shift in five) samples ±50px around the cursor (`workshift.rs`). Re-verify headed.
- **SUSPENDED** (mag-velocity half): depth-first lookahead columns are tile-era. v0.0.9's
  answer to zoom-out is remap-restore of the hoard, which the virtues doc argues is the
  better mechanism anyway.

r[cz.tenacious.nores-not-flat-black+1]

**Normative summary.** Unfinished pixels without proximate work must not be painted as set-black; use an outside-flavored placeholder.

**Acceptance criteria.**
- [ ] STANDS. v0.0.9 symbol: `CompletedPoint::Dummy{}` — collector initializes packages with
  it (`work_collector.rs`), escaper paints it outside-looking (`escaper.rs`). Re-verify the
  full sample→shade path never renders Dummy as Inside. (Principle record:
  `docs/assistant/collected-wisdom.md`.)

r[cz.fast.natural-zoom-2x+1]

**Normative summary.** One mouse wheel bump zooms by 2× magnification (one POT step).

**Acceptance criteria.**
- [ ] STANDS. Scroll handling with debt thresholds lives in `headgroup/window/inputs.rs`
  (verified present). Re-verify: one discrete bump ⇒ zoom_pot ±1.

r[cz.fast.settings-100ms+1]

**Normative summary.** Settings changes visible within 100ms (hoard recolor / shade path).

**Acceptance criteria.**
- [ ] STANDS in design: v0.0.9 `Animable` settings are timestamp+speed driven
  (`settings.rs`), and cosmetics recolor from the hoard without recompute. Re-verify timing
  on the restored shade path.

r[cz.fast.cosmetic-17ms-1080p+1]

**Normative summary.** Continuous cosmetics animable within 17ms at 1080p.

**Acceptance criteria.**
- [ ] STANDS in design (same Animable path). Re-verify frametime on restored shadergroup.

r[cz.fast.scroll-10-in-300ms+1]

**Normative summary.** Sustain 10 zoom bumps within 300ms (applied ticks, not harness wall alone).

**Acceptance criteria.**
- [ ] STANDS. The scroll-debt accumulator (`inputs.rs`, with sign-reversal halving) exists at
  v0.0.9. Re-verify 10 applied bumps in 300ms.

r[cz.fast.shift-space-5bps+1]

**Normative summary.** Shift/Space zoom hold rate about 5 bumps per second (center origin).

**Acceptance criteria.**
- [ ] Re-verify bindings and hold rate on restored `inputs.rs`.

r[cz.fast.no-tick-backlog+1]

**Normative summary.** Fast spinning neither skips nor backlogs scroll ticks (debt gaps).

**Acceptance criteria.**
- [ ] STANDS. Scroll-debt with reverse-sign clearing is present at v0.0.9 (`inputs.rs`).
  Re-verify N thresholds ⇒ N zooms, no deferred burst.

r[cz.fast.input-next-frame-17ms+1]

**Normative summary.** Movements/zooms visible this or next frame (≤17ms at 60Hz).

**Acceptance criteria.**
- [ ] Re-verify on restored headgroup; v0.0.9's same-turn input apply is the design intent.

r[cz.system.memory-default-1gb+1]

**Normative summary.** Default memory limit is 1GB CPU + 1GB VRAM class.

**Acceptance criteria.**
- **SUSPENDED.** v0.0.9 has no memory-limit setting — the hoard is one screen package plus one
  previous package, so there is nothing to budget. The product rule (user-settable limit,
  floor from screen size) returns with any multi-package hoard; see collected-wisdom.

r[cz.cosmetic.bailout-range-2-255+1]

**Normative summary.** Bailout radius accepts at least [2, 255].

**Acceptance criteria.**
- [ ] Re-verify on restored settings/worker. v0.0.9 had animated bailout working (developer
  acceptance list), so the range clamp should exist; confirm the bounds.

r[cz.deep.min-zoom-pot-capacity+1]

**Normative summary.** Types/gears can represent magnification factor ≥ 2^3600000 (pot magnitude).

**Acceptance criteria.**
- [ ] STANDS as a capacity check. `IntExp` exists at v0.0.9 (`utils.rs`; locations are
  (IntExp, IntExp, pot) in `assemblies/structs.rs`). Re-verify representable range — no long
  zoom run needed.

r[cz.hoarding.one-answer-per-point+1]

**Normative summary.** There is one answer per point; cosmetic settings recolor from hoarded work.

**Acceptance criteria.**
- [ ] STANDS in its purest form: v0.0.9 has exactly one package with one slot per seat —
  competing answers are unrepresentable (virtues doc §7). Cosmetic recolor rides the same
  package. Re-verify.

r[cz.system.tile-manager-protect-current-lookahead+1]

**Normative summary.** The work store never prunes on-screen or lookahead work for memory; if protected work alone exceeds the limit, bump the limit.

**Acceptance criteria.**
- **SUSPENDED** (no tile manager, no pruning at v0.0.9). The protected-work principle is
  recorded in collected-wisdom for any future hoard.

r[cz.system.max-homotheties+1]

**Normative summary.** At most ~8 magnifications/homotheties remain in play at once.

**Acceptance criteria.**
- **SUSPENDED.** v0.0.9 keeps exactly one magnification in play (plus one hoard slot), which
  trivially satisfies the bound; the rule only bites when lookahead returns.

r[cz.ui.coords-parse+1]

**Normative summary.** Coordinate field accepts likely forms (space/comma/plus-i, parens,
brackets, imag-leading) and rejects invalid input without user confusion.

**Acceptance criteria.**
- Ported: [`coords.rs`](../../src/assemblies/headgroup/window/coords.rs) parse/readout/Apply; HUD coord bar; `SetPos` center semantics. Verify tags on coords tests.

r[cz.ui.coords-apply+1]

**Normative summary.** Apply is enabled whenever the field is valid (including when
already at that location); applying moves viewport center; field is not cleared.

**Acceptance criteria.**
- Ported with coords Apply enable rules and goto round-trip tests.

r[cz.ui.location-readout+1]

**Normative summary.** Read-only location field shows viewport center with copy;
coordinates entry/display always includes magnification.

**Acceptance criteria.**
- Ported: location HUD string includes `mag 2^N`, Copy button, center (not UL).

r[cz.ui.viewport-fill+1]

**Normative summary.** One viewport covers the entire window and resizes with it.

**Acceptance criteria.**
- [ ] Re-verify on restored headgroup (default res constant + resize path).

r[cz.cosmetic.layer-model+1]

**Normative summary.** Coloring supports normalize scales (log/reciprocal), colorize
functions (sin/modulo), ordered layers with per-layer color/opacity, and optional
highlights (in-filaments, out-filaments, nodes) in the script list.

**Acceptance criteria.**
- [ ] STANDS. v0.0.9 `settings.rs` has the full instruction set: PaintEscapeTime /
  PaintSmallTime / PaintSmallness / HighlightInFilaments / HighlightOutFilaments /
  HighlightNodes / HighlightSmallTimeEdges, with per-layer opacity, color, shading
  (Modular/Sinus), and Normalizing. (The tile era had *lost* most of these — the revert
  restores them.) Re-verify each kind paints.

r[cz.cosmetic.defaults+1]

**Normative summary.** Default cosmetics allow browsing: escape time, in-filaments
black, out-filaments as outside ∞-escape; may show other features subtly.

**Acceptance criteria.**
- [ ] Re-verify the default layer list on restored `settings.rs`.

r[cz.ctrl.drag-anchor+1]

**Normative summary.** User can zoom back to a particular point after starting a
mouse drag there (drag-anchor preserved across zoom-out then zoom-in).

**Acceptance criteria.**
- [ ] Re-verify on restored `inputs.rs` (drag handling exists; anchor semantics unverified).

r[cz.ctrl.hover-zoom-origin+1]

**Normative summary.** Except Shift/Space (center origin), zoom origin is mouse hover;
point under cursor stays fixed.

**Acceptance criteria.**
- [ ] Re-verify the complex-under-pointer invariant on restored `inputs.rs`.

r[cz.display.offscreen-arrows+1]

**Normative summary.** Red arrows appear when the set is mostly/fully off-screen or
almost/fully too small; zooming out / going off-screen is not disallowed.

**Acceptance criteria.**
- [ ] GAP. The classifier and arrow UI were tile-era; absent at v0.0.9. Product rule stands;
  see the foundations rule of the same geometry.

r[cz.tenacious.no-max-iter+1]

**Normative summary.** No max-iteration-count setting; points are iterated to
completion while still visible.

**Acceptance criteria.**
- [ ] STANDS. v0.0.9 has no max-iter knob; the worker iterates each seat to bailout or loop
  detection, resuming across bouts (virtues doc §4). Re-verify settings surface has no knob.

r[cz.hoarding.no-compute-settings+1]

**Normative summary.** No computation settings that force recompute of inside/outside
membership; cosmetics recolor from hoard only.

**Acceptance criteria.**
- [ ] STANDS. v0.0.9 settings are all cosmetic/animable; membership is never recomputed for a
  settings change. Re-verify.

r[cz.deep.snappy-at-depth+1]

**Normative summary.** At depth target, headgroup stays at full framerate; pan/zoom
execute at framerate while browsing the answer hoard.

**Acceptance criteria.**
- **SUSPENDED** until depth work lands. The standing part: browsing the hoard must never block
  the headgroup — v0.0.9's one-package remap is the reference behavior.

r[cz.calib.lowres-synthesis+1]

**Normative summary.** Interpolate/output low-res where appropriate; when older work
is disproven by newer, synthesize without discarding either.

**Acceptance criteria.**
- [ ] STANDS in golden form: v0.0.9's remap *is* the low-res synthesis — zoom-in magnifies old
  work honestly (nearest-neighbor), new work writes over it, and provisional edge answers fill
  the frontier without being marked delivered (virtues doc §5, §7). Re-verify visually.
