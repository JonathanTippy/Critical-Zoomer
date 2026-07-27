Responsibilties

Actually does iterations on tiles.

input: tile address
output: tile<CalibratedAnswer>

- The tile worker must tenaciously complete its current tile before starting the next.
- The tile worker must determine, on stencil update and reference update, whether to use cpu or GPU and which precision 'Gear' to go into.
- The tile worker must default to spiralling in from the outer edge of the tile. The provides plenty of points for the GPU to work on even in just one tile.

CalibratedAnswer:
contains the relevant statistics for rendering as they are progressing in the form of ranges.
- escape time & z when escaped
- period
- in/out
- small time (time to min magnitude)
- smallness (min magnitude)
- escape time slope angle (used for in filament detection)

Incidentally, Answer contains the same values, but they arent ranges.


C generator:

The c generator attempts to initialize for a given type and stencil and fails if the type can't distinguish all the stencil's points.
If it succeeds, generating c values takes a fast path of adding the screen loacation times the point space to the screen's base locatiton, avoiding all intexp operations.
It does so by working relative to a given reference point, so only the delta's precision is required.

Gears:

gears are really just types. There must be 12 gears (in stack memory) and one adaptive gear (in heap memory) for tile work.
smaller types are preferred for their speed.
types:
- f32 (cpu & gpu) (fastest, most preferred)
- f64 (cpu only)
- i32 & i32exp (cpu & gpu) (stack)
- i32 & i32 & i32exp (cpu & gpu) (stack)
- i32 & i32 & i32 & i32exp (cpu & gpu) (stack)
etc up to 8x i32 integer significand
- [i32;N] & i32exp (cpu only) (stack, array)
- rug float (cpu only) (only heap gear, least preferred)

Yes, this will require a lot of typing. And?

gpu must be preferred to cpu


Actual iteration:

always use standard perturbation math. include derivative (for angle determination). Apply series approximation skip when catastrophic absorption holds.

Detect glitches using the standard test, that is, when |Z + z| << |Z|:
fall back to a const 'big z = 0' orbit (correct 0 case). This should naturally result in the c generator yielding more precision requirements on little z, changing the gear to result in more precision being used for little z. glitch handling is done exclusively by the tile worker and it does not notify the reference worker.
