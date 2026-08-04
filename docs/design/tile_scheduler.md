The tile scjeduler must order tiles based on magnification velocity: that is, the rate at which magnification is changing.
If positive, the user is zooming in: focus on foveated lookahead.
If zero, the user is stationary. foveated screen fill. also lookahead.
If negative, the user is zooming out: focus on low res backtracking.

When working foveated, spiral out from mouse pointer.
when doing foveated lookahead, do depth-first single tile column down to desired depth ( 8 bumps).
When hoarding old work, save 1-4 tiles which cover the screen for each magnification back to 8 bumps past.
