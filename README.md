![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

## Da plan (subject to change)
### The MVP (0.0.1)  done!
- blazing fast window exists | ✔️
- workday-based worker exists | ✔️
- window and worker communicate | ✔️
### The MVP 2: electric boogaloo (0.0.2)
- window / worker linkakge is stable during quick inputs | in progress...
### Rember (0.0.3)
- worker remembers just-completed work when moving or zooming
### ? (0.0.4)


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
