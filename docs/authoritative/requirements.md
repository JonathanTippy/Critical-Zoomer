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

r[cz.system.memory-default-1gb+1]
The app must have a memory limit box in settings defaulting to 1gb meaning 1GB cpu memory + 1GB vram.
The maximum limit must be unlimited and the minimum limit must be calculated on-demand.
The minimum limit must be able to bump the slider if it rises.

r[cz.system.tile-manager-protect-current-lookahead+1]
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

r[cz.ui.coords-parse+1]
- empty field at top of screen which accepts coordinates 
- The field accepts ALL likely forms of coordinate entry;
  the user must never be confused about why his coordinates are not accepted.
  Field requires: Two numbers separated by a space or comma, or plus with i.
  Field accepts: parens square brackets, braces, extra spaces, other decorations.
  If present, rich inputs must be handled correctly:(5i + 6) = (6 + 5i)

r[cz.ui.coords-apply+1]
- 'apply' button by the field which is greyed out when the field is empty or invalid.
  'apply' must not be grey out whenever its already equal in location to the viewport location.
  When applied, the viewport (considered to be located at its center) must be moved to the location in the field. 
  The field must not be cleared.
- The coordinates entry and display must also include a magnification level, always. a coordinate is not sufficient to define a view.

settings:
- secondary window with widgets on it. Opened via a gear button floating in the top right corner.

### Display Scheme

window:

r[cz.display.window-default-800x480+1]

- The app must default to 800x480 on startup and not restore a customized size on launch.

viewport:

r[cz.ui.viewport-fill+1]
- one viewport must cover the entire window. It must dynamically resize with the window.

r[cz.math.mandelbrot-real-axis-symmetry+1]

- The viewport must display the mandelbrot set.
  See Controls Mechanics for details on wandering off / zooming out too far.

location:

r[cz.ui.location-readout+1]
- one read-only selectable field must be at the top of the screen with a 'copy' button by it.
  The location displayed must correspond with the center of the viewport.

## Application Details

### Cosmetic Options

r[cz.cosmetic.layer-model+1]

The app must have coloring options for normalizing the input data:
- log scale
- reciprocal scale

and then colorizing that result based on various functions:
- sin
- modulo

The app must allow escape time, periodicity etc to be colored separately and ordered in a list which determines painting order.

The app must allow specifying the base color for each layer, its opacity inside the set, and its opacity outside the set.

The app must highlight features such as in filaments, out filaments, and minibrots (nodes), and allow including these results in the coloration or not.

r[cz.cosmetic.bailout-range-2-255+1]
The app must allow customizing the bailout radius to at least any value in: [2, 255].
Bailout radius must be animable at full vsync rate via continuation from stored escape z with a limited iteration count.
In almost all cases, even 50 iterations is plenty because values escape extremely quickly.

All these cosmetic features must run quickly because they start from hoarded work.

r[cz.fast.cosmetic-17ms-1080p+1]
All cosmetic features (that are continuous rather than enumerated) must hit 17ms frametime at 1080p.

r[cz.cosmetic.defaults+1]

The cosmetic settings must come set to a reasonable default which allows browsing without needing to edit them:
- shows escape time
- shows in filaments as black pixels
- show out filaments colored like out pixels with ∞ escape time
- may show other features subtly

### Controls Mechanics

r[cz.ctrl.drag-anchor+1]

The user must be able to zoom back into a very particular point if they began a mouse drag there;
This allows them to zoom out to see the whole set or slightly larger surroundings, and then zoom back in without losing their place.

r[cz.ctrl.hover-zoom-origin+1]

Except when using space and shift,
Zooming must be origin-ed at the spot the mouse hovers, implying when zooming, 
the spot under the mouse cursor stays fixed.

r[cz.display.offscreen-r2-circle+1]
The viewport must not disallow zooming too far out / moving so the set is off-screen.

r[cz.display.offscreen-arrows+1]
It must add red arrows when it determines that the set is mostly or fully off-screen or is almost or fully too small to be seen.

r[cz.ctrl.scroll-up-zooms-in+1]

Scroll up must be zoom in.

## Central Differentiators

### Seamless

r[cz.tenacious.no-max-iter+1]

The app must not have a "max iteration count" setting;
points must be iterated to completion. 
This should keep up with the user but might not. Low-res interim systems are acceptable.

r[cz.seamless.perturbation-always-on+1]

The app must not have a perturbation toggle;
perturbation must always be on.

r[cz.seamless.gpu-preferred+1]

The app must not have a GPU toggle;
GPU acceleration must always be on.

r[cz.seamless.reference-background+1]

The app must not have a reference orbit input;
Reference orbits must be computed in the background and must not show a progress bar or prevent user activity.

r[cz.seamless.foveated-mag-velocity+1]

