# Salvage from tile-era code (LOWER TRUST — unvetted)

**Trust tier 2.** Rescued 2026-08-06 from the tile-era code: commits `e6a0560..7ff5ef5`
(branch tip `7ff5ef5`) plus the uncommitted `stash@{0}` ("tile-era code WIP before v0.0.9
revert"). Like `salvage-from-trash.md`, nothing here has been vetted against a running build,
and — importantly — **nothing here is ported yet**. This is a porting menu with evidence and
hook points, not a promise. Promote a feature by porting it and deleting its entry.

## Portable features, ranked

### 1. Location readout + goto panel — the big one

**What it does.** Top-center HUD bar with a read-only location field showing the viewport
center as `re ± imi  mag 2^N` (compact decimal, never the truncated `n...` IntExp Display)
with a Copy button, and a goto text field with live validation and an Apply button. Apply
accepts the HUD's own readout format verbatim, plus legacy `re, im`, `re im pot`, `a+bi`,
`(5i + 6)`, and the word `home` (restores HOME_POSITION framing). Apply emits SetZoom
*before* SetPos so a pasted location lands identically regardless of current magnification.

**Where.**
- `stash@{0}:src/assemblies/headgroup/window/coords.rs` (670 lines; 326 existed at `7ff5ef5`,
  the stash adds ~344 of fixes and tests). Key items: `f64_to_intexp` (line 9),
  `ul_for_center` (36), `viewport_center` (60), `format_intexp_readout` (79),
  `format_location_readout` (96), `parse_complex` (113), `commands_from_goto_line` (231),
  `split_mag_suffix`/`parse_mag_token` (287-310), `apply_button_enabled` (317), tests 322-EOF.
- UI wiring: `stash@{0}:src/assemblies/headgroup/window/mod.rs:696-750` (the `coord_bar`
  egui Area), plus the `coord_input` state field and the CZ_GOTOFILE harness hook
  (mod.rs:398-430).
- Transform side: `stash@{0}:src/assemblies/headgroup/window/transforms.rs:106-135` — SetPos
  center semantics via `ul_for_center`; stash also implements the `NavigateTo` arm with the
  same semantics (tests transforms.rs:352-406).

**Evidence it worked.** ~25 unit tests including exact roundtrips (paste HUD string →
validate → SetZoom-then-SetPos), pan/drag sign-convention tests, and a transform-level test
proving the same pasted line lands identically from mag 0 and mag 5. The DAT record
(`Trash/dat-2026-08-01.md`) lists precisely these gaps — magnification missing from the
location display, IntExp display wrong, no text oracle — as broken at DAT time; this work is
the direct fix, written after.

**Coupling.** Essentially zero — IntExp math, string parsing, egui. (One test constructs a
tile-flavored `SamplingContext`; the production functions only need `location` and
`screen_size`.)

**Porting difficulty: LOW-MEDIUM.** v0.0.9 hook points:
- `SamplingContext.location: ObjectivePosAndZoom` already exists
  (`src/assemblies/headgroup/window/sampling.rs:41`); `ZoomerCommand::SetZoom`/`SetPos`/
  `MoveTo` exist (sampling.rs:16-34) — but the `SetPos` match arm is an **empty stub**
  (sampling.rs:162-163), so the port must bring the tile-era SetPos implementation with it.
- IntExp lives at `src/utils.rs:49` at v0.0.9; the tile era moved it to `crate::intexp` —
  imports need rewriting.
- Extend the HUD debug block at `src/assemblies/headgroup/window/mod.rs:380-440`; v0.0.9 has
  no coord bar today.
- v0.0.9 has no `NavigateTo` command variant — add it or drop `commands_from_navigate_line`.

### 2. TPS counter → PPS analog — trivial core

**What it does.** HUD shows `fps:X / tps:Y / 1s low: Z`, where TPS is a rolling 1-second
count of newly completed whole tiles ingested — deliberately *not* the WIP emit rate (that
was an earlier bug, and there is a regression test for it). At v0.0.9 the analog is PPS:
points per second.

**Where.** `stash@{0}:src/assemblies/headgroup/window/rolling.rs:157-191` (`TpsCounter`:
`VecDeque<Instant>` with `record`/`prune`/`tps`), tests at rolling.rs:193-231. Wiring:
`window/mod.rs:97-98` (state field), 384-386 (record on completion), 554-555 (HUD format).
The "what counts as a completion" gate (`sampling.rs:222`) is tile-coupled — leave it behind.

**Evidence it worked.** Four unit tests including the WIP-inflation regression; live TPS was
a tracked standard (`cz.perf.home-100tps`).

**Porting difficulty: TRIVIAL counter, one real decision — the PPS event definition.**
v0.0.9's `rolling.rs` already exists (`rolling_frame_calc`); the tile-era file is a clean
superset. Natural feed: `update_sampling_context`, called from
`src/assemblies/headgroup/window/mod.rs:308-314` — record the incoming `View`'s point count
per packet. Extend the HUD string at `mod.rs:408` with `pps:`.

### 3. Coloring-layer add/remove + settings UI fixes

**What it does.** Settings window gains "Add layer" (all seven v0.0.9 layer kinds) and
"Remove selected" (keeps ≥1 layer); `ColoringInstruction::template(kind, id)` factory;
`Settings::add_coloring_layer` / `remove_selected_coloring_layer` /
`ensure_coloring_script`. Plus two genuine UI bug fixes: the animation-period slider
previously reused the bailout-radius limits (now clamped 0.25..=120 s with an "s" suffix), a
slider for `bailout_max_additional_iterations`, and a settings-change-detection fix so
animated-bailout changes actually propagate (compare `.animated` and `.range`, not just
`.value`).

**Where.** `stash@{0}:src/settings.rs` diff (methods ~85-125, `template` factory ~330-408,
tests ~791-953); UI in `stash@{0}:src/assemblies/headgroup/window/widgetize.rs` (add/remove
menu ~74-104, period-slider fix ~282-285).

**Evidence it worked.** Six new unit tests (add all seven kinds → 10 layers; remove keeps ≥1;
unknown kind rejected without consuming an id; GPU opcode-packing parity).

**Coupling.** None — pure egui over the same `ColoringInstruction` enum v0.0.9 already has.
v0.0.9's `widgetize.rs:22-44` is literally the pre-feature version of the same list.

**Porting difficulty: LOW.** The stash diff applies almost verbatim to v0.0.9's 549-line
`src/settings.rs`.

### 4. Zoom-debt input feel

**What it does.** Smooth fast zooming: scroll deltas accumulate into "debt" and emit zoom
pots continuously (10 zooms in 300 ms max rate, no tick backlog); Shift/Space held gives ~5
bumps/s; scroll-up-zooms-in sign convention; opposite-scroll resets debt toward the new sign.

**Where.** `7ff5ef5:src/assemblies/headgroup/window/inputs.rs` — `consume_scroll_debt`,
`consume_key_zoom_debt`, `scroll_step_to_zoom_pot` (pure functions, unit-tested in-file).
Skip the `mag_velocity` EWMA helpers (`update_mag_velocity_ewma`, `mag_velocity_mode`) — they
feed the tile scheduler's foveation mode, and v0.0.9 has no scheduler to consume them.

**Evidence it worked.** In-file unit tests.

**Porting difficulty: LOW-MEDIUM.** v0.0.9's `inputs.rs` (214 lines) is the same file
pre-rewrite; drop the pure functions in, then wire `parse_inputs` scroll/key deltas through
them instead of per-event bumps.

## Excluded — tile-coupled, do not port

- **GPU display/shade work** (`stash@{0}:window/gpu_display/`: rank-insertion fix,
  sampling.wgsl NORES-skip in `load_raw`, the 274-line `shade_tests.rs` GPU-oracle parity
  suite) — entirely about tile grids, homothety-tower ranks, and atlas cell slots. Nothing
  maps to v0.0.9's single-screen CPU sampling path. Sole salvageable idea: the *shader-oracle
  parity test pattern*, if v0.0.9 ever grows a GPU shade pass.
- **`tile_session.rs` / `tile_session_tests.rs`** (+472/+213 in stash) and the `gpu_tps_tax`
  counters — tile machinery, dead by revert.

## Suggested port order

1. Location readout + goto panel (biggest user-visible win; also un-stubs `SetPos`).
2. PPS counter (a day's work; reuses existing `rolling.rs`).
3. Settings layer add/remove + the two widgetize fixes (self-contained, tested).
4. Zoom-debt input feel (polish).
