![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

## Da plan (subject to change)

### The MVP (0.0.1)  done!
- blazing fast window exists | ✔️
- workday-based worker exists | ✔️
- window and worker communicate | ✔️
- 
### Fix the jank (0.0.2)
- window / worker linkakge is stable during quick inputs | in progress...
- window / worker handle resolution changes
- fix zoom while drag

### Rember (0.0.3)
- worker remembers just-completed work when moving or zooming
- rework worker resolution and updates; add work collector after worker so worker can send updates every workday
- sampler remembers several frames to sample from
- etc
  
### Settings (0.0.4)
- settings for coloring
- controls settings
- etc

### Lookahead (0.0.5)
- lookahead

### Data Combiner (0.0.6)
- data combiner actor to make use of high resolutions
- combiner can also work progressively if there is not room to store all the data

### Data Interpolator (0.0.7)
- data interpolator to make up inbetween data when there are not enough points

### Attention (0.0.8)
- Attention

### Perturbation (0.0.9)
- perturbation
- perturperturbation if possible






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
