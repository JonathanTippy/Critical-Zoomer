THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Critical Zoomer Feature Requirements

## Surface Shape

### Form Factor

Critical Zoomer must be released as A Single x86-64 binary; 
a Linux desktop app supporting X11 and Wayland in the exact same binary file.
It must be FOSS. 
Github Releases must contain this file.
The app must be distributed on Flatpak and in debian linux.

### System Policy

The app must have a memory limit slider which lives in settings and which allows the user to cap memory use to prevent big surprises.
The maximum limit must be 1gb and the minimum limit must be calculated on-demand.
The default limit must be 512MB.
The minimum limit must be able to bump the slider if it rises.
If the minimum limit collides with the maximum limit, 
it must bump window resizing and prevent increasing the window size, but not prevent decreasing it.
The on-screen hoard must never be evicted for memory reasons.

### Control Scheme

moving: 
- WASD
- Arrow keys
- Mouse Left click drag

zooming: 
- mouse scroll (zoom origin is mouse's current hover location)
- shift key = zoom in, space key = zoom out (zoom origin is the center of the screen)

move viewport to (0 + 0i):
- home button floating in top right corner.

coordinates:
- empty field at top of screen which accepts coordinates 
- The field accepts ALL likely forms of coordinate entry;
  the user must never be confused about why his coordinates are not accepted.
  Field requires: Two numbers separated by a space or comma, or plus with i.
  Field accepts: parens square brackets, braces, extra spaces, other decorations.
  If present, rich inputs must be handled correctly:(5i + 6) = (6 + 5i)
- 'apply' button by the field which is greyed out when the field is empty or invalid.
  'apply' must not be grey out whenever its already equal in location to the viewport location.
  When applied, the viewport (considered to be located at its center) must be moved to the location in the field. 
  The field must not be cleared.

settings:
- secondary window with widgets on it. Opened via a gear button floating in the top right corner.

### Display Scheme

window:
- The app must default to 800x480 on startup and not restore a customized size on launch.

viewport:
- one viewport must cover the entire window. It must dynamically resize with the window.
  The viewport must display the mandelbrot set.
  See Controls Mechanics for details on wandering off / zooming out too far.

location:
- one read-only selectable field must be at the top of the screen with a 'copy' button by it.
  The location displayed must correspond with the center of the viewport.

## Application Details

### Cosmetic Options

The app must have coloring options for normalizing the input data:
- log scale
- reciprocal scale

and then colorizing that result based on various functions:
- sin
- modulo

The app must allow escape time, periodicity etc to be colored separately and ordered in a list which determines painting order.

The app must allow specifying the base color for each layer, its opacity inside the set, and its opacity outside the set.

The app must highlight features such as in filaments, out filaments, and minibrots (nodes), and allow including these results in the coloration or not.

The app must allow customizing the bailout radius to at least any value in: [2, 255].

All these cosmetic features must run quickly because they start from hoarded work.
All cosmetic features (that are continuous rather than enumerated) must animate at 60fps 1080p.

The cosmetic settings must come set to a reasonable default which allows browsing without needing to edit them:
- shows escape time
- shows in filaments as black pixels
- show out filaments colored like out pixels with ∞ escape time
- may show other features subtly

### Controls Mechanics

The user must be able to zoom back into a very particular point if they began a mouse drag there;
This allows them to zoom out to see the whole set or slightly larger surroundings, and then zoom back in without losing their place.

Except when using space and shift,
Zooming must be origin-ed at the spot the mouse hovers, implying when zooming, 
the spot under the mouse cursor stays fixed.

The viewport must not disallow zooming too far out / moving so the set is off-screen.
It must add red arrows when it determines that the set is mostly or fully off-screen or is almost or fully too small to be seen.

## Central Differentiators

### Seamless

The app must not have a "max iteration count" setting;
points must be iterated to completion. 
This should keep up with the user but might not. Low-res interim systems are acceptable.

The app must not have a perturbation toggle;
perturbation must always be on.

The app must not have a GPU toggle;
GPU acceleration must always be on.

The app must not have a reference orbit input;
Reverence orbits must be computed in the background and must not show a progress bar or prevent user activity.

### Deep

The app must go as deep as the user wants.
This means 100 hours of comfortably zooming in, here estimated to be 2^(10 * 2 * 60 * 60 * 100) which is factor 2^3600000;
The app must zoom to at least factor 2^3600000.

depth doesn't compromise responsiveness requrements: the app must still feel snappy when at its depth target.

### Tenacious

The app must discard the concept of a "max iteration count" and instead always 
attempt to finish its work, as long as its still visible.

Unfinished pixels must not be colored flat black: 
If work (In or Out conclusion) exists covering the pixels, 
they must be filled from low-res work, or if bailout was unexpectedly difficult, 
which occurs when zooming into (-2, 0), using a best-effort approximation of the escape time.
If it does not, the pixels must be clearly recognizable as not known.

### Hoarding

There must be only one answer per view; Mandelbrot work must be deterministic.

Work must be kept in a buffer so it survives cosmetic changes.

There must not be a "max iteration count" setting which forces a full recompute pause.
In fact, there must be no computation settings whatsoever;
no settings with regard to computations done in determining whether a point is inside the set or outside the set.

Across viewport transforms, work still in frame must still be hoarded, and read from the hoard:
- When moving
- When zooming out
- When zooming in

That work must be actually saved (not redone):
- When moving
- When zooming out
- May / May not when zooming in: the savings would be tiny. Depends if its in the way of the other two.

Display settings (including highlighting, bailout, and coloring) change how pixels look but must start from hoarded work, not replace it.

### Fast

all settings must feel instant: result visible within 100ms.

All non-enumerated, rendering related settings must be animable at full monitor refresh rate. (60hz 1080p)

Definition of "natural": Zooming must zoom at 2x magnification per mouse wheel bump.
(when zoom origin is center, this means the middle half of the screen (by side length) becomes the entire screen.)
The app must be able to sustain real time activity when zooming 10 bumps within 300ms, and when repeating that movement every second.
The space / shift control options will be a little slower than the mouse, about 5 bumps per second.

The user must see an immediate step on every wheel tick, and fast spinning must not skip or backlog ticks.
Work might not keep up the pace; it Should, but if it doesn't, 
the user must see what they just saw, just magnified, so they must see big square pixels / low-res.
The user must see their movements and zooms on this or the next frame; 17ms at 60hz.

### Calibrated

The workgroup must interpolate and output low-res where appropriate.
Ranges must be used to keep track of in-progress work.
For example, in WIP points,
- Some lower bound of the escape time is known.
- Some lower bound of min magnitude time is also known
- The escape location is known to be somewhere in the ring between circle r=2 and circle r=6 (2^2=4+2=6)
- Some min magnitude upper bound is known

When low-res work is proven incorrect by bounds, it should be nudged by them by the smallest amount to make the result possible.
