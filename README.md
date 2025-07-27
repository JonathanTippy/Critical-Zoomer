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
- rework worker resolution and updates; add work collector after worker so worker can send updates every workday
- colorer now recieves 'area representative results' which can be in, out, or edge. out filaments can be detected by differing periodicity (except cardioid corners); in filaments can be detected by differing escape time derivatives.
- sampling remembers several iamges to sample from for lookahead and faster home
  
### Point tracking + Settings (0.0.4)
- track one (draggable?) point with lines
- track an area?
- settings for coloring
- controls settings
- mixmap settings
- bailout settings
- colors and bailout can be animated with different values
- resource usage settings (mostly dummy for now)

### Gears + Checked work (0.0.5)
- results are calculated in two precision levels and if they differ the precision increases
- i16 - i128, after that use rug or smth big integers. perhaps some custom code for i256 or something may be faster than rug but likely rug is best

### Attention (0.0.6)
- Attention (workgroup focuses on the area around the cursor)
- possibly custom cursor image

### Workgroup expansion (0.0.7)
- n screen workers based on core count
- n point workers based on core count
- one gpu-accelerated screen worker
- implement resource usage settings; default to using all resources when focused, and none when unfocused.

### Perturbation (0.0.8)
- more precise mouse position...?
- standard perturbation, with lookahead through point workers and attention
- checked via arbitrary precision

### Can we crack it? (0.0.9)
- check that relativity is followed (machinery is prepared for n-deep real time)
- perturperturbation...? (check as deep as possible. If it checks correct, assume it remains correct.)






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
