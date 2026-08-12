# Interview: v0.1 product direction — depth quality + color cycling

- **Date:** 2026-08-12
- **Status:** in progress
- **Prior session:** [2026-08-12-precision-wall-gear-switching.md](2026-08-12-precision-wall-gear-switching.md)

**Summary (lock so far, not closed):** v0.1 is product-forward. Three
must-ships: (1) depth target with quality you would stand behind — headed
then oracle/DirectKernel pins; (2) cycling color as an extra shading choice
(working hypothesis: hue on normalized escape time; XaoS as research
target; default stays sinus); (3) 1080p must **feel** as good as current
default resolution — not by capping the window or decoupling head view
from window pixels. **Pause new GPU compute/escape grind** — CPU is
enough for v0.1. One thing at a time. Delivery gap: capture the *layers*
of “works” so assistant checks match what the developer sees; assistant
may say “looks better” as a candidate only.
Layer locks: whole answer; worker finishes entire context (redo OK);
gear fail = admit-none, false-shallow, **or post-admit precision drop**;
shade is a pipe; head view = window pixels (**do not cap the window**);
1080p is a feel target; post-v0.0.9 AI comments are ghost candidates.
Head 100% CPU: `351afdf` restored bare `request_repaint`; not fixed.

---

### Developer

Okay, product direction for release v0.1. There are a lot of bugs and open
issues. I'm not really sure how to deal with them, but I know that I want to
focus most of my attention on moving the product forward towards the end goal.
Two things I can think of off the top of my head which must be in v0.1 are the
depth target must be reached with completely quality outputs that are fully
trusted and tested, and the colorer needs an additional setting which does the
cycling color algorithm, which is the common algorithm. When I was implementing
it initially, I got a little excited about not doing coloration for the sake of
like accuracy, and I still really do like the filters that I created which were
sign-based, and I don't want to get rid of them, but I should really also
implement the color cycling which is kind of the standard for it to be an app
that's taken seriously. Interview.

### Assistant

Locking what I heard:

**Attention policy.** Open bugs stay listed; they do not automatically own the
calendar. v0.1 work is judged by progress toward the end goal, not by emptying
the stack. Items that block the two must-ships get pulled in; the rest stay
parked unless they make the depth picture untrustworthy.

**Must-ship 1 — depth with trusted quality.** Not “types can represent the
target magnification.” The product bar is: you can actually get there, and what
you see is quality you would stand behind — fully trusted and tested, not a
hero screenshot with known lies (false interior, rectangular precision blocks,
truncated references, Dummy black).

**Must-ship 2 — cycling color, additive.** Keep Sinus / Modular (the
sign-based / accuracy-minded filters). Add the common cycling algorithm as
another shading choice so a serious viewer recognizes the app. Default script
can stay the current look until you say otherwise.

**Not claimed yet.** What else is in or out of v0.1 (fullscreen feel, GPU
multipliers, vsync park, certified boundary, collector publish rewrite). How
“cycling” is defined (hue wheel vs discrete palette vs modulo of a user
palette). What “fully trusted” means as a test bar (oracle equality vs headed
DAT vs both).

Follow-ups below in chat.

---

### Developer

