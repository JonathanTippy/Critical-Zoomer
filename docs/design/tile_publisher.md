The tile publisher must sit between the gpu uploader and the heagroup.

The tile publisher actor must use a GPU shader to combine hoarded tiles with the new tile to fill in agnostic areas from proximate data. Note that this is the actor responsible for satisfying "no agnostic" rule. Actors before this point are calibrated & honest about knowledge. After this point, best effort answers so the headgroup can think only in terms of complete tiles of answers.
It does so by checking the proximate data, and editing it only if it has now beed disproven by bounds. If it is disproven, it chooses the closest value in the bounds to the old proximate data, to optimize for visual continuity. This is the definition of how calibrated answers are converted to answers. THe process requires a "bias" in the form of an answer.
I could say "if no proximate data, emit nores" and that is likely how it will end up working, though conceptually, nores is just the natural continuation of proper dynamic res proximate data handling.
Regardless of how its conceptualized, the tile publisher must always publish nores when no proximated date is at hand.

memory policy: evict hoarded work to stay within memory limit. If the screen itself and its lookaheds take up more than the limit, bump the limit. communicate with headgroup to achieve this. The code which does this must be part of the tile collection manager, shared code between the headgroup and workgoup which ensures the collection of tiles considered in play is the same between the groups. There is no size difference as both groups store answers.

The publisher publishes at max 1000/s but at least 30 per second, depending on ease of current work.
