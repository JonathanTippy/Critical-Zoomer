# Interview: Series approximation performance contract

- **Date:** 2026-08-11
- **Source chat:** [Series approximation performance contract](a4fe99bc-5707-4d65-8ae1-10fbb4b39642)
- **Chat id:** `a4fe99bc-5707-4d65-8ae1-10fbb4b39642`

**Summary:** Project interview that drilled into series approximation. Prior live sketch deferred for membership bugs. Developer contract: SA always-on as seat init (not bout work), nearly free probe, coeffs fused one step per reference iterate; prior sketch rejected. Session closed with docs updated; next was to plan first implementation chunk.

---

### Developer

Interview me about the project

### Assistant

I'll interview you about **Critical Zoomer** — not a quiz on the docs, but what you care about and how you think about it.

Start wherever you want; short answers are fine. We can go deep on anything that lights up.

---

**1. Origin**  
What made you start this? Was it “I want a fast Mandelbrot zoom,” something about Rust/concurrency, or something else entirely?

**2. What “done right” means**  
When the app feels *correct* to you — not just “pretty” or “fast” — what are you noticing? (Interaction feel, fidelity, scheduling honesty, something else?)

**3. The hard-won part**  
You’ve put a lot of weight on what v0.0.9 proved. What’s the one mechanism or behavior you’d be most upset to see accidentally redesigned away?

---

Pick one to answer first, answer all three, or push back on the framing — your call.

### Developer

It was mostly UX because I found that every app I tried was clunky to use and I just didn't really like using them. And I really wanted to explore deeper in the Mandelbrot set without having to deal with bad UX. In terms of what I notice when I interact with it, it's mostly the feel and whether it feels like it's actually smooth and hitting its tick rates. And obviously the rate at which it completes the work is a big deal. And I want to push that forward, but to some degree it is as difficult as it is. But I still think there's a lot more we can get out of perturbation, especially by adding series approximation in a way that makes sense. The thing that I'd be most upset to see accidentally redesigned away is the time slicing of the scheduling and the work shift pattern where the work is done in small pieces, which don't even have the opportunity to hang and be too long in stall. Something that really concerns me about the series approximation is that I don't know if it was implemented in a way that made sense when we tried it because it didn't seem to go very quickly. And these things are extremely sensitive to things like whether you're using the stack, whether you're using slow operations such as division, and whether you're doing too many things in a loop. So I think of my hand-coded V9 code as a strong example of a good example that demonstrates those good performance practices.

### Assistant

That lines up: UX first, then feel (smooth ticks + completion rate), with work-shift time slicing as the non-negotiable, and perturbation/SA as the next performance bet — but only if it’s coded like your V9 habits, not like a “correct but slow” first try.

A few follow-ups:

**4. Clunky, concretely**  
When other Mandelbrot apps felt bad, what were the top one or two concrete UX failures? (Lag after zoom, sticky drag, blank frames, UI fighting you, something else?)

**5. Tick vs complete**  
When those two fight — “keep the tick honest / never stall” vs “finish the frame sooner” — which do you protect first, and have you ever regretted that choice?

**6. Series approximation that “makes sense”**  
If a future SA attempt *did* make sense, what would you expect to see that the last try didn’t? (E.g. fewer ops in the hot path, no division in the loop, stack-friendly layout, still fitting inside short work shifts — or something else?)

Answer any of those, or steer us somewhere else (V9 coding habits, perturbation today, etc.).

### Developer

There are two categories of UX failures that were pretty common. There was the pattern itself, which was the click and drag pattern, which I think is perfectly valid, but I just don't want to use it. I want to move around very quickly like a video game and not be stopped by any slowness or computation. So the idea is that I want to be able to go around and move around freely, regardless of what computation is going on, and that there's never chugging, there's never, oh, you increased the iteration count so high that now the app is frozen, and there's never... Like the app has so much work to do that it's getting overwhelmed and the frame rate is decreasing, that sort of thing. Yeah, tick versus complete. I think my hand code is the prime example of how you don't have to choose between a solid tick rate and really good throughput. And when it seems like those two are fighting, they're actually not, and it's just an excuse for shoddy work. But I do value the tick rate higher if I had to choose one. But again, I don't think I have to. The series approximation, I didn't see the code. So I'd appreciate an audit of what actually happened and whether it held up with the quality that I expect.