Oh, yes, of course, there is one other thing. The app must, at minimum, work
smoothly and feel really good at 1080p, just like it does at standard
resolution now. And it's acceptable if 4K, for that, it could just have like
a... So the resolution of the texture can differ from that of the window. So
it could just max out at 1080p, I think would be reasonable for the moment.
And we'll see what happens, because it could be that if we design it properly,
4K will just fall out kind of for free. I don't really have a 4K monitor, but
I could manage to simulate it just by like expanding the window more. But I
don't want to get too much creep. But then again, yeah, I do think that being
able to run in full screen is like pretty important. Okay, to answer your
first question, it's C. Both. I don't know if you can say neither ships
without the other. One has to come before the other, I would assume. That's
kind of the way it might be tautological. For your second question, yes, it's
Hue cycle on normalized escape time, I think. I don't know. I need more
research into what the standard is there. I feel like it might differ. A
really good example is the app called Chaos spelled with an X, I think. But I
might tweak it to make it look better because I honestly think it does get
very messy and noisy. The default look, yes, the default look should stay the
sinus coloring method. For your last question, yes, there is a third must
ship as I just said. But I don't know. I'm not sure. The problem with the
bugs is that it's just really frustrating because it feels like whatever I'm
doing is not super productive and mostly it's coming down to like I test
something and then the assistant tries to fix it and says it's fixed and says
that it had a test that caught it and it fixed it and then I look and there's
literally no difference and it's not fixed at all. So that's just really
frustrating and forward progress on these bugs has been excruciatingly slow
and I don't know if I don't really know what the issue is. And maybe I'm just
expecting too much, trying to go too fast. I did have three windows open at
the same time. So, I suppose that could be the issue. With like work
reverting, is that something that can happen? Because I did see that in the
chat context a lot. I would say that it like lost some work.

### Assistant

Locking this turn:

**Must-ship 3 — 1080p feel, fullscreen allowed.** Default-res smoothness is
the bar at 1920×1080. Window size and compute/color texture may differ;
texture may cap at 1080p so a larger or fullscreen window upsamples rather
than computing 4K. 4K as a compute target is creep; if the split is designed
cleanly it may fall out later. Fullscreen itself is in.

**Trust order.** Headed quality and oracle/DirectKernel both required for
depth. Sequential: one can land first; the second is still required before
calling the must-ship done. Not “ship one and skip the other.”

**Cycling.** Working hypothesis: hue on normalized escape time. Research
target is XaoS (Chaos spelled with X). Expect to tweak because stock cycling
gets messy/noisy. Default script stays sinus. Not locked: XaoS also has a
separate *palette cycling* (animate the palette over time) vs mapping
iteration to hue — those are different features.

**Bugs vs speed.** The false-fixed pattern (test green, headed unchanged) is
the real pain, not “too many tickets.” Three concurrent windows on one tree
can revert or lose work; that is a known failure mode. One live editor at a
time is the recovery rule. Open bugs still do not automatically jump the
three must-ships unless they make depth/1080p/color untrue.

---

### Assistant (study turn — prior interviews)

Read the earlier sessions before asking again. What was already locked and
should not be re-asked:

**Origin / UX (2026-08-11 series interview).** The app exists because other
zoomers were clunky. Wanted: video-game free roam, never frozen by iteration
count, never chugging because the app is overwhelmed. Feel = smooth ticks +
completion rate. Tick vs throughput is not a forced tradeoff; if they fight
it is shoddy work; if forced, protect the tick. Non-negotiable: small
interruptible work shifts.

**Fullscreen / 1080p feel (2026-08-11 GPU/fullscreen interview).** Already
said fullscreen is a bigger deal than GPU 160× validity. Symptom evolved
from “worker silent until leave fullscreen” to sluggish ~1 FPS workgroup
output + incomplete/incorrect bands past ~1.5× default pixels. Diagnosis
locked: headgroup healthy (~120 FPS); shadergroup too slow; workgroup
dropping work (bands). **Same feel as windowed is required.** Stec deleted;
channel-full → wait for collector (not undeliver Finals). Colorer GPU only
as an honest rewrite: feature parity, no simplifications, tests on every
behavior, manual OG↔GPU gear. GPU color later accepted as default. Escape
stays OG until shipping is actually fixed in the live app.

**Cadence (2026-08-11 actor layout).** Content (publish + shade) at real
head/egui vsync; head may uncap. ~20 FPS content is OK (video); ≲15 smells;
≲10 is the failure. Head is the game. Live HUD rates beat Criterion. Do not
declare shade/GPU “fixed” until headed `esc:`/`col:` say so.

