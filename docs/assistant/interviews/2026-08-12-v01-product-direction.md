# Interview: v0.1 product direction — depth quality + color cycling

- **Date:** 2026-08-12
- **Status:** in progress
- **Prior session:** [2026-08-12-precision-wall-gear-switching.md](2026-08-12-precision-wall-gear-switching.md)

**Summary (opening lock, not closed):** v0.1 attention is product-forward, not
bug-stack completion. Two must-ships named so far: (1) depth target with
outputs that are fully trusted and tested; (2) colorer gains a cycling-color
mode (the common algorithm) as an additional setting, without removing the
existing sign-based / sinus filters.

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
