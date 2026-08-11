# Interview: Project interview (continued) — naive GPU

- **Date:** 2026-08-11
- **Status:** in progress
- **Prior session:** [2026-08-11-series-approximation-contract.md](2026-08-11-series-approximation-contract.md)

**Summary:** Product-judgment interview. SA out of scope. GPU bar locked (exact
answer parity + ~160× ±20%, no soft ladder; don’t unwire yet). Oracle =
manual/test, doubling-until-stable + twin period. Topic switched to fullscreen /
Stec.

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