**Depth / trust (feasibility + precision-wall).** Continuous browse without
cost showing through is the central feasibility claim. Refs: greedy keep,
never discard for glitch. Transition rectangular blockiness / gear kaput
were **not** declared fixed after the 10-bit admit slider. Naive black
wrong-interior is a periodicity tangent. Oracle: every-field bit-exact;
test-primary; manually selectable; never auto gearbox; precision-doubling
until stable + strong period.

**This session’s three must-ships** sit on that, they do not replace it.
1080p feel was already a product bar; the new relief is **texture may cap at
1080p** so a larger/fullscreen window need not compute 4K. Depth-trust was
already “headed is truth; tests pin; don’t say fixed until you look.”
Cycling is the new cosmetic must-ship.

**Inferred, not re-asked:** headed confirmation before calling a bug fixed
(standing law). 1080p “feels good” = same never-stall / tick honesty / fill
as default res, with content allowed to be vsync-class (~20 OK, ≲15 smell) —
not “HUD rates identical to 854×480.” GPU 160× is a validity bar for that
gear, not named as a v0.1 must-ship here.

**Still actually open (not repeating):** whether the 1080p cap is the
**workgroup stencil** (answers computed at ≤1080p) or only the **color
texture**; upsample of a 1080p texture into a larger window; order of the
three must-ships if you want to name one; XaoS hue-map vs palette-spin
(you asked for research, not a pick).

---

### Developer

yup

### Assistant

**Locked:** the 1080p cap is the **whole compute grid** — workgroup answers
at most 1080p — then upsample into a larger or fullscreen window. Not a
color-texture-only cap.

Parked (research, not a pick): XaoS hue-map vs palette-spin. Order of the
three must-ships unnamed; any of them can start when you say go.

---

### Developer

I'd like to continue the interview focusing on the delivery strategy as of yet
and what's gone wrong and what's the plan for how to do it better.

### Assistant

Delivery-strategy turn. Not re-asking the three must-ships.

**What already went wrong (from prior interviews + quality review — correct
me if the story is wrong):**

1. **False-fixed.** Assistant lands a pin, says the test caught it, you look,
   no difference. Dual-device, admit-margin slider, dummy-head ~60 Hz escape
   are the type: mechanism in tree, **product not verified**. Dummy-head and
   Criterion are not the window.

2. **Wrong bar for the symptom.** Headed bugs got unit tests aimed at a
   nearby function. Green ≠ the band / block / rate you see.

3. **Concurrent editors.** Three windows on one tree revert and lose work.
   Chat then argues about code that is gone.

4. **Grind over product.** PPS loops and gear work absorbed attention while
   fullscreen feel, depth lies, and cosmetics you actually want for v0.1
   moved slowly. Quality-slip review already named this: feature grind while
   the baseline feel was assumed untouched.

5. **Soft-skip history** (DAT): lowering the bar to look green. Doctrine
   forbids it; the cousin failure now is **claiming green on the wrong
   check**.

6. **“Fixed” in the issue stack** before you said so. Standing correction
   exists (precision wall, headed escape) but it keeps happening.

**Candidate plan (proposal, not lock):** one live agent; one must-ship slice
at a time; RCA from *your* headed observation; change; you look; only then
pin tests that encode that observation; never write “fixed” until you say
it. Loops only after the headed failure is named. Xvfb/PNG is a first pass
for still frames, not a substitute for feel/rates/depth browsing.

---

### Developer

is mostly correct, but I do feel as if there's a lot of the other things as
well. I don't want to ban the assistant saying it looks better, but there are
clearly some considerations that affect whether it works or not that we're
not properly capturing, and I want those to be fully understood and
documented so that the assistant can act in a way that makes sense and
matches with what I see, and that might be a complicated thing to connect
together because there are a few layers that it could be at, but that's what
I want to track down. One question, yes, basically we can pause GPU work.
It's really a great optimization, but I think the app can work on CPU and
work decently well, probably well enough for 0.1. I want to get more focused
in and do one thing at a time so that we can make sure that we do that thing
well.

