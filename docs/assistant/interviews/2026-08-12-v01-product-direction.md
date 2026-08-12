# Interview: v0.1 product direction — depth quality + color cycling

- **Date:** 2026-08-12
- **Status:** in progress
- **Prior session:** [2026-08-12-precision-wall-gear-switching.md](2026-08-12-precision-wall-gear-switching.md)

**Summary (lock so far, not closed):** v0.1 is product-forward. Three
must-ships: (1) depth target with quality you would stand behind — headed
then oracle/DirectKernel pins; (2) cycling color as an extra shading choice
(working hypothesis: hue on normalized escape time; XaoS as research
target; default stays sinus); (3) 1080p must feel as good as current
default resolution. Window may be fullscreen/larger; **the whole compute
grid (workgroup stencil / answers / shade) caps at 1080p** and the window
upsamples. Open bugs do not own the calendar except when they make those
three untrue. False-“fixed” headed bugs and concurrent-window reverts are
process failures, not a reason to empty the stack first.

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
