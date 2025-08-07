![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust


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
- window and worker communicate | ✔
- ️no iteration count; all results are always calculated completely until escape or loop.
  
### Fix the jank (0.0.2) done!
- window / worker linkakge is stable during quick inputs | ✔️
- fix drag after zoom | ✔️
- fix drag too far | ✔️
- window / worker handle resolution changes | ✔️
- fix zoom while drag | ✔️
- implement home button functionality | ✔️

### workgroup rewrite (0.0.3) WIP...

- worker no longer workday based, but workshift and checks its in channel every shift | ✔️
- rework worker resolution and updates; add work collector after worker so worker can send updates at a fixed rate | ✔️
- fix all jank | ...

### Dynamic Res & filament detection (0.0.x)

- ARVs will at first be generated using just one point each (and in fact fewer than that) (and its neighbors for filament detection), and progressively add more points.

- colorer now recieves 'area representative values' which can be in (period), out (escape time), or edge ((standard, in filament, or out filament), (period, escape time, ratio)).
  standard edges are just where the sample points don't all agree on in or out.
  out filaments can be sometimes detected by differing periodicity (except cardioid corners);
  in filaments can be sometimes detected by differing escape time derivatives.
  Because of how unreliable this will be, it should remain an opt-in option to color filaments differently until it becomes reliable.

- can we have perfect rendering early through filament highlighting?
  Perhaps, however providing the option of progressively improving detail through data combination
  (average or other methods can be used) may result in more pleasing and correct results, so the option should be available.
  Depending on memory available, this additional work may need to be largely discarded after completion. For now, the work will be discarded.

### Memory (0.0.x)

- worker remembers just-completed work when moving or zooming

### Quantified ARV Certainty (my favorite part) (0.0.x)

- ARVs will contain an uncertainty score as well, which can be used in coloring.
- The score is calculated by how much a doubling of res changes the resultant ARV values. The more change, the more uncertain.
- edge points will require sub-filament detection to catch narrow structures
- flat in-areas will just be large blocks
- after no change, the pixel can be considered completed.
- Likely, the coloring step will add subtle noise if uncertainty is present.

### Settings (0.0.x)

- settings for coloring
- controls settings
- mixmap settings
- bailout settings
- colors and bailout can be animated with different values
- resource usage settings (mostly dummy for now)

### Point tracking (0.0.x)

- track one (draggable?) point with lines (point worker)
- track an area? (only use case for parelelized point worker)


### Gears + Checked work (0.0.x)
- results are calculated in two precision levels and if they differ the precision increases
- i16 - i128, after that use rug or smth big integers. perhaps some custom code for i256 or something may be faster than rug but likely rug is best
- worker should never get stuck on a difficult point

### Attention (0.0.x)
- Attention (workgroup focuses on the area around the cursor)
- possibly custom cursor image


### Memory 2: electic boogaloo (0.0.x)

- memory should store as much work from positive dynamic res as deemed necessary based on memory available and current attention location.

### Workgroup expansion (0.0.x)
- n screen workers based on core count
- n point workers based on core count
- one gpu-accelerated screen worker
- implement resource usage settings; default to using all resources when focused, and none when unfocused.
- Finally, in some cases and on certain hardware, there should be only one active point worker which should be the *only pinned actor*. 
  This is to ensure that it can run at the cpu's maximum frequency. Other work should be performed mostly by the gpu anyway.
  CPU frequencies should be monitored and cases like this handled.
  

### Perturbation (0.0.x)
- more precise mouse position...?
- standard perturbation, with lookahead through point workers and attention
- checked via arbitrary precision

### Can we crack it? (0.0.x)
- check that relativity is followed (machinery is prepared for n-deep real time)
- perturperturbation...? (check as deep as possible. If it checks correct, assume it remains correct.)

### The MVP 1 (0.1.0)
- everything in all existing features should be extremely polished, responsive, and nice to use, or if that isn't possible, deleted.
    - sampler smearing works on all edges (if possible without hurting fps)
    - drag + zoom out stores exact drag start location
    - drag + zoom in as precise as possible (perhaps guess at the structure selected and interpolate it if that isn't annoying)
    - drag sliding and smooth zoom (optional, only if not annoying)
    - easy to use location & zoom get and set
    - instant home button

- zooming depth
    - zooming to 2^3600 in real time to provide an entire 6 minute zoom depth at the easy spots
    - zooming to 2^600 in real time to provide an entire 1 minute zoom depth at the most difficult known spots
    - if perturbation farming works:
        -  infinite zooming at any location