### Assistant

Free navigation without chugging is the UX bar; tick and throughput shouldn’t be a forced tradeoff. I’ll audit the series-approximation attempt against your V9 performance habits next.

### Assistant

Pulling the old live wiring and comparing it to the V9-style hot path.

### Assistant

## Series approximation audit

**Short verdict:** The live attempt was a correctness-first sketch that was later pulled for membership bugs. Against your V9 performance bar, it would not have been expected to feel fast — and `safe_skip` in particular could fight the never-stall / work-shift model.

### What actually happened

1. **Wired live** around the gear-ladder checkpoints (`PublishedReference.series`, `apply_series_skip` on seat start in both f64 and FloatExp pert kernels).
2. **Deferred** the same day the deep membership pins were fighting (false “in” / blocky δc). Production path no longer builds or applies series; dormant module is `src/series.rs`, with tests that production kernels do **not** call `apply_series_skip`.
3. Deferral reason in the docs is **correctness under relative δc**, not “we measured SA and it was slow.” So you never got a clean “SA vs no-SA” win after the math was trusted.

### Does it match your V9 quality bar?

**Mostly no** on the hot path. Math shape is recognizable (Martin / Heiland-Allen prefix skip); engineering is not V9-grade.

| Habit you care about | What the attempt did |
|---|---|
| Stack / tight layout | Coeffs as `Vec<Vec<ComplexFloatExp>>` — heap row-per-iterate, clone-heavy build |
| Avoid slow ops in the loop | Skip path is all `ComplexFloatExp` / `FloatExp` (even from the f64 kernel). No lean f64 series path |
| Don’t do too much in a loop | `safe_skip` is the problem |
| Small interruptible pieces | `safe_skip` runs as one chunk at seat start, not under a bout cap |

**`safe_skip` cost (why it wouldn’t feel quick):** for each candidate `n` up to orbit length it fully `evaluate`s the series (O(order) complex FloatExp ops), then another O(order) tail probe — **O(orbit_len × order) per seat**, with **no reuse** across `n` and **no binary search**. If the orbit is long and the useful skip is small, you can spend more finding the skip than iterating those steps. That alone is enough to explain “series was on and it didn’t go faster.”

**Other notes:** order was modest (2 / 4 / 8 by orbit length). Skip never invents a final answer (good). Eval itself isn’t division-heavy. Derivative `dd` was not series-advanced (fine for “simple series,” but incomplete vs a serious deep path).

### Bottom line

Your suspicion is right: it was **not** implemented in a way that had to win on speed. Correctness pressure then yanked it before anyone owed a V9-style rewrite (flat coeffs, f64 where legal, incremental or binary skip search, bout-bounded skip work, measure PPS with/without).

---

**Interview continue**

**7.** When you picture SA done “like V9,” is the win mainly **deep zoom** (long orbits, big skips), or do you also expect it to help **home / mid** where skips are short?

**8.** For never-stall: should skip discovery itself be bout-capped (partial progress across shifts), or is “skip must be so cheap it’s free at seat start” the real requirement?

### Developer

Yes, the wind is mainly deep zoom. However, I think of it as something that shouldn't affect performance very much in cases where it doesn't yield a huge improvement. So there wouldn't be much of a point to doing a branch there. And it should just always run. Yes, skip must be so cheap that it's free. Definitely. 100%. That's like a binary search through the back of the orbit. It's like literally nothing if it's done well.

### Assistant

That’s a clear bar: always-on SA, free skip probe (binary search on the orbit), big win only when depth gives one — no “maybe enable series” branch.

So the contract is roughly:

- **Always run** — no mode / heuristic gate that costs more than the skip itself  
- **Probe is O(log N) evals**, not a linear walk of the orbit  
- **If the skip is tiny, you barely notice** — overhead must stay in the noise next to one seat start  
- **Deep zoom** is where the saved iterations pay for everything  

**9.** On the always-on point: when you’re at home / shallow and the probe returns skip≈1, is “free” measured as *wall time vs a seat with series disabled*, or as *doesn’t steal budget from the workshift’s iterate bouts* (or both)?

**10.** Separate from skip: building coeffs on the reference — is that allowed to be heavier (background reference worker), as long as seat-start skip stays free?

### Developer

Basically both, yes, it's extremely important that it doesn't affect the amount of time that a single seat takes very much. It is expected that it will impact it a tiny bit. That's kind of unavoidable, but not very much. Especially in these easy cases, it'll be like a single access per point, I think. And even in harder cases, again, binary search, it's like almost nothing. And so it doesn't even make any sense for series approximation to steal budget from the iterate bouts. I don't even consider how that would work, I think. The series approximation is part of the initialization of the point. Which does bring up a question, because does series approximation, does switching the gear require the points to be restarted? I don't think it does. Honestly, it's already been working really well. But this is kind of a tangent. In terms of actually building the coefficients on the reference, this should be done as the reference is done. And it will I don't know if it will make the reference computation slower, but it will be a part of it. I mean, obviously, it'll make it slower to some degree. But it shouldn't be like a big O change, it'll just be a little bit more math to do at the same time. So I want it like rolled together, it wouldn't be a separate process, because that would just not make sense. And it would be extra complicated. but again when you ask is it allowed to be heavier to the degree that it absolutely must yes but absolutely no more than that I want airtight performance mindfulness from the start with all of this

### Assistant

Clear constraints. Checking the gear-restart tangent against the current code, then one more question.

### Assistant

Locked in: SA is **init**, always-on, probe nearly free (binary search / ~one access when skip is useless), **not** a bout-budget thief; coeffs **rolled into** reference iteration with no extra big-O and no separate pipeline — airtight from the start.

**Gear tangent — your instinct matches the code.**  
Seat restart is keyed off **reference generation** (`delta.generation != published generation`, or no delta yet), not gear. Gear **promotes mid-orbit** (f64 → scaled → FloatExp) and keeps going. So switching gear does **not** force a series re-init / full restart. Restart is “new reference (or glitch / zero-bind path),” which is when init — and thus SA — would run again.

---

**11.** When coeffs are rolled into the reference loop: do you want them advanced **one series step per reference iterate** (same workshift cadence as the orbit), or is “finish orbit, then a tight coeff pass over stored Zₙ” still acceptable if it’s still O(N·order) and fused in the same worker?

**12.** Zooming out from SA for a moment: what’s the next interview topic you care about most — PPS / kernel selection, never-stall feel, depth gears, or something else?

### Developer

The Yes, one series step per reference iterate. Next interview topic. Yeah, it does feel like we've kind of reached a point. Let's call it here. Go ahead and make the appropriate documentation updates. Um, And then we'll plan the first big chunk of work.

### Assistant

Interview closed. Docs now carry the SA contract.

**Recorded in**
- Series rule — performance + always-on + init/free probe + fused coeffs; prior sketch rejected
- Depth design — product intent + prior-attempt autopsy + re-enable gates
- Issue stack — deferred gap updated; “next = plan first chunk”
- Decisions — `D-SERIES-2`…`6`
- Collected wisdom — short binding note
- `src/series.rs` header — points at the target, not the dormant sketch

Checkpoint: `automatic checkpoint 2026-08-11 10:50:27 -0500`. Tracey CLI isn’t installed here; rule ↔ impl/verify markers still resolve by search.

Whenever you’re ready, we can plan the first implementation chunk against that contract.

### Developer

draft a plan

### Assistant

Researching the reference path and seat init so the first SA chunk plan stays concrete.

### Assistant

Have enough context: first chunk is a performance-minded rewrite plus live re-enable under the documented contract. Drafting that plan.

### Developer

Series approximation — first implementation chunk

Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself.

To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.

