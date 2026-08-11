# Interview: Project interview (continued) — naive GPU

- **Date:** 2026-08-11
- **Status:** in progress
- **Prior session:** [2026-08-11-series-approximation-contract.md](2026-08-11-series-approximation-contract.md)

**Summary:** Product-judgment interview. SA out of scope. GPU bar locked; Oracle
manual/test. Fullscreen/high-res: ~1 FPS feel past ~1.5× default pixels; headgroup
120 FPS healthy; animated-bailout points at shadergroup; banding/incomplete +
incorrect seats. Audit in progress — display-path cost + drop-on-behind suspects.

---

### Assistant

Archived prior interviews under `docs/assistant/interviews/`. Opening questions
about first-chunk / SA handoff — developer redirected.

### Developer

I'm letting the series approximation continue in another context, so we can
ignore that here. I'm aiming to start with product judgment like last time. The
main thing I want to talk about is the GPU implementation, because from what
I've seen, it never really hit the 160x target. And as you know, I care very
strongly about performance. And it just seems like we were pushing towards it
and we got to like 2x, is what I saw, which is like nothing. So clearly it's
just wrong, and something is completely wrong, because it should at least have
been at like, I don't know, 30x easily. The other thing is that there's some
quality issues with that code. It's outputting black splotches and phantom
Mandelbrot's, and it also seems to slow down as it goes. So I'm just really
concerned about quality.

### Assistant

Got it — SA parked; this session is GPU product judgment. Your read matches the
split the docs already admit: an iterate-heavy IPS probe once claimed ~160× on
this machine, while live home fill / PPS stayed near floor (~1–2× class). You’re
saying the *product* bar is the latter class — and ~2× is failure, not progress.
Quality defects (black splotches, phantom Mandelbrot, mid-fill slowdown) are
first-class, not “polish later.”

Follow-ups on metric, soft ladder, quality ranking, trust vs speed.

### Developer

Which number is 160, all of the above? I thought IPS was a good place to start
because it's likely easier than PPS. However, it might have been a little
misleading because if the code isn't wrong, then fixing IPS wouldn't necessarily
fix PPS. No, I'm not interested in soft ladders. I think it needs to hit 160
plus or minus 20% before it's considered valid. The black splotches and phantom
Mandelbrot seem to be maybe the same bug, they have a similar style. Obviously,
those are the untrustworthies because they result in wrong output. I don't know
if I understand what you mean by untrustworthy. Slowdown also matters a lot. I
mean, all three matter a lot. I'm really not interested in accepting any gear
that doesn't have visual parity with CPU naive, especially given how easy it
should be to test that. Which, yes, does imply that it should at least attempt
to compute periods. And when I say equality, I mean the entire answer, like the
answer struct, the answer struct should be exactly equal. And also, I don't know
if I should bring this up, but the full screen issue is a big deal. The app
should work at full screen. I think a big part of that is probably that I
decided to make the dumb stack queue. And that needs a focused... We didn't
discuss that because I need to decide how to actually design that. And I think
it worked better when it was a VEC. But at the same time, it's like a queue
going into a channel where I don't even know if it's necessary. It might just
be an extra queue for no reason. but now I'm just kind of going all over the
place and I'm not sure if the answers must be strictly equal they need to be
like I don't know I want there to be a standard that they're the same but yeah
I think they should be exactly equal I think they should be because otherwise
the answer is wrong and it's very clear that from the bottom up it's all
deterministic they need to be exactly equal that's actually the correct bar so
for the moment we can just compare it to the night the original naive
implementation I was thinking of making a Oracle gear I don't know whether that
got done but it would be like a gear that gives the correct answers for the
purpose of testing which would probably start by testing against a naive
implementation because that's the one that looks right

### Assistant

Locking what I heard (and parking the wander for a second):

**GPU validity bar (binding for this topic)**
- No soft ladder. Live path must hit **~160× ±20%** on the product metrics
  (IPS *and* PPS / fill — all of them). IPS-first was a tactic, not a substitute
  bar; a fast iterate loop that still crawls on completions is still invalid.
- Wrong output (black splotches / phantom Mandelbrot — likely one failure style)
  and mid-fill slowdown are all blockers. “Untrustworthy” = produces wrong
  answers; speed bugs are separate but also blockers.
- **No gear in the race without parity with CPU naive.** Periods included.
- Correctness standard: **`CompletedPoint` / answer payload bit-exact vs CPU
  `DirectKernel` (original naive)** on the same seats — deterministic bottom-up,
  so inequality means the GPU answer is wrong. Visual parity follows from that.
- Oracle gear idea: a test-facing “correct answers” path; not done as a named
  gear today. What exists is `DirectKernel` (production naive) plus some
  rug/perturb oracles for depth — not a full-frame Answer oracle gear for GPU.

