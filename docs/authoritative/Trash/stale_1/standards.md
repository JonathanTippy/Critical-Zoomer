THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.

Eval standards for the project. Design requirements.

Terminology:
benchmarking:
IPP: iterations per point. Used to evaluate algorithm efficiency. must be controlled for location, as points do legitimately differ. Compare iterations used to optimal iterations (out -> escape time, in -> preperiodic + period)
IPS: iterations per second. Used to evaluate workgroup performance. For most honest benchmarking of other overhead, pick areas where points take few iterations. For most honest benchmarking of iteration code, pick slower points.

heads up display:
TPS: new completed tiles added to the headgroup collection per second. Show to user in HUD. Not workgroup emission rate.
FPS: frames per second (duh) only refers to the unified headgroup hybrid window actor framerate.



# Workgroup

Mandelbrot Work Performance north star:
Algorithm code:
- One path. branching the algorithm code into simple and hard cases to avoid optimizing the optimizable hard case is banned.
  The mandelbrot code must not branch based on available reference, but fall back to the 0 reference case and always use perturbation.
  The mandelbrot code must not branch for different types, but use the mandelbrotable trait for all math (except for the GPU code which must all be written manually and then tested for parity with CPU to debug)
- On stack as much as physically possible: heap is slow.
- On GPU as much as physically possible: keep the GPU fed.
- branches go outside of loops. not inside of loops.
- if branches must be used, always make the true side the more common path.
Work Management:
- aggressively leverage boundary tracing + infill
- aggressively restless & greedy scheduling
- must get around to everything eventually (make the hoard recontinuable)

r[cz.perf.foveation-half-time+1]
- foveated (Dedicate half of available working time to filling the current stencil, and half to lookahead.)

r[cz.perf.play-minimize+1]
- aggressively minimize play: no or very small initialization phases. continuous delivery of work so far.


Workgroup performance requirements;

- on home view, must average 100TPS (~5s to complete the home view)

r[cz.perf.min-300m-ips-cpu+2]

r[cz.perf.min-30b-ips-gpu+1]
- Always, must hit minimally 300M IPS (single core CPU) and 6B IPS (GPU) even when using perturbation (a few extra features is NO excuse for 10X slower. See first performance north star.)
  obviously the 11th gear (largest non-stack) might be a bit slower, but not 10X slower. See first performance north star.

r[cz.perf.optimal-ipp+1]
- IPP must be optimal always

r[cz.perf.play-8bump-100ms+1]
- When the user has zoomed in 8 bumps at a time, *some* new work must be visible within 100ms of the last bump of the gesture. See north star on play.

# References addednudm

r[cz.ref.zero-orbit-same-path+1]

The reference datastructure must correctly handle looping points.
There must be a const reference orbit starting at big Z of zero
when a better reference is not available, this const reference must be used.
The path of code used must not be different.

# Publisher Addendum

r[cz.pub.gpu-native-work+1]

Work remaining native n the GPU is of pivotal importance 
because otherwise there is not enough throughput to complete a full screen of work quiclly in easy cases.

# Benchmarking addendum

IPS requirements must be tested in real plausible situations, not microbenches. This means that the benchmark will include all scheduling overhead.
It should test both the worst case for scheduling (easy tiles, outside of r=2) and the best case (work takes longer, eg inside the set).

Microbenches should also be made as its useful to know whether an issue is the math itself or scheduling or something else.

# Play addendum

r[cz.play.actor-poll+1]

- Each actor must always check its input channel at a quick pace at the start of its loop

r[cz.play.actor-drain+1]

- Each actor must fully drain its channel when anything is there

r[cz.play.latest-wins+1]

- each actor must immediately prioritize the most recent work over previous work (exception: the headgroup must ingest all unique new tiles. Neither dropping tiles nor getting behind are acceptable.)

# TPS addendum

r[cz.perf.home-100tps+1]

r[cz.perf.home-10000tps-gpu+1]
For the home view at default res, Even when on CPU, average tps must >= 150. When on GPU, tps must >= 3000.

# Oracle addendum

r[cz.math.perturbation-naive-oracle+1]

To check perturbation implementation is correct, exact answer parity is required with a trusted naive implementation at home view and several well known sites of interest.

The trusted naive implementation oracle goes suchly:
- compute point answer at N bits precision
- compute point answer at 2N bits precision
- same? -> done. differ? -> double again.

Note that the entire answer is compared. not merely the result.

# Boundary tracing/Infill addendum

To check infill / boundary tracing speed and period detection in a hard case, zoom into the neck at -0.75 + 0i.

# TPS addendum

TPS for view is measured as the average TPS when filling the whole view.
GPU TPS is required to follow FLOPS performace, eg 20X a single CPU core. On a different machine, this may differ.

# Headgroup

Perforamnce north star:

r[cz.perf.headgroup-stable-path+1]
- One path: shader does the same things every frame. there must be no frametime change when panning vs stationary.

r[cz.perf.headgroup-shaders-2ms+1]
Headgroup shaders together must hit 2ms frametime at all times even at 1080p. See north star.

r[cz.perf.headgroup-vsync+1]
Headgroup hybrid window actor rate must have vsync enabled to prevent using all the GPU. do Not do some janky manipulation to force the framerate, simply enable vsync and let egui handle it.

r[cz.ctrl.zoom-in-homothety+1]
Zoom in must result in the point stencil homothery following this exact transformation:
magnification += 1
locations: subtract pointer location, divide by two, add pointer location

The sampling shader must always take the same path: see north star.

Scroll *up* must correspond to zoom *in* which is an *increase* of magnification pot by one.
(In egui, scroll up is a positive scroll delta.)
r[depends cz.ctrl.scroll-up-zooms-in+1]




