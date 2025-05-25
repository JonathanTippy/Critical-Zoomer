![Untitled-Artwork_shortened](https://user-images.githubusercontent.com/54297927/212390663-ff8359e9-438a-4742-8cf6-3b7675a27f7a.jpg)
Artwork by Deborah Tippy

# Critical Zoomer
a mandelbrot set zoomer written in rust

## Current state of the project
- originally i had ambitions to make it a rival to Xaos but i didnt work on it for a long time so i fixed it up the last few days well enough that it works passably
- not worth using for majority of fractal enthusiasts
- really just a proof of concept

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
- movement: arrow keys
- adjust number of iterations: i to increase and o to decrease
- adjust number of extra bits: e to increase and r to decrease
- adjust zoom: f to zoom in and g to zoom out

## Power of two denominator fractions (beware, mumbo jumbo ahead)
here is a short explanation; by using power of two denominator fractions, all of the math can be done using integers. thats right; all of the mandelbrot rendering is done using integers! Here's how it works:
You first decide how many bits you need. you will need at list enough values to uniquely identify each pixel, accounting for how far in you have zoomed (which requires additional precision), and also around 10 extra bits so your mandelbrot won't get messed up due to not enough precision. then, you convert each pixel to a numerator which fits that denominator. now, the math. we need to do a couple things: addition, and mulitplication. lets start with addition. in order to add two of these numbers, we simply add the numerators. we can do this since they have the same denomenator and therefore the ones of each numerator represent the same value. Multiplication is almost as fast, but since we're talking about a fraction we need to get fancy. we could just multiply the numerators together, but they will be squared relative to the numberator. no matter, actually this is useful; i make use of it by comparing these squares added together to the numerator of four which is two squared, because you must square values anyway if you want to get a circle. i could just accept the square but it messes up the coloring of parts outside of the set.