**Fullscreen / Stec** — separate design interview. `Stec` is the fixed-cap LIFO
completion buffer (`completed_points`); work still feeds a channel to the
collector. You’re unsure whether that buffer is load-bearing or an extra queue
on top of the channel, and whether a plain `Vec` felt better. We should not
solve that inside the GPU correctness bar — but it’s on the stack as its own
topic because fullscreen must work.

Clarifying so we don’t lose the thread:

**8. Exact equality scope**  
Bit-exact vs `DirectKernel` on: **escape/repeat discriminant + period /
escape_time**, or **every field** (derivative, smallness, small_time, escape
location)? F32 GPU vs f64 CPU — do you require F64 GPU path for the equality
pin, or equality after a defined narrowing?

**9. Oracle shape**  
Is “Oracle gear” literally a selectable compute gear in the PPS race, or a
**test-only oracle** (run DirectKernel offline / in harness and assert
seat-by-seat equality)? Sounds like the latter for now — confirm.

**10. Topic queue for this chat**  
For the rest of *this* interview, stay on **GPU correctness + 160× validity**,
and park **fullscreen / Stec / channel** as the next session — or do you want
to switch to designing Stec now?

**11. One sentence product verdict**  
Finish this for me if it’s fair: “Naive GPU is not a valid gear until it matches
DirectKernel answers exactly and sustains ~160× ±20% without mid-fill collapse.”

---

Answer any; especially 10 if you want to change lanes.

### Developer

Yes, every field bit exact equality. It would be the Oracle would be a test only
gear. Never run in the app. Although honestly, you know, it should be an option,
but it shouldn't be selected by the automatic gearbox. It should be manually
selectable. And then I think I've explained this before, but the Oracle should
for each point computed, doubling the precision until the answer stops changing.
And then that's considered the correct answer. The Oracle should also have very
strong period determination. And that comes down to tracking the derivative and
doing a very thorough twin test. And then once the twin is determined correctly,
then it's a, that's the period. I think there's some trash talks about that or
something. Let's focus on full screen next, because I think it is more
important. Unless that's an accurate verdict. I'm not sure if I want to unwire
it because it is pretty cool how it's a bit faster sometimes, but yeah, I'm not
sure.

### Assistant

**Oracle (refined):** every-field bit-exact. Oracle = correctness path: primarily
tests, **manually selectable in the app**, never auto-picked by the PPS gearbox.
Mechanism you want: **precision doubling until the answer stops changing**, plus
**strong period via derivative + thorough twin-test** (trash `period_detector`
prose; decisions D-PER-*; `PERIOD_CONFIRMATION_ITERATIONS`). What exists today is
close but not that: FloatExp `OracleKernel` (test/bench, not auto) and a separate
rug **doubling** oracle for pert math — not yet one unified “double until stable
+ twin period” manual gear.

**GPU verdict (softened):** not “unwire now.” Validity still means exact
DirectKernel/Oracle parity and ~160× ±20% without mid-fill collapse — but while
it’s sometimes a bit faster and cool, you’re unsure whether to yank it from the
race. Leave wiring as a separate decision; don’t treat today’s soft “cool when
faster” as meeting the bar.

Switching topic → **fullscreen**.

---

**12. What breaks at fullscreen?**  
When you go fullscreen / large resolution, what’s the failure you care about:
**lag / backlog**, **stall / never finishes**, **missing / wrong pixels**,
**crash / hang**, or **UI feels dead**?

**13. Stec vs Vec vs channel**  
Suspects you named: fixed-cap `Stec` completion buffer, maybe better as growable
`Vec`; and “queue into a channel that might be redundant.” Which symptom made
you blame Stec — `BufferFull` / unfinished seats at large res, or general lag?

**14. Product requirement**  
Is “works at fullscreen” = **same never-stall + eventual fill as windowed**, or
also **same feel** (tick honesty, no accumulating display lag at 1080p+)?

**15. Design interview mode**  
For Stec: do you already lean “delete Stec, drain completions straight to the
collector channel,” or is the open question “keep a bounded buffer but size it
to the view”?

---

Answer any; we’ll stay on fullscreen until you say otherwise.

### Developer

So currently when I switch to full screen, the worker just stops outputting
anything until I switch back and then it works again. That's basically what
happens. I don't really like the stack thing because it requires a large stack
and didn't seem to actually yield huge improvements. But yeah, it feels
redundant to have a buffer and then a channel. Yes, same feel is necessary. And
yes, I think we should delete this deck and join directly to the collector
channel because that would ensure that we don't get nasty queue ups and getting
behind.

### Assistant

Locked for fullscreen / completion path:

