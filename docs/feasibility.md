This document defends the theoretical plausibility of a mandelbrot stack which reaches deep while remaining fast-feeling.

There are three layers to address:
- screen location & magnification management & storage
- Mandelbrot function evaluation
- Work prioritization 
- Work storage

Of the layers, mandelbrot function evaluation is by far the most difficult. It leans on extremely strong work prioritization and work storage to enable continuous effort in a visually acceptable manner. It also leans on a paridoxical combination of tenacity (so results do eventually get done) and laziness / greediness (so that the user sees easy work get done quickly).

# screen location & magnification
This can be handled with a sufficiently precise value for location. even at comfortable zooming speed, it will grow quite large, however one quite large number is not hard for a modern computer to handle quickly. even a million bits still allows operations to be completed in under a millisecond, which I have verified in tests of rug integer.
How, then, to avoid duplicating this by every pixel?
use a homothety. The homothety defines the screen's location and zoom level, and the location can be shared by the points in the screen, so they can be addressed by coordinates which are made integer by the homothety.
By similar applications of big number + integer coordinates, the operations on the large number can be kept to a big O of 1.

# mandelbrot function evaluation
Perturbation has been accepted for a few years now. It is not magic, but the creator of superfractalthing said that it makes the time to render have more to do with the complexity of the image than the depth. Superfractathing is a living example that this is true, though it takes the less pleasant "drag a rectangle" UX pattern.

# work prioritization
Another shining example is "BROT", a perturbation based foveated renderer. It demonstrates that foveated rendering can enable an actually snappy and in fact almost perfect feeling mandelbrot browsing experience, in easier areas.

# work storage
Work must be stored not only acquisitively, but also recontinuably. That is to say, it must be possible to put down a piece of work and recontine it at a better time.

# what is out / in

out:
- perfect, fast performance even as areas get infinitely complex

in:
- perfect, fast performance in simple areas
- aggressive leverage of algorithms, storage, and compute
- A better experience than ever seen before in any one app

Conclusion:

perfect is not aimed for (yet) and may or may not be possible, but existing solutions all have gaps.
