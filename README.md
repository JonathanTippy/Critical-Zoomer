![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

## Da plan (subject to change)

### The MVP (0.0.1)  done!
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

### Memory + Area pixels (0.0.3) refanagling...
- worker remembers just-completed work when moving or zooming
- worker no longer workday based, but workshift and checks its in channel every shift 
- rework worker resolution and updates; add work collector after worker so worker can send updates at a fixed rate 
  ARVs will at first be generated using just one point each (and in fact fewer than that) (and its neighbors for filament detection), and progressively add more points.
  
- colorer now recieves 'area representative results' which can be in (period), out (escape time), or edge ((standard, in filament, or out filament), (period, escape time, ratio)).
    standard edges are just where the sample points don't all agree on in or out.
    out filaments can be sometimes detected by differing periodicity (except cardioid corners);
    in filaments can be sometimes detected by differing escape time derivatives.
    Because of how unreliable this will be, it should remain an opt-in option to color filaments differently until it becomes reliable.
    
- can we have perfect rendering early through filament highlighting? 
  Perhaps, however providing the option of progressively improving detail through data combination
  (average or other methods can be used) may result in more pleasing and correct results, so the option should be available.
  Depending on memory available, this additional work may need to be largely discarded after completion.
  
### Point tracking + Settings (0.0.4)
- track one (draggable?) point with lines (point worker)
- track an area? (only use case for parelelized point worker)
- settings for coloring
- controls settings
- mixmap settings
- bailout settings
- colors and bailout can be animated with different values
- resource usage settings (mostly dummy for now)

### Gears + Checked work (0.0.5)
- results are calculated in two precision levels and if they differ the precision increases
- i16 - i128, after that use rug or smth big integers. perhaps some custom code for i256 or something may be faster than rug but likely rug is best
- worker should never get stuck on a difficult point

### Attention (0.0.6)
- Attention (workgroup focuses on the area around the cursor)
- possibly custom cursor image

### Workgroup expansion (0.0.7)
- n screen workers based on core count
- n point workers based on core count
- one gpu-accelerated screen worker
- implement resource usage settings; default to using all resources when focused, and none when unfocused.
- Finally, in some cases and on certain hardware, there should be only one active point worker which should be the *only pinned actor*. 
  This is to ensure that it can run at the cpu's maximum frequency. Other work should be performed mostly by the gpu anyway.
  CPU frequencies should be monitored and cases like this handled.
  

### Perturbation (0.0.8)
- more precise mouse position...?
- standard perturbation, with lookahead through point workers and attention
- checked via arbitrary precision

### Can we crack it? (0.0.9)
- check that relativity is followed (machinery is prepared for n-deep real time)
- perturperturbation...? (check as deep as possible. If it checks correct, assume it remains correct.)

### Polished (0.1.0)
- everything should be extremely polished, responsive, and correct





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
