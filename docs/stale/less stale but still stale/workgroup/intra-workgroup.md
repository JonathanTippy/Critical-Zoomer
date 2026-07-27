# Workgroup breakdown

## Workcore

The workcore is the part which completes actual iterations on tiles.

Input: Tile<Answer> (from work tile master)
rate (incomplete): 60
rate (complete): 0
purpose: scheduling starts from proximate answers

Output: Tile<CalibratedAnswer> (to work hoarder publisher)
rate (incomplete): 60
rate (complete): 0

Input: TileRequest

## Publisher

responsible for publishing (combining proximate data with new work to continuously improve a tile)
this does imply maintaining its own collection of tiles.

Input: Tile<CalibratedAnswer>

Output: Tile<Answer>

Input: TileRequest

## Worktilemaster

responsible for scheduling tiles

Input: Tile<Answer> (from work publisher)
rate (incomplete): 60
rate (complete): 0
purpose: scheduling starts from proximate answers

Input: PointStencil (from headgroup)
rate (moving): 60
rate(still): 0

Output: TileRequest (homothety + seat address)

input: edgeresult (enum: has out, not has out)
