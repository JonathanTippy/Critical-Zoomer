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

### How `auto_vsync_hz` is learned (stable)

**Do not measure present frame times** into `auto_vsync_hz` — that jittered
content timers. The head aims at egui’s declared vsync period:

1. Each present: `Settings::resolve_auto_vsync_hz(ctx.predicted_dt, probed)`.
2. egui’s `predicted_dt` is the integration’s “expected vsync period.”
3. eframe still often leaves `predicted_dt` at the **1/60 placeholder**; when
   that is all egui reports, the head uses a **one-shot winit monitor probe**
   (`ActiveEventLoop` + `refresh_rate_millihertz`) before `run_native`.

Fan Settings to content actors only when the resolved Hz moves (≥0.5 Hz).

### Workgroup publish

Collector emits **whatever work is done so far** on the content beat — not a
promise of a complete frame every interval. WorkUpdates still arrive densely
from the worker; the collector swaps them into the resident package and
publishes on `resolved_content_period()`.

### Shadergroup

Escaper and colorer wake on the same content period, swap latest input, always
run the resident body, and `try_send` (no actor-level dirty skip-send).

### Escape gear

**OG remains default.**

## Actor wake shape

Per-actor `wait_periodic(content_period)` raced with `wait_avail` for data
swap. Timer = production intent; channel = latest-resident swap.

## Headgroup

`NativeOptions.vsync` defaults on. Settings: vsync checkbox + max FPS when
uncapped. Stable `auto_vsync_hz` fans Settings to colorer, escaper, worker, and
collector.

## HUD

`fps:` / `pub:` / `esc:` / `col:` / `ctrl:` from emission Instants + rolling
RateCounters; `ips:` / `pps:` unchanged.

## Verify

- Content actors share head-reported vsync period (or manual Hz).
- Head: vsync default; max FPS + disable vsync paces present.
- Escape default stays OG.
- `auto_vsync_hz` does not track instantaneous present FPS.
