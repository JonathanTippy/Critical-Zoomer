![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

## Roadmap (subject to change)
### MVP (0.0.1)  in progress.....
- blazing fast window exists | ✔️
- workday-based worker exists | ✔️
- window and worker communicate | almost...
- zooming to 2^60x in real time | 
### Something (0.1.0)  awaiting completiong of MVP...
- properly restart window actor on window crash
- settings
- attention
- perturbation & other advanced methods (possibly derbail, possibly boundary tracing, hopefully perturperturbation)
- multi-platform (app acts as it should at all resolutions, and on windows linux and mac, iphone and android)
- basic polish (animations, data is stored, data is combined, app acts as it should in all cases)
- fully use the machine (use all available cpu gpu ram and storage while making sure not to bother the user)
- zooming to 2^3600x in real time, or if perturperturbation is possible, zooming infinitely in real time.
### more cool stuff (0.x.x)  awaiting completiong of 0.1.0...
- maybe eye tracker
- maybe torrent solution sharing (if perturperturbation proves impossible)
- maybe resource sharing across LAN
- etc
### (1.0.0)   awaiting completiong of 0.x.x...
- zooming infinitely in real time
- UX is perfect



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
- the mandelbrot set, tracking a given point and overlaying its path, and perhaps in the future:
- julia sets
- the buddhabrot
- other degrees of the mandelbrot set