- **Symptom:** enter fullscreen → worker stops outputting; leave fullscreen →
  outputs again.
- **Feel:** same never-stall / tick honesty as windowed — required, not optional.
- **Design lean:** **delete `Stec`**, publish completions **directly into the
  collector channel** (no intermediate fixed-cap stack). Reason: Stec needed a
  large preallocated stack for little gain, and buffer+channel is redundant and
  lets the worker get ahead of the display path.

(Note in tree today: `completed_points` is a ~100k-cap `Stec`; worker→collector
channel is capacity **50** messages. `BufferFull` on Stec already forces undeliver
+ shift break. Your lean replaces that local queue with channel backpressure.)

Follow-ups to pin the contract before any redesign:

**16. Backpressure when the channel is full**  
If the collector is behind: should the workshift **break early** (same spirit as
today’s `BufferFull` — seats stay undelivered, never-stall), or **block/wait** on
send? (Blocking would risk the hang you’re trying to avoid.)

**17. Batching**  
Direct-to-channel: one `WorkUpdate` per workshift (drain whatever finished this
shift), or smaller/more frequent sends? Any preference?

**18. The stop-on-fullscreen clue**  
When it freezes: does HUD still tick (IPS/mode change) with **zero completions**,
or does the whole worker look dead? And does leaving fullscreen “fix” only
because a new resize/Replace restarts work — or do buffered answers suddenly flush?

**19. Scope of “delete Stec”**  
Just the completion `Stec`, or are you also suspicious of other fixed buffers in
that path?

---

Especially 16 and 18 — those decide whether this is a backpressure redesign or
also a resize/pivot bug.

### Developer

So for the channel of full behavior, I'd like you to look into the steady state
lessons, but whichever one doesn't result in things getting behind, and you
wouldn't expect breaking early to result in that, but I can see how if it's
doing work and then trying to send it, and it just continues doing work but not
sending it, then there would be a lot of work that might simply get missed and
not sent would actually be the issue. So it wouldn't be behind in a way. If it
blocks on set, that one sounds more scary, but it may be right. And the problem
is that we're thinking about a scenario which really should never happen, and if
it is happening, then that is the issue. But it is worth handling it gracefully.
For batching, I want to lean very heavily on what we've found already works. So
aim to change as little as possible. So the fruising clue, you're not being very
clear there, but I can see your question, and it is possible that it's some
other part of the work group rather than the screen worker. That will just need
to be tested and profiled. So yeah, just read the steady state lessons, and then
we'll talk about this a bit more, and then probably we'll want to debug this one
step at a time.

### Assistant

**Steady-state lessons that bear on channel-full (read, not redesigned):**

1. **Architecture:** there is no light vs heavy load — only load. **Channel
   backpressure is a sign of incorrect code**, not normal stress
   (`docs/authoritative/architecture.md`).

2. **Small channels (10–50)** are a *promise* to consume toward the tip, not a
   resource knob. Growing them makes a landfill for stale frames
   (`workgroup-virtues` §2; `r[cz.craft.small-channels+1]`). Acceptance: senders
   **never block** on stale-consumer buildup; coalescing/drain handles overflow.
   Visible banding from completion-channel backpressure was a known product bug
   — fixed by draining, not bigger buffers.

3. **Neither drop nor get behind** on the display side: every unique published
   snapshot must be ingested (`r[cz.play.latest-wins+1]`). Inputs coalesce to
   newest; outputs must not vanish.

4. **Undeliver-on-full** (`r[cz.craft.undeliver-on-full+1]`): when the completion
   buffer can’t take the answer, seat stays **undelivered**, shift **breaks** —
   backpressure → re-queue, **never a dropped answer**. Your “do work then fail
   to send while continuing” fear is exactly the forbidden pattern: delivered
   (or discarded) without a successful publish. The golden handle is atomic
   push + undeliver if full.

5. **Virtues §12** already calls out Stec + the completion staging buffer as
   provisional: second queue in front of the `WorkUpdate` channel; keep only if
   batching/LIFO earn it. Your delete-Stec lean matches that honest 10%.

6. **Emergent cadence:** every non-empty shift sends; no artificial throttle
   (`r[cz.craft.emergent-cadence+1]`). Batching change ≈ keep per-shift send as
   today.

**Synthesis for Q16 (channel full):**  
Steady-state says channel-full **shouldn’t be a steady regime** — if it is, the
downstream actor is too slow / not draining (the real bug). Graceful handle that
matches existing law: **try_send; on full, undeliver affected seats and break the
shift** (same as today’s BufferFull) — do **not** mark delivered then keep
iterating, and do **not** block (blocking fights small-channels + never-stall /
constipation lessons). Blocking is the scarier option and contradicts
“senders never block.”

