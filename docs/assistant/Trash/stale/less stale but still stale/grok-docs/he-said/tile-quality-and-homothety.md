# Tile quality / scheduling / storage (2026-07-18)

## Quotes

> Some quality issues before moving forward. Either fix now or note down to fix later, when it makes sense:
> 1. tile order is simple index order (should spiral outward from cursor
> 2. tiles do not use boundary tracing which makes them slow
> 3. tiles have black line visual glitch
> Given that you have added the visual script, I would have expected you to catch at least no. 3.

> Good, also tiles will allow progressive refinement to combine with foveated rendering and lookahead, yielding a three-path tile scheduling policy, but thats probably for later so fix 2 and 3 and maintain your notes.
> Also, looking at your code, your tiles are not on the stack. As only one is managed at a time, and the size is const, no reason not to use array.
> Also, your tiles are apparently screen-children, which might be how it needs to be for now, but the desired design is that tiles of equal zoom magnitude share a homothety, maxing out at a total of 8 homotheties because of storage space.

Status / follow-through: `../tile-status.md`.
