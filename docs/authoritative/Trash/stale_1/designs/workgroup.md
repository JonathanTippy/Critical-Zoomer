THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.
THE ASSISTANT MAY NEVER USE THE ACRONYMS INDICES OR MARKERS IN THIS FILE AND MUST SPEAK IN PLAIN ENGLISH.

# actor modules

- WorkCore
- Integrator<T>
- HoarderPublisher<T>

### I/O

WorkCore: RX<View<()>>, TX<PointUpdate<CalibratedAnswer>>, TX<PointUpdate<()>>, TX_Bundle<BitmapUpdate>
Integrator<T>: RX<PixelStencil>, RX<PointUpdate<T>>, RX<BitmapUpdate>, TX<View<T>>
HoarderPublisher<C, T>: RX<PixelStencil>, RX<View<C>>, TX<View<T>>

# actor threads

- WorkerCore (W)
- Integrator<CalibratedAnswer> (WI)
- HoarderPublisher<CalibratedAnswer, Answer> (WHP)
- Integrator<()> (UI)
- HoarderPublisher<(), ()> (UHP)

# actor layout diagram:


             Headgroup
            /  |  |   \
           /   |  |    \
          \/   \/ \/   \/ 
        UI   UHP   WI   WHP



        WI -> WHP ---------> Shadergroup
        ^
        |
        W -> UI -> UHP
        ^            |
        *------------*



# actor responsibilities desc:

### WorkCore

The workcore does needed work & prioritizes missing areas.

The workcore retains the most recent unit view, mostly for the use of its bitmap.
The workcore owns a screen-sized vec of points in progress.
It edits its retained unit view bitmap when points get completed, 
and uses it to avoid working on already completed points.
When a point is completed, the seat + answer + stencil serial no. is sent, and a seat + unit + stencil serial no.
When a new view is recieved, the retained view is swapped out.

### Integrator

The integrator takes a continuing stream of T and turns it into a usable view.

The integrator builds views from the T from the worker and does sparse remaps to produce a view with the most recent work and the most urgent stencil which is then sent to the T hoarder publisher.
In order to do so quickly enough it will require sparse viewport filling to apply transforms to small amounts of T quickly. quicker move -> more transforms + less work done. 
If sparse sampling scales sufficiently well, the integrator can run at a similar speed even with rapid movements.

### Hoarder Publisher

The hoarder publisher transforms the current view of T while combining it with the latest T and any retained old T.

The hoarder publisher retains the current frame of T, remaps it on transforms, combines it with old T & new T, and publishes the best T view available to its TX.
It also combines new calibrated answers with retained answers; the workercore produces calibrated answers, and the publisher makes a guess, biased towards the previous value of that location, to convert those into answers.

### Notes

 a lot of what the integrator and publisher do is the same, because they both aim for the same moving target. The important differentiator is the timescale: the integrator works on a closer to worker timescale, while the publisher is quite a bit slower.
The integrator makes sense of a fast-moving stream of points for different stencils, while the hoarder/publisher builds the final View<Answer>. The integrator takes single points while the publisher takes views.

The workcore can trust the new unit view and swap out its bitmap; besides, even if its slightly outdated, the workcore doesn't have time to perform the remap which checking would necessitate.

## Mandelbrotable Trait

The workcore needs a mathematical trait allowing various algorithms to be run using various types.