### Assistant

**Locked this turn**

- Diagnosis stands, **plus** the other leaks: too many slices, too much
  talk, loops on the wrong bar. Not only false-fixed.
- Assistant **may** say “looks better.” That is a candidate read, not a
  product close. The gap is missing **considerations** — not a speech ban.
- **Pause GPU work for v0.1.** CPU naive/pert is the ship path; GPU is a
  later optimization. Colorer GPU already default may stay as landed
  cosmetics unless you say to force OG; **new** GPU compute/escape grind
  stops.
- **One thing at a time**, done well.

**The thing to track down:** layers between what you see and what an
assistant check can honestly claim. Proposal to argue with (not lock):

| Layer | What you actually notice | What we often pin instead |
|---|---|---|
| 1 Answers | membership / period / escape look right | DirectKernel field equality on a tiny stencil |
| 2 Publish / remap | bands, dummy black, unfinished sold as done | “channel send succeeded” / percent_completed |
| 3 Gear / type | blocky transitions, HUD gear vs picture | admit-margin unit test, view_gear floor pin |
| 4 Shade | color/filaments/bailout anim | Color32 vs OG on a fixture |
| 5 Present | fullscreen, 1080p feel, upsample | dummy-head RateCounters |
| 6 Tick / roam | never chug, pan/zoom at head rate | workshift never-stall micro |

False-fixed = we greened a **lower or adjacent** layer than the one you
were looking at. Dummy-head vs window is 5 vs 6. Admit slider vs blockiness
is 3’s mechanism vs 3’s picture. Dual-device vs `esc:` is 5’s lab vs 5’s
headed HUD.

---

### Developer