The app must use fovated rendering to prioritize the area around the mouse pointer and do deep lookahead, balancing it with filling in the screen, so that the user will be met with the best res possible given available working time and recent movements.

### Deep

r[cz.deep.min-zoom-pot-capacity+1]
The app must go as deep as the user wants.
This means 100 hours of comfortably zooming in, here estimated to be 2^(10 * 2 * 60 * 60 * 100) which is factor 2^3600000;
The app must zoom to at least factor 2^3600000.

r[cz.deep.snappy-at-depth+1]

depth doesn't compromise responsiveness requrements: the app must still feel snappy (definition: headgroup running at full framerate. pan controls and zoom controls executed at framerate, browsing headgroup's current answer hoard.) when at its depth target.

### Tenacious

The app must discard the concept of a "max iteration count" and instead always 
attempt to finish its work, as long as its still visible.

r[cz.tenacious.nores-not-flat-black+1]
Unfinished pixels must not be colored flat black: 
If work (In or Out conclusion), being exact or proximate, exists covering the pixels, 
they must be filled from low-res work, or if bailout was unexpectedly difficult, 
which occurs when zooming into (-2, 0), using a best-effort approximation of the escape time.
If it does not, the pixels must not be unceremoniously colored black.

r[cz.display.nores-when-no-proximate+1]
The app must include a "no resolution" point, the point at infinity, which completes the dynamic res stack and fills in missing data.

### Hoarding

r[cz.hoarding.one-answer-per-point+1]

There must be only one answer per point; Mandelbrot work must be deterministic and, as far as is possible, exact with regard to values relevant to rendering.

Work must be kept in a buffer so it survives cosmetic changes.

r[cz.hoarding.no-compute-settings+1]

There must not be a "max iteration count" setting which forces a full recompute pause.
In fact, there must be no computation settings whatsoever;
no settings with regard to computations done in determining whether a point is inside the set or outside the set.

When the view moves, the data buffers which hoard work must not be cleared. This data must be used to provide a continuous output, and to prevent redoing already done work.

Display settings (including highlighting, bailout, and coloring) change how pixels look but must start from hoarded work, not replace it.

### Fast

r[cz.fast.settings-100ms+1]
all settings must feel instant: result visible within 100ms.

All non-enumerated, rendering related settings must be animable at full monitor refresh rate. (17ms 1080p)

r[cz.fast.natural-zoom-2x+1]
Definition of "natural": Zooming must zoom at 2x magnification per mouse wheel bump.
(when zoom origin is center, this means the middle half of the screen (by side length) becomes the entire screen.)

r[cz.fast.scroll-10-in-300ms+1]
The app must be able to sustain real time activity when zooming 10 bumps within 300ms, and when repeating that movement every second.

r[cz.fast.shift-space-5bps+1]
The space / shift control options will be a little slower than the mouse, about 5 bumps per second.

r[cz.fast.no-tick-backlog+1]
The user must see an immediate step on every wheel tick, and fast spinning must not skip or backlog ticks.
Work might not keep up the pace; it Should, but if it doesn't, 
the user must see what they just saw, just magnified, so they must see big square pixels / low-res.

r[cz.fast.input-next-frame-17ms+1]
The user must see their movements and zooms on this or the next frame; 17ms at 60hz.

To that end, component or components responsible for generating work must be extremely fast and efficient with scheduling; What the user recieves is a direct result of how efficiently the time available is used. Target is that scheduling & data transfer are insignificant compared with time spent working even in trivial cases. (without cheating by making work harder than it is)

### Calibrated

r[cz.calib.lowres-synthesis+1]

The app must interpolate and output low-res where appropriate.
When older work is proven incorrect by newer work, the app must show a synthesis which discards neither and takes full advantage of all data continuously.

# E2E Addendum:

r[cz.e2e.harness-stack+1]

The most important 3 things;

r[cz.e2e.controls-bindings+1]

r[cz.e2e.controls-no-jump+1]

- controls work as expected (tie to requirements) and don't jump around or do weird things

r[cz.e2e.perf-home-fill+1]

r[cz.e2e.perf-zoom-simple+1]

r[cz.e2e.perf-zoom-hard+1]

- the app is as fast as expected (<5s to fill the home screen, smoothly humming along (apparently, completely and utterly perfect (meaning new full view of answers are complete within 100ms of zooming in)) when user zooms into simpler areas, does as well as it should (lower res, still keeping pace (meaning apply greedy methods and progressive refinement)) when user zooms into less simple areas)

r[cz.e2e.visual-oracle+1]

r[cz.e2e.visual-assistant-review+1]

- app contains no visual artifacts (compute oracles via known good code & oracle proving tests, then test against those. Also always do an assistant visual check, which is imperfect (makes mistakes human eyes wouldnt) but worth doing.)

Controls and visual will be hard to test but must be rigorously tested via properties & known-good oracles.


