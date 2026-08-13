# Pipeline refresh rates (binding — 2026-08-11)

Status: **split (ghost-hunt 2026-08-12).** Content-tier cadence (collector /
escaper / colorer `wait_periodic`, absorb-all publish, HUD emission rates)
landed. **Head present pacing did not stay landed.** `351afdf` (“preferred
vsync code”) restored bare `ctx.request_repaint()` every `update`. Do not
read this file as “window CPU / vsync spin is fixed.” Charter notes in
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

### Head present (open — 100% CPU)

**Intent (not live):** `request_repaint_after(period)` always — never bare
`request_repaint()`. With broken/absent GL vsync, bare repaint spun at hundreds
of FPS and pins the window actor at 100% CPU.

**Live (2026-08-12):** `window/mod.rs` calls bare `ctx.request_repaint()`
(`351afdf`). `NativeOptions.vsync` is **true**. Settings fields
`head_vsync_enabled` / `head_max_fps` exist; **settings UI for them was
removed** and does not pace present. Developer still sees ~100% window CPU
at “vsync rates.” Profiling the **worker** park after fill is the wrong
actor. **Not fixed.**

`NativeOptions.vsync` stays **on** (GL swap Wait). Turning it off to dodge
deferred-settings dual-Wait caused uncapped presents (~1500 FPS HUD) and a
stencil/attention storm that kept the screen worker unparked — reverted.
Deferred settings still uses `request_repaint_after(100ms)` on its own viewport;
settings dual-Wait FPS coupling remains an open egui/glow issue to solve without
disabling root Wait.

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
shadergroup stay on the content continuum. Park waits stay on the warm input
set (Replace, attention, settings, references, slow heartbeat) — do not silence
that pipe. The park predicate must be **O(1)** (`seats_need_work`); scanning
every seat (~410k) on each wake after fill was burning ~30% worker CPU with
zero workshifts (`CZ_PROFILE_CPU` settle profile 2026-08-11).

Post-fill settle must be checked over ~10 s wall time
(`steady_state_home_stays_parked_for_10s_after_fill`). Shade/colorer/collector
staying busy after fill is continuum cost on a full resident package — not a
channel-storm by itself.

### Shadergroup

Escaper and colorer wake on the same content period, swap latest input
(drain-to-newest via `take_newest_plan`), always run the resident body, and
`try_send` (no actor-level skip-send). Compile-time ban: `build.rs` rejects
`dirty`/`clean` tokens in `src`/`benches`.

### Escape gear

**Escape path remains OG default.** Colorer default is GPU (not this section).

## Actor wake shape

Per-actor `wait_periodic(content_period)` raced with `wait_avail` for data
swap. Timer = production intent; channel = latest-resident swap.

## Headgroup

`NativeOptions.vsync` defaults on. Stable `auto_vsync_hz` fans Settings to
colorer, escaper, worker, and collector. Head vsync/max-FPS **widgets are
not in the settings UI**; fields remain on `Settings`. Present loop is
still immediate `request_repaint` (see Head present).

## HUD

`fps:` / `pub:` / `esc:` / `col:` / `ctrl:` from emission Instants + rolling
RateCounters; `ips:` / `pps:` unchanged.

**`ctrl:` coupling (2026-08-12):** controller Replace stamps only reach the
window when a collector publish carries `controller_emitted_at`. Low `ctrl:`
under motion usually tracks dense publish/remap lag, not a fat Replace payload
(Replace is stencil-only). Events whose Instant is older than the RateCounter’s
1 s window age out to 0 at paint. Detail: `collector-publish-bottleneck.md`.

## Verify

- Content actors share head-reported vsync period (or manual Hz).
- Head: `NativeOptions.vsync` on; **present still immediate-repaint**; window
  CPU open (`issue-stack.md`).
- Escape default stays OG. Colorer default is GPU (not this section).
- `auto_vsync_hz` does not track instantaneous present FPS.
- Idle home: screen **worker** load drops once seats are delivered (O(1)
  `seats_need_work`). That does **not** imply the window thread is idle.
