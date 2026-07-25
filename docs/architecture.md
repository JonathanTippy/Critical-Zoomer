THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Critical Zoomer Architecture Plan

## Definitions

Work: The hoarded unit; the results of iterations up till the point is determind as in the set or outside the set (bailout at r=2)

## Assembly API

### Stencil

The stencil defines the set of points which make up a screen and their exact locations in complex space. The headgroup sends a stencil to the workgroup to notify it of the most urgently needed screen.

### Tile

A tile is a const POT sized square of points. It should be 64x64 but may not be if other considerations effect it.
Tiles of the same magnification should share a stencil as their location definition. Too many stencils is too much data.

The tile is the unit with regard to work, chosen becuase single points would involve too many serial items per second and too much per-point overhead, and entire screens involve too much inefficiencies from being forced to consider and manipulate all the points.
Tiles solve these issues and make transferring, managing, and persisting work possible.

Tiles must have a CPU variant and a GPU variant.

## Assemblies

### Headgroup

#### IO

Input: GPUTile<GPUAnswer>
flow per second (incomplete): [30, 60]
flow per second (complete): 0 

Output: Stencil
flow per second (moving): 60
flow per second (still): 0

The headgroup must be responsible for all things which run strictly at window framerate and face the user.
It must ensure that the user sees what they expect immediately, even if the work really hasn't quite caught up.
It must also contains all settings and application IO.

The Headgroup must send the viewport position, factor of zoom, and screen size to the workgroup whenever any of those change.

The Headgroup must own the display answer hoard and places incoming GPUTiles into it
There must be no Color32 hoard. gpu Answer tiles must be stored, and sampled then shaded at display time.

### Workgroup

#### IO

Input: Stencil
flow per second (moving): 60
flow per second (still): 0

Output: Tile<Answer>

flow per second (incomplete): [30, 1000]
flow per second (complete): 0


The workgroup must be responsible for completing work. It must store its own collection of already completed work for continuity of outputs and scheduling. This will probably be the same point collection as in the headgroup.
It must immediately pause or discard work which is no longer present in the viewport.
It must recieve the current viewport location, level of magnification, and screen size from the Headgroup, 
and based on those, decides what work to do.

When the screen is not yet complete, the Workgroup must always publish some new work at the minimum interval; it must be able to pause work and continue it in the next workshift.

### GPU Uploader

Input: Tile<Answer>
flow per second: [0, 1000]
Output: GPUTile<GPUAnswer>
flow per second: [0, 1000]

uploads tiles to the GPU

## Technologies

### Rust

Rust is the language for this project:;
chosen for its great performance while still being easier than manual memory languages like C.

### Steady State

Steady state is the cornerstone of this project;
Previous free implementations either use one core, and start to chug when there's too much work to do, or they use a secondary "come back when you're done" core, which can't display its partially completed work. 
Steady state allows the developer to build a machine: a system where data does what it ought to do, not what threading limitations forced.
This app is built on the steady state philosophy: there is no light load or heavy load, only load.
Channel backpressures are a sign of *incorrect code*, not transient stress.

### Egui

Egui is the current standard for rust desktop application dev.

### Rug

Rug is the current standard for large numbers.

## Requirements Allocation

Form Factor: Headgroup
System Policy: Workgroup
Control Scheme: Headgroup
Display Scheme: Headgroup
Cosmetic Options: Headgroup
Controls Mechanics: Headgroup
Seamless: Headgroup & Workgroup
Deep: Workgroup
Tenacious: Workgroup
Hoarding: Workgroup
Fast: All