Okay, let's go through these one-by-one. For one, answers, membership, period,
escape. I assume you're talking about the definition of the answer. If the
assistant has been treating the answer as if the only part of it that matters
is the Mandelbrot result inside, that is wrong. The entire answer matters
because it is what results in the rendered outputs. Number two, this one is
quite difficult because you're talking about browsing around and then watching
the work group go through several cycles of mixed stencils and work packages
where it's remapping and combining new work. Now that I think about it,
actually, no. It's a pretty simple issue because the issue is, the truth is
that the work, the screen worker, and maybe I wasn't clear enough about this,
but the screen worker must always complete its entire work context regardless
of what is thought to be already in the remapped published work if that makes
sense. Its job is simply to complete its entire context regardless of if it's
redoing work that was already done. And we could talk about prioritizing the
work a tiny bit better, but it was designed this way because the waste is not
catastrophic and it makes the design simple. Gear slash type number three, so
the blocky transitions are caused by two possible issues. One is that the C
generator simply admitted no gear and so the worker stopped. That should be
pretty simple to detect. The other one is that the C generator admitted a gear
which it should not have admitted and it resulted in these rectangular blocky
looking, like the entire screen just looks rectangularly low res, like
rectangular pixels. And so those are kind of the two failure cases for that.
For shading, I don't feel like that's a real candidate. The cool thing is the
shader group is basically just a pipe, like what goes in comes out. There's
literally no way it could change anything. And so it's really helpful to
basically just ignore it in terms of what's actually going on in the behavior
and the answers. That's a big part of what makes the assembly design so cool
is that the business parts of the app are only the work group and the head
group, really. So that could end up being an issue if we decouple the window
resolution with the head group view resolution. So I do actually feel pretty
strongly that the head group view resolution should be the same as the head
group window resolution. Now, the problem with this is that it's going to be
quite hard to test because as far as I know, there are no tests yet which test
this kind of multi-step browsing where you've gone to various places. And
there are, there's testing. So The main way to test, I don't feel like that's
actually a possible issue because kind of either it works or it doesn't. This
is part, this is why the project leans on the steady state philosophy that
it's like a shader pipeline and everything kind of stays the same and it's why
there's a lot of what looks like waste but must not be optimized away because
that's what keeps it steady and keeps it flat. Same stuff goes through, it
just happens to be different data. And so in reality, I don't foresee those
becoming an issue a lot. There is also the cases where I see something and I
want there to be a test that goes to that location and test that location for
the behavior of the worker usually. And the problem is I want those tests to
be accurate to the default resolution would be but I also want them to be fast
so I want them to be like a cut down resolution. But there's a tension there
because the cut down resolution is way faster. There's also another thing
which is that the, what resolution you choose actually affects the scale
because the scale is coupled with the pixels per power of two constant. And
that is central to the design, I'm not changing that. It's imperfect but we
just need to handle the powers of two correctly and then it won't be a
problem. The standard underlying, like the standard that other apps use which
I wouldn't be unhappy about the app using as a kind of way of reasoning about
this, like documentation reasoning about this to decide how to use the powers
of two code which I'm not changing. The way to think about this is that you
have your location obviously but then you have the width of the side of the
screen. And so that would be how you could think of a view in terms of what
actually needs to be tested because that's what would look the same even if
it's a different resolution. And then the side of the screen is defined as, I
usually think of that as being like the smaller side so if it's not square
then it would be whichever side is smaller. So, for what said fixed, no
difference. The head group was said fixed in terms of the cadence as it
relates to its CPU usage. For some reason, the head group is spinning at 100%
CPU usage, even when it's running at v-sync rates, which should be like almost
no CPU. Now, I honestly have no idea why this is happening, but for some
reason, profiling seems to have failed to discover it, or there was some
breakdown when we were trying that. It's all in Git history, so I would
appreciate a second look to see what actually happened. Anyway, yes, it's
highly important that we get this comprehensive theory and understanding
applied, and we chase down all of those ghosts and misleading documentation or
comments, because those are like a cancer in the code base, and they're just
going to ruin it. Anything that's been added by an AI agent after v0.0.9 is a
candidate for that.

### Assistant

**Layer corrections (locked)**

1. **Answers.** The whole `CompletedPoint` / answer payload matters — not
   membership alone. Shade paints from the entire answer; a “Mandelbrot
   result only” pin is the wrong bar.

2. **Publish / remap.** Not a multi-cycle mixed-stencil mystery. **Screen
   worker completes its entire work context** even if remap already published
   those seats. Redo is allowed; waste is not catastrophic; simplicity is the
   point. Do not “optimize away” skip-if-already-in-remap. Prioritize a
   little later if needed.

3. **Gear / type — two failure shapes only.** (A) C-generator admits
   **nothing** → worker stops (should be easy to detect). (B) C-generator
   **false-admits** a too-shallow gear → whole screen rectangular low-res
   pixels. Those are the blocky-transition cases.

4. **Shade — drop as a mismatch candidate.** Shadergroup is a pipe: what
   goes in comes out. Behavior/answers live in **workgroup + headgroup**.
   Ignore shade when hunting “why doesn’t it match what I see.”

5. **Head view = window pixels.** Strongly: headgroup view resolution
   **equals** window resolution. Decoupling those is the present-path
   footgun. Compatible with the earlier 1080p **compute-grid** cap only if
   the cap is workgroup (answers), and head still presents 1:1 with the
   window (upsample in the head, not a smaller head view). Flag if that
   reading is wrong.

6. **Steady-state / waste.** Same pipeline, different data. Looks like
   waste; must not be optimized away. Multi-step browse “either works or
   doesn’t” under that philosophy — don’t invent a special mixed-stencil
   test theory.

**How to pin a location you saw**

- Test the **worker** at that location.
- View identity for docs/reasoning (other-app standard, not a code change):
  **center + width of the smaller screen side.** Same view at another
  resolution.
