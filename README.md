![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

**Specification:** [SPEC.md](SPEC.md) — authoritative goals, input contract (each scroll tick = 2× zoom, zero perceived wait), architecture, algorithms, and roadmap.

**Goal:** Browse the Mandelbrot set very deeply at user-driven speed; completeness and speed are met together (see SPEC §1).

## How to use (linux)
- install rust (go to rustlang.org)
- install build essential:
- sudo apt install build-essential
- install m4:
- sudo apt install m4
- download the repo using:
  git clone https://github.com/JonathanTippy/Critical-Zoomer
- cd Critical-Zoomer
- cargo build
- cargo run

## Controls
- movement: mouse drag :smiley:
- adjust zoom: scroll :smiley:

## Scope
- the mandelbrot set, and tracking a given point and overlaying its path. (and maybe displaying a given point's julia set)
- The mandelbrot set is the only nontrivial fractal that isn't ugly.


## Da plan (subject to change)

### The MVP 0 (0.0.1)  done!
- blazing fast window exists | ✔️
- workday-based worker exists | ✔️
- window and worker communicate | ✔️
- ️no iteration count; all results are always calculated completely until escape or loop. | ✔️
  
### Fix the jank (0.0.2) done!
- window / worker linkakge is stable during quick inputs | ✔️
- fix drag after zoom | ✔️
- fix drag too far | ✔️
- window / worker handle resolution changes | ✔️
- fix zoom while drag | ✔️
- implement home button functionality | ✔️

### workgroup rewrite (0.0.3) done!

- worker no longer workday based, but workshift and checks its in channel every shift | ✔️
- rework worker resolution and updates; add work collector after worker so worker can send updates at a fixed rate | ✔️
- fix work cancellation | ✔️
- fix work skipping | ✔️
- rewrite WC / window linkage | ✔️
- fix zoom while drag | ✔️
- fix home button | ✔️
- fix screen resizing | ✔️

### work saving (0.0.4) done! (for now)

- add work collector actor | ✔️
- implement work saving when zooming in | ✔️
- implement CD | ✔️

### work saving 2 + flood fill (0.0.5) done!

- work saving when moving | ✔️
- flood fill out points | ✔️

### settings (0.0.6) done

- fix zoom too far | ✔️
- settings page allows custom layering of colorings | ✔️
- also allows animation of variables | ✔️

## Roadmap (named phases — detail in [SPEC.md](SPEC.md) §9)

| Phase | Status |
|-------|--------|
| Interactive foundation (0.0.1–0.0.6) | Done |
| Input-first zoom and cache | In progress |
| Steady-state messaging | Planned |
| Period detection | Planned |
| Graphics processor (hybrid B) | Planned |
| Classical perturbation | Planned |
| Series approximation | Planned |
| Cross-platform (Linux, Windows, Mac, web) | Planned |
| Lookahead prefetch (gaze) | Planned |
| Exploration (path tracking, Julia) | Planned |

Historical MVP checkmarks above remain accurate for 0.0.x shipped work.

## general optimization

- better actor comms (steady-state messaging phase)
- better knowledge leverage
- better movement


