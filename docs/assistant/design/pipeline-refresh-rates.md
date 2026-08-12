# Pipeline refresh rates (binding — 2026-08-11)

Status: **implemented** (cadence throughput pass). Charter note in
`issue-stack.md`. HUD stage rates use emission Instants (successful put only).

## Goal

Whole-view stages should feel like a **smooth, double-buffered, vsync-paced**
pipe. Actors **never block waiting** on the next stage: each holds an
**internal latest buffer** and **swaps** when newer input arrives.

## Two tiers

| Tier | Who | Cadence | Role |
|---|---|---|---|
| **Content / continuum** | Workgroup *publish* (collector → shade), entire shadergroup | **Real monitor vsync period** via Settings `auto_vsync_hz` (or manual Hz) | Video-like: bailout/color anims. |
| **UI / feel** | Headgroup present | **Vsync by default**; disable vsync → pace to `head_max_fps` | Game-like present; content tier stays on content period. |

Hardcoding “60 FPS” as the long-term content rate is rejected; 60 is only the
bootstrap until the head learns the display period.

### Head present (no spin)

Head paces with `request_repaint_after(period)` always — never bare
`request_repaint()`. Period is Automatic `auto_vsync_hz` when “pace to vsync
period” is enabled, else `1 / head_max_fps`.

**GL swap Wait is off** (`NativeOptions.vsync = false`). eframe glow applies one
`SwapInterval` to *every* viewport surface; with Wait, a deferred settings
window plus the root each block a full vblank → ~½ FPS (egui#5836 class). Timer
pacing aims at the monitor period without serializing presents. Settings
deferred viewport keeps its own `request_repaint_after(100ms)`.

### How `auto_vsync_hz` is learned (stable)

**Do not measure present frame times** into `auto_vsync_hz` — that jittered
content timers. **Do not create a second winit EventLoop** to probe the
monitor before `eframe::run_native` — that poisons the process with
`WinitEventLoop(RecreationAttempt)`.

The head aims at egui’s declared vsync period:

1. Each present: `Settings::resolve_auto_vsync_hz(ctx.predicted_dt, None)`
   (rounded to whole Hz).
2. egui’s `predicted_dt` is the integration’s “expected vsync period.”
3. eframe often leaves `predicted_dt` at the **1/60 placeholder**; Automatic
   then stays on that stable rate (manual Hz overrides). A future
   non-EventLoop monitor source may fill the optional probe argument.

Fan Settings to content actors only when the resolved Hz moves (≥2 Hz).

### Workgroup publish

Collector emits **whatever work is done so far** on **every** content beat —
not only when new WorkUpdates arrived, and not a promise of a complete frame
every interval. WorkUpdates still arrive densely from the worker; the collector
**absorbs all of them** into the resident package (never drain-to-newest on
worker→collector), then publishes on `resolved_content_period()`.

Only the **screen worker** parks when every seat is delivered. Collector and
shadergroup stay on the content continuum.

### Shadergroup

Escaper and colorer wake on the same content period, swap latest input
(drain-to-newest via `take_newest_plan`), always run the resident body, and
`try_send` (no actor-level skip-send). Compile-time ban: `build.rs` rejects
`dirty`/`clean` tokens in `src`/`benches`.

### Escape gear

**OG remains default.**

## Actor wake shape

Per-actor `wait_periodic(content_period)` raced with `wait_avail` for data
swap. Timer = production intent; channel = latest-resident swap.

## Headgroup

`NativeOptions.vsync` is **false** (DontWait) so deferred settings cannot
serialize GL Wait presents. Settings: “pace to vsync period” checkbox + max FPS
when uncapped. Stable `auto_vsync_hz` fans Settings to colorer, escaper, worker,
and collector.

## HUD

`fps:` / `pub:` / `esc:` / `col:` / `ctrl:` from emission Instants + rolling
RateCounters; `ips:` / `pps:` unchanged.

## Verify

- Content actors share head-reported vsync period (or manual Hz).
- Head: timer pace to vsync period by default; max FPS when uncapped; GL Wait off.
- Escape default stays OG.
- `auto_vsync_hz` does not track instantaneous present FPS.