**Batching:** change as little as possible — keep one emergent send per
non-empty shift after deleting Stec.

**Fullscreen freeze:** open that it may be collector / escaper / colorer /
resize path, not only screen worker — diagnose step by step with tests +
profile before redesigning backpressure.

**20.** Does that reading match your steady-state intent — graceful =
undeliver-and-break on failed try_send, and treat persistent channel-full as a
downstream bug to find?

**21.** Ready to outline a debug ladder for “silent on fullscreen” (instrument
which actor stops first), or more judgment first?

### Developer

Oh, also, I just tested full screen again, and this time, it just kind of seems
to get sluggish and kind of behind. And then when the work is coming in, it's
just slow. The frame rate of the actual output from the work group is feeling
like around 1 FPS. And the other thing is that there's these bands of incomplete
points and what looks like incorrect points that are coming through, which
could be like channel overwhelm or something else. So the other thing is that
when I test whether this is a work group issue or a head group issue or a shaded
group issue, the head group is running at 120 FPS. It's perfectly healthy, very
good. The shaded group, when I animate the bailout, it goes about 1 FPS. So that
could be where the issue is. It could simply just be too slow. But then there's
also that banding issue where some work seems to be dropped. So honestly, I want
an audit into what's going on. And I just want to keep investigating and
discussing until we have a clear direction to go. and the line there is that at
about 1.5x default res number of pixels things start to go pear shaped

(Screenshots attached: jagged incomplete bands, vertical seams, black blocky
regions, phantom-smooth Mandelbrot patches cut by tiling/banding.)

### Assistant

**Audit sketch (discussion only — no redesign yet)**

**Your measurements that land**
- Headgroup ~120 FPS → window/UI not the bottleneck.
- Shadergroup ~1 FPS when animating bailout → matches “display path too slow.”
- Pear-shaped past **~1.5× default pixel count** (default `854×480` ≈ 410k →
  threshold ≈ **615k pixels**). Issue stack already suspected 1080p display-path
  lag (escaper full-frame + colorer multi-pass + clones); your 1.5× line is a
  sharper cliff than “only at 1080p.”
- Visuals: incomplete bands + incorrect-looking seats + black blocks +
  phantom-smooth patches — collected wisdom: *regular unfinished bands are a
  scheduling/channel symptom, not math*.

**What the code is doing (shadergroup)**

1. **Escaper re-walks the entire frame every ~8ms wake** whenever it holds a
   package — not only on new work. Per pixel: neighbor scans + optional extra
   bailout iterations; also `settings.clone()` **per pixel**. At ≥1.5× default
   pixels that alone can collapse to ~1 Hz class under animated bailout (wisdom
   target was **60 Hz at 1080p worst case** — clearly missed).

2. **Colorer likewise recolors the full frame whenever it holds values**, with
   multiple full-image layer passes + `output.clone()` on send.

3. **Both drain-to-newest by *dropping* intermediate full-frame packages** when
   `avail_units > 1`. That is input-coalesce style applied to the *display*
   path. Steady-state said display must not drop unique published snapshots —
   here intermediates are discarded when the shade path falls behind. That can
   look like “work dropped / banding” even when the worker is fine: you only
   ever paint the latest incomplete grid, and slowly.

4. **Workgroup side still in play for bands:** Stec `BufferFull` → undeliver;
   small completion channel (50); high completion rate at large res can leave
   geometric unfinished regions. Headgroup healthy doesn’t clear worker→collector
   congestion — it only clears UI.

**Two failure modes that can co-exist**
- **A. Shade path too slow (proven-ish by bailout animation @ 1 FPS):** O(pixels)
  every pulse → backlog → drop intermediates → ~1 FPS painted truth.
- **B. Publish/completeness integrity:** undelivered / Dummy / wrong seats showing
  as bands and “incorrect” points — may be worker backpressure, remap, or GPU
  quality; not yet isolated.

Deleting Stec helps **B’s queue-up theory** but does **not** fix **A**. If A
dominates, Stec deletion alone won’t restore fullscreen feel.

**Proposed investigation order (still discuss, then step debug)**
1. Confirm cliff: measure escaper+colorer wall time vs pixel count at 1.0× / 1.5× /
   2× default (bailout anim on/off).
2. Confirm drop behavior: log dropped package count when behind at 1.5×.
3. Only then decide: shade-path cost cut vs completion-path (Stec→channel) vs
   both.

**22.** Does the “two failure modes” split match what you’re seeing — slow paint
even when work exists (A) *plus* wrong/incomplete bands (B)?

**23.** For direction: prefer next step to be **timed audit of escaper/colorer vs
res**, or **headed isolation** (force DirectKernel, no GPU; bailout anim off) to
see if bands remain?
