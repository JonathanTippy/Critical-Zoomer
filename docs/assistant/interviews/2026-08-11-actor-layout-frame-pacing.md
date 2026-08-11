# Interview: Actor layout, frame passing, and pipeline cadence

- **Date:** 2026-08-11 (afternoon)
- **Status:** in progress — interview mode; do not forget parked items
- **Related design lock:** [pipeline-refresh-rates.md](../design/pipeline-refresh-rates.md)
- **Prior session:** [2026-08-11-project-interview-continued.md](2026-08-11-project-interview-continued.md)

**Summary (living):** Developer accepted GPU color look → **GPU color is now
default**. Escaper stays OG default. Three refresh rates locked on paper
(workgroup out ~20 Hz, shade vsync/~60, head vsync + optional max FPS). Need a
deeper interview on **actor layout × frame passing × throughput** so pacing
work does not thrash the shade unjam or get forgotten among other threads.

---

## Parking lot (must not forget)

Carry-forward from this afternoon’s design talk + shade pass. Strike when done.

| Item | Status | Where |
|---|---|---|
| GPU color default | **done** (this session) | settings `resolved_color_gear` → Gpu |
| GPU escape default? | **not asked** — still OG | settings / escape gear |
| Three refresh rates design lock | **doc’d, not implemented** | `pipeline-refresh-rates.md` |
| Headgroup uncapped today (`VSYNC=false` + every-frame `request_repaint`) | **bug vs lock** | window/mod.rs |
| Headgroup max-FPS typed field + vsync toggle | not started | settings + window |
| Shadergroup wake ~8 ms → vsync period (~16.7 ms) | not started | escaper/colorer |
| Workgroup collector publish ~20 Hz | not started (collector `max_sleep` 50 ms today) | work_collector |
| Resident latest-buffer + swap (never block) | partial (coalesce/try_send); formalize | shade + window |
| Shade pace from headgroup frame tick vs timer | open interview Q | below |
| Workgroup TTFP / update-rate at ≥1.5× / 1080p | **parked** after shade unjam | issue-stack |
| Workgroup “stops when GPU color” / dual wgpu contention | **parked** | issue-stack / dual device |
| Precision wall ~2^41 absolute naive black | accepted as wall, not HUD lie | prior interview |
| Auto gearbox must never pick shade GPU | still law | virtues |

---

### Developer

I'm happy with the new color. You can switch that to the default. I don't know.
I'm kind of trying to do a bunch of things and I don't want anything to get
forgotten, but I'm not sure. I think we need to discuss the actor layout as it
relates to frame passing and throughput more. So it's switched into more of an
interview style and just make sure that nothing we're talking about here gets
forgotten.

### Assistant

GPU color → default (manual still forces OG/GPU). Escape left at OG default
until you say otherwise.

Interview mode on **actor layout / frame passing / throughput**. Everything
from the refresh-rate talk is in the parking lot above and in
`pipeline-refresh-rates.md` so it cannot fall out of the chat.

**How it works today (facts, not proposal):**

- Escaper & colorer wake on `wait_periodic(8ms)` **or** channel avail — so they
  often re-escape/recolor ~125 Hz while holding a package (bailout anim).
- Window: `VSYNC=false` + `request_repaint()` every frame → uncapped head.
- Collector: `wait_periodic(50ms)` (~20 Hz-ish already in spirit).
- Shade channels are small; full → coalesce drop newest-kept (`drop:` HUD).
- Actors already keep a resident `state.values` / screen and swap on take —
  close to your “internal buffer, swap on new” rule; the missing piece is
  **cadence alignment**, not inventing buffers from scratch.

**Your lock, restated:**

1. Never block an actor waiting on the next stage — resident + swap.
2. Shade path one consistent rate ≈ vsync (slowest shade member sets the
   honest smooth rate).
3. Workgroup *output* ~20 Hz; shade vsync; head vsync-or-max-FPS.
4. Faster-than-next-stage only creates pressure; better to sync.

---

### Interview questions (please answer in any order)

**Q1 — Shade drive signal.** For the first implementation of shade vsync
pacing, which do you want?

- **(A) Timer-only:** escaper/colorer `wait_periodic(1/60)` (or shared shade
  period), independent of the window’s present.
- **(B) Head-driven:** headgroup emits a frame tick (or stencil) each present;
  shade wakes on that tick (+ still swaps if a package arrives early).
- **(C) Hybrid:** timer at 60 as floor, but also wake on head tick when
  uncapped head is faster — only if you care about shade tracking uncapped UI.

**Q2 — One shade beat or two.** Escaper and colorer today are separate actors
with their own 8 ms wakes. Under vsync, should they:

- stay **two actors** but share one period (escaper produces values, colorer
  paints, both on the same Hz), or
- be treated as **one shade stage** for pacing purposes (colorer only runs when
  escaper finished this beat — stricter pipeline, lower parallelism)?

**Q3 — What “20 Hz workgroup output” means exactly.** Is that:

- collector → escaper publish rate only, or
- also a soft target for “a full home frame should refresh answers ~20×/s”
  (harder — couples to workshift fill time)?

**Q4 — Uncapped head vs shade.** When the user sets max FPS 144 and disables
vsync, should shade **stay at 60**, or attempt to match the head max (and thus
do more escape/color work)?

**Q5 — Escape gear default.** Color is now GPU by default. Leave escape on OG
until headed bailout-anim proves GPU escape, or switch escape default to GPU
in the same spirit?

**Q6 — Implementation order among the parked pile.** Of the non-forgotten list,
what should be the *next* code change after GPU-color-default: restore head
vsync + max-FPS UI, retarget shade wakes, collector 20 Hz polish, or the parked
workgroup TTFP diagnosis?

---

### Assistant notes (for continuity)

- Do not implement pacing until Q1–Q4 are answered enough to avoid thrash.
- GPU color default is a small, reversible settings/docs change; parity tests
  still pin OG↔GPU equality.
- Dual wgpu (shade vs naive_gpu) remains a parked product risk when both run.
