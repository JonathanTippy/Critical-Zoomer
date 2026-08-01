The tile publisher must sit between the gpu uploader and the heagroup.

The tile publisher actor must use a GPU shader to combine hoarded tiles with the new tile to fill in agnostic areas from proximate data. Note that this is the actor responsible for satisfying "no agnostic" rule. Actors before this point are calibrated & honest about knowledge. After this point, best effort answers so the headgroup can think only in terms of complete tiles of answers.

r[cz.int.publisher-nores-bias+1]

r[cz.range.guess-biased-nearest+1]

It does so by checking the proximate data, and editing it only if it has now beed disproven by bounds. If it is disproven, it chooses the closest value in the bounds to the old proximate data, to optimize for visual continuity. This is the definition of how calibrated answers are converted to answers. THe process requires a "bias" in the form of an answer.
I could say "if no proximate data, emit nores" and that is likely how it will end up working, though conceptually, nores is just the natural continuation of proper dynamic res proximate data handling.
Regardless of how its conceptualized, the tile publisher must always publish nores when no proximated date is at hand.

Definition of nores: nores is not a special case. it is literally just what result you get when you put infinity throug hthe system. the shaders in the headgroup will not be able to distinguish it from other answers.

r[cz.int.publish-cadence+1]

The publisher publishes at [20, 100000] Hz.
