THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


## IO

The workcore is the part of the workgroup which does the work.
It expects a View<()> (stencil and bitmap).
The reason for this is that it needs to know:
1. what screen (location, magnification, resolution) is required
2. what work is already done / proximately represented

It will not know the exact values of this already done work; it won't be able to use it for scheduling algorithms, but it will know that it is already done so to not bother doing it.
This is acceptable because these known and/or proximated points almost always constitute either an already complete block or sparsely scattered single points, and they can be considered to be "outside of the screen" for edge following purposes.

## Responsibilities

The workcore will take two phases:
1. complete only work which was neither proximate nor exact
2. complete work which was proximate but not exact
It will not complete work which was exact, as that work is already done.


