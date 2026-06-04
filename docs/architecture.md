THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Critical Zoomer Architecture Plan

## Definitions

Work: The hoarded unit; the results of iterations up till the point is determind as in the set or outside the set (bailout at r=2)

## Assemblies

### Headgroup

The headgroup must be responsible for all things which run strictly at window framerate and face the user.
It must ensure that the user sees what they expect immediately, even if the work really hasn't quite caught up.
It must also contains all settings and IO.

The Headgroup must send the viewport position, factor of zoom, and screen size to the workgroup whenever any of those change.
The Headgroup must send the settings to the other two groups anytime they change.

### Workgroup

The workgroup must be responsible for completing and hoarding work.
It must immediately pause or discard work which is no longer present in the viewport.
It must recieve the current viewport location, level of magnification, and screen size from the Headgroup, 
and based on those, decides what work to do.

For any pixel which is not yet completed (determined in or out), it must be filled in:

I. When moving.
  1. from previous work / lookahed, best res available
  2. failing that, by smearing / extruding the pixels at the edge of the screen.
     Along with this, during pans/moves, the workgroup must prioritize screen edges which are being extruded in the work schedule. 
     (edges on the opposite side to the current direction of travel)
     This allows a 2d problem to become a 1d problem, and produces a less than ideal but much better than nothing "active temporal dynamic resolution".
     Obviously its not completed work, and secondarily, its biased; technically the extruded sections should have their source values at their center, 
     but instead they have them at the edge.
     As soon as the movement has ceased and the worker is allowed to catch up, it should re-do the smeared areas,
     but not waste time redoing the rest of the frame which was merely translated.
  3. failing that (initial startup), it must produce an "incomplete" signal. This is done by rapidly varying all attributes randomly.
     There must not be a "dummy" or "idk" or "incomplete" struct / enum.
II. When zooming in
  from previous work / lookahed, best res available. Zooming in is manifestly a subset; no further fallbacks necessary.

III. When zooming out
  1. from previous work / lookahed, best res available
  2. failing that, it must produce an "incomplete" signal.


When doing best-res output:

 1. For points outside the set, the workgroup must interpolate rather than using large square pixels:
For a given point C, At a given iteration count, 
the magnitude of Z can be estimated by averaging the magnitudes of Z at that iteration count of two points either side C weighting the distances, where one of the chosen points has at least as large an escape time as C itself.
This is assumed to yield the iteration count at which C escaped; it will be necessary that all pixels store Z at smallest iteration count of escape of any of its neighbors. 
From these Zs, the Mandelbrot function allows deriving the subsequent Zs. The algorithm can then advance until escape, checking the interpolated Z until it escapes, and assigns it the estimate escape time, which is the smallest escape time of its parents plus however many steps the interpolation took.

 2. For points inside the set, squares are fine.


 3. Interpolation 

When 

The Workgroup must send hoarded work for the current frame to the Shadegroup anytime it is remapped due to a transform or progress is made.





### Shadegroup

The shadegroup must be responsible for cosmetic layers generated from work.
It must recieve the current frame from the hoard (at least the current frame according to the Workgroup)
and processes it into RGB pixels.
One of its responsibilities is custom bailout raidii; 
it must satisfy the [2, ∞] bailout radius range. It must do so by continuing from the escape location, which must be included in the completed work by the Workgroup.

The Shadegroup must send RGB pixels and their location to the Headgroup whenever they have changed, and at framerate when animating.

## Technologies

### Rust

Rust is the language for this project:;
chosen for its great performance while still being easier than manual memory languages like C.

### Steady State

Steady state is the cornerstone of this project;
Previous free implementations either use one core, and start to chug when there's too much work to do, or they use a secondary "come back when you're done" core, which can't display its partially completed work. 
Steady state allows the developer to build a machine: a system where data does what it ought to do, not what threading limitations forced.

### Egui

Egui is the current standard for rust desktop application dev.

### Rug

Rug is the current standard for large numbers.

## Requirements Allocation

Form Factor: All
System Policy: All
Control Scheme: Headgroup
Display Scheme: Headgroup
Cosmetic Options: Headgroup & Shadegroup
Controls Mechanics: Headgroup
Seamless: Headgroup & Workgroup
Deep: All
Tenacious: All
Hoarding: Workgroup
Fast: All
