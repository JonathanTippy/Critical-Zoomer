choose:
upper left corner of screen (point stencil homothety location)
build:
rug float with requisite precision for point discrimination plus 20 bits
bind:
each stencil has one reference orbit wich is the latest one computed; if the new one is not ready yet, no matter, jsut use the old one.
work already started with the old reference prevents it from being discarded. new work is started with the new reference.

update trigger: when point stencil homothety magnification changes. pan should be insignificant.

actual iteration: store all stack types in the produced reference. include the term for series approximation.

reference orbits are delivered to the tile worker.