- `PIXELS_PER_UNIT` / power-of-two coupling **stays**. Cut-down test
  resolution is faster but changes scale; handle POTs correctly rather than
  decoupling pixels from zoom. Tension named, not solved this turn.

**Ghosts.** Post-v0.0.9 AI comments/docs that disagree with live code or
with you are cancer. Chase them. Candidate for every assistant-added note.

**Second look: head 100% CPU “said fixed”**

Live code still does **bare** `ctx.request_repaint()` every `update`
(`window/mod.rs`). Docs (`pipeline-refresh-rates.md`) still say **never**
do that — it spins hundreds of FPS and pins the window thread when GL
vsync Wait is absent/broken.

Git: cadence work used `request_repaint_after(period)`. Commit
`351afdf` **“preferred vsync code”** (you) replaced that with
`request_repaint()` and the comment “Let Egui handle vsync.” Issue-stack
then **shelved** 100% CPU: worker parks after fill; vsync/max-FPS **UI**
did not change behavior; “revisit later.” Profiling that showed worker
park at 0 after fill was looking at the **wrong actor**. The window
thread still asks for an immediate repaint every frame. `VSYNC=true` on
the GL swap is supposed to Wait; if you still see 100% CPU at “vsync
rates,” either Wait is not actually blocking, or `update` is doing
full-frame work (e.g. a new `sampler_buffer` every pass) at a high Poll
rate. **Not declared fixed.** Docs that say present pacing is implemented
are a ghost relative to `351afdf`.

---

### Developer

Okay, one additional failure mode that could cause a block because I just
realized is that maybe the C generator is admitting it as it should and then
somehow after that point it's failing due to other precision issues. I could
see that happening if the code is written in a really retarded way basically.
It could also happen in perturbation I suppose, again, if the code is written
in a really retarded way. Oh, yes, perfectly reasonable to cap the window at
1080p, I think. I mean, it might be a little bit mean to people with 4K
monitors, I'm going to be honest. I feel so strongly that the design should
not be compromised here that I can't really recommend it. But I should be
more detailed about what I mean when I say the code is written in a retarded
way. I'm drawing a blank. Honestly, it's pretty straightforward in my head.
But I should put it down. How could this be written wrong? If the naive,
there are two branches here. You could have the naive or the perturbed.
Naive could be written wrong. Actually, no, I don't know if I want to split
them up. The mechanism that I'm thinking of where this is written wrong is
that there's some layer in between the C generator and the actual worker that
is dropping precision because it's basing in some certain precision level,
like F64, for example. That was a big problem early on that there was, I
think, an F64 layer somewhere that was just forcing the precision wall. It
suffices to say that if any code after the C generator uses insufficient
precision, that could also yield a rectangular-looking low-res look.

### Assistant

**Gear / type — three failure shapes (was two)**

(A) C-generator admits nothing → worker stops (detectable).
(B) C-generator false-admits a too-shallow gear → rectangular low-res.
(C) **C-generator admits correctly, then a later layer drops precision**
(e.g. an f64 interlayer between admit and the iterate) → same rectangular
look. Naive or pert; do not split the theory. Early post-v0.0.9 already
had an f64 layer forcing the wall. Live suspect, not a claim: worker
channel still described as carrying f64 completions while compute gears
go FloatExp — that is exactly (C) if those completions are the values
that get reused/remapped as if they still had depth.

**Window vs 1080p (supersedes “cap the window”)**

You will not recommend capping the window. Head view **equals** window
pixels; that design is not compromised to be nice to the 1080p feel bar
or mean to 4K. 1080p remains the **feel target** (must work well there).
A hard window max is a product cop-out, not the architecture. The earlier
“compute grid caps at 1080p / upsample” idea is in conflict with
head-view=window if the window is larger — parked until you pick a
non-compromised way to keep 1080p feel without shrinking the view.
