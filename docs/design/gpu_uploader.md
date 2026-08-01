This actor must work based on standerd steady state practice:
wakes at some minimum rate or when a new tile is in the queue, works through the queue by uploading the data in the tiles to the gpu and emitting gpu tiles, differing only in that their data is native to te gpu memory instead of the cpu memory.
bypass when GPU native, don't pass through noop work.

### GPU Uploader

Input: Tile<CalibratedAnswer>
flow per second: [0, 100000] (20Hz refresh rate, max 100k TPS.)
Output: GPUTile<GPUCalibratedAnswer>
flow per second: [0, 100000] (20Hz refresh rate, max 100k TPS.)

0 rate because may be bypassed.

uploads tiles to the GPU.
May be bypassed if the worker did the tile natively on GPU.
