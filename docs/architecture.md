THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Critical Zoomer Architecture Plan

## Definitions

Work: The hoarded unit; the results of iterations up till the point is determind as in the set or outside the set (bailout at r=2)

## Structures

### Stencil

The stencil defines the set of pixels which make up a screen and their exact locations in complex space.

### View

The view is responsible for holding and manipulating located frames of computed and/or shaded results.
The project must contain 0 Vecs of pixels; it must use Views for all such cases.
Views must be used in all actors to store the actor's input buffer, and as the actor's produced output. The view must be produced then filled using the indexing methods, not edited to insert a vec which may or may not be correctly lengthed to agree with the stencil.

The Headgroup must use a View to sample RGB frames under user movement.
The workgroup must use views to manage completed work and work in progress, and to resample on movement. The workgroup must also make use of the bitmap to prioritize misses (0) over representative values for new work.

The view contains a stencil, a vec of data which is the same length as the number of pixels defined by the stencil, and a vec of bytes defining whether each pixel was mapped exactly, or at all.
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

 Pixels must be properly aligned so structures don't appear to shift when greater detail is available.


When transforming older work:

  The code used for transforming old work must be the same code used in the Headgroup to sample rgb values and provided the same transform as input and ensured to be the same frame position, zoom, and resolution as the RGB buffer in the Headgroup so the results are guaranteed to match.


The Workgroup must send hoarded work for the current frame to the Shadergroup anytime it is remapped due to a transform or 50ms has passed and the screen is not yet complete.
When the screen is not yet complete, the Workgroup must always have some new work at this interval; it must be able to pause a particularly difficult point and continue it in the next workshift.


### Shadergroup

The shadergroup must be responsible for cosmetic layers generated from work.
It must recieve the current frame from the hoard (at least the current frame according to the Workgroup)
and processes it into RGB pixels.
One of its responsibilities is custom bailout raidii; 
it must satisfy the [2, ∞] bailout radius range. It must do so by continuing from the escape location, which must be included in the completed work by the Workgroup.

The Shadergroup must send RGB pixels and their location to the Headgroup whenever they have changed, and at framerate when animating.

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

Form Factor: Headgroup
System Policy: Workgroup, Shadergroup
Control Scheme: Headgroup
Display Scheme: Headgroup
Cosmetic Options: Headgroup & Shadergroup
Controls Mechanics: Headgroup & Workgroup
Seamless: Headgroup & Workgroup
Deep: Workgroup
Tenacious: Workgroup
Hoarding: Workgroup
Fast: All
