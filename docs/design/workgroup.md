Responsibilities:

completes work in the best order and best speed possible for the user, while outputting continuously high-quality, continuous, frequent answer tile updates.

Ranges must be used to keep track of in-progress work.
For example, in WIP points,
- Some lower bound of the escape time is known.
- Some lower bound of min magnitude time is also known
- The escape location is known to be somewhere in the ring between circle r=2 and circle r=6 (2^2=4+2=6)
- Some min magnitude upper bound is known

more details on this in the tile worker.

Sub-actors:

- tile scheduler (foveated scheduling + lookahead, sends tile addresses to worker)

- intratile scheduler (boundary tracing / filling scheduling & vetoing.)

- tile worker (must have a default schedule of spiralling into the tile from the outer edge which it follows unless the intratile scheduler has provided more information.)

- gpu uploader (when bypassed, publisher is still used)

- tile publisher (combines hoarded data with most recent work & publishes the best possible tile to the headgroup.)

- reference worker


layout:

                            reference owkrer
                                      |
tile scheduler                           |
\/          /----------------------------/
tile worker > intratile scheduler
            <
\/\----------------\
gpu uploader         |
\/                   |
tile publisher    <---/
