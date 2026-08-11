# Interview: Actor layout, frame passing, and pipeline cadence

- **Date:** 2026-08-11 (afternoon)
- **Status:** in progress — interview mode; do not forget parked items
- **Related design lock:** [pipeline-refresh-rates.md](../design/pipeline-refresh-rates.md)
- **Prior session:** [2026-08-11-project-interview-continued.md](2026-08-11-project-interview-continued.md)

**Summary (living):** Developer accepted GPU color look → **GPU color is
default**. Escaper **stays OG** (GPU escape currently slower). Cadence revised:
**two tiers** — content (workgroup publish + shadergroup) at **real vsync from
head/egui** (not hardcoded 60); head vsync-or-max-FPS. Prefer **per-actor
timers** + channel data swaps. **No code** until a later big plan; keep
discussing design.

---

## Parking lot (must not forget)

Carry-forward from this afternoon’s design talk + shade pass. Strike when done.

| Item | Status | Where |
|---|---|---|
| GPU color default | **done** | settings `resolved_color_gear` → Gpu |
| Escape gear default | **locked OG** (GPU escape slower for now) | settings / escape gear |
| Two-tier cadence (content=vsync from head; head=vsync\|maxFPS) | **doc’d, not implemented** | `pipeline-refresh-rates.md` |
| Hardcoded 60 for shade | **rejected** — use egui/head vsync rate | interview |
| Workgroup publish at vsync (partial packages OK) | **locked on paper** | was ~20 Hz; revised |
| Headgroup uncapped today | **bug vs lock** | window/mod.rs |
| Headgroup max-FPS + vsync toggle + **broadcast vsync Hz** | not started | settings + window |
| Per-actor timers at vsync period (not channel-paced) | **preferred** | escaper/colorer/collector |
| Shade wake ~8 ms → vsync period | not started | escaper/colorer |
| Content floor smell ≲15 Hz (worse ≲10) | product bar | headed / HUD |
| Resident latest-buffer + swap | partial; formalize with cadence | shade + window |
| Workgroup TTFP / ≥1.5× | **parked** | issue-stack |
| Dual wgpu / stops-on-GPU-color | **parked** | issue-stack |
| Precision wall ~2^41 | accepted | prior interview |
| Auto gearbox never picks shade GPU | still law | virtues |
| **Big plan** of all undiscussed work | **wait** — no code yet | developer |

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

### Developer (answers to Q1–Q6)

I think I misspoke when I said that the work that the shader group could
default to 60fps. It probably should just have correct communication from the
head group of what the Vsync rate is from egui, because that just should be
the same. It would be a shoddy work I could quick fix to accept just running
at 60 FPS. The other thing is that I actually think the workgroup should emit
at that same VSync rate. Just because there's not really a reason for it not
to, and it's going to be slightly more enjoyable if it does. So yeah, kind of
two tiered rates defaulting to everything being VSync, but then the head group
can be uncapped or set with a limit. Yeah, that seems reasonable. One,
probably we should lean towards individual timers inside the actors rather
than based on channels, although I am really curious what the steady state
lessons have to say on that. I want to stay away from that. I think one of
the great benefits of steady state is that it allows later actors to do well
even if earlier actors don't. So like you could have a bad bailout animation
rate but good escape time coloring animation rate if that makes sense. So
like the coloring results could run well when something else isn't running
well. But this is kind of a moot point because it should be that they just
all run at the same rate. That's just kind of the graceful case but, I don't
know, three. The work group, as I just said, should probably just run at
Vsync and publishing is a weird thing because it's just publishing all of the
work that is done so far. Obviously it's not going to necessarily have all of
the work done every 50 milliseconds. That'd be great. But it's not part of
the design at the moment for, yes, the shader group stays at 60. It stays at
whatever Vsync is because animations are more like, so videos, you can watch
a video and it can be 24 FPS and it's fine. But you can play a video game and
it needs to be 120 for you to feel good. That's the difference. That's why
the head group needs to be allowed to go higher and the rest of it doesn't
really matter as long as it's Vsync. And honestly it's not even a big deal if
it's like 20 because it remains responsive and that's the goal. The problem
is when it gets down to like 10 and below that range. Maybe more like when it
gets below 15 or something. Five, escape here. Leave OG as the default
because for some reason the GPU one is slower. Six. I just want to wait on
code changes for a bit. When I do want to do changes, I'll probably ask for a
big plan that includes everything that I've discussed that hasn't yet been
handled, which will be a very large chunk of work. But I still want to hold
off on that and just discuss the design more.

### Assistant

Recorded and rewrote `pipeline-refresh-rates.md` to match. **No code.**

**Answers locked:**

| Q | Lock |
|---|---|
| Q1 | Not channel-paced. **Per-actor timers** at the **head-reported vsync period**. Hardcoded 60 rejected. |
| Q2 | (implicit) keep independent actors so a later stage can still look good if an earlier one is sick; ideal = same vsync. Still open: strict one-beat pipeline vs two timers same Hz — see Q7. |
| Q3 | Workgroup **publish at vsync**; payload = **work done so far**, not full-frame-every-tick. |
| Q4 | Shade/content stay on **vsync**; do **not** chase uncapped head. |
| Q5 | Escape **OG default** (GPU escape slower today). |
| Q6 | **No code** until you ask for one big plan of remaining discussion items. |

**Steady-state on timers vs channels:** pull-reactor + single-wake-up treat
**timers and channels as peers** in `await_for_any!`. Cadence via
`wait_periodic(vsync_period)` plus channel avail for data swap is idiomatic.
Channel-*only* pacing would starve anim when no new package arrives and would
couple a sick upstream to a silent downstream — which fights the “later actors
can still do well” benefit you like. So your timer preference aligns with the
stack lessons; we’re not inventing an anti-pattern.

Video vs game metaphor is the clearest product cut: content = video (vsync is
enough; ~20 OK; ≲15 smells); head = game (may want 120).

---

### Follow-up interview questions

**Q7 — Shade coupling.** With two timers at the same vsync Hz, should escaper
and colorer stay **fully independent** (each paints/escapes from its resident
buffer every beat — colorer may paint a slightly older values frame), or
should colorer **prefer** values produced this beat when available (soft
same-beat, no hard barrier)?

**Q8 — How vsync Hz is communicated.** Settings broadcast already fans out from
the window. Prefer:

- **(A)** head writes `vsync_hz` / `frame_period` into `Settings` each present
  (or on change), shade/workgroup read it for their `wait_periodic`, or
- **(B)** a tiny dedicated “cadence” channel / signal, or
- **(C)** something else?

**Q9 — Uncapped head and content.** When head is uncapped at 144, content
timers still use **monitor vsync period** (e.g. 144 Hz panel → ~6.9 ms), or
always the **panel’s vsync** even if present is faster than vsync? (Usually
“vsync rate” means the display’s refresh; uncapped present can exceed it.)

**Q10 — Below-floor behavior.** When shade cannot hold ≥~15 Hz at the current
res, is the product response **drop: + fix cost** (current law), a **resolution
/ quality brake**, or just “headed notice” for now?

**Q11 — What else for the eventual big plan?** Besides cadence + head
vsync/max-FPS + vsync broadcast, call out any undiscussed items you want on
that plan now (TTFP, dual wgpu, escape GPU perf, etc.) so the parking lot stays
complete.

---

### Assistant notes (for continuity)

- **No pacing/code implementation** until developer requests the big plan.
- Dual wgpu remains parked product risk.
- Interview file + `pipeline-refresh-rates.md` are the memory; keep appending.
