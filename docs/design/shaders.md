Two shaders: sampling shader, and shading shader.

sampling shader runs first and constructs a full frame of answers. shading shader determines the colors based on current settings.

shading shader phases:
- edge annotation (checks escape time slope angle to find hard inversions which is where in filaments are. also does period & small time comparison for those edges (period edges are out filaments) minibrots / nodes are points where smallness approaches zero, also detected by hard angle inversion.)
- escape
- layered coloring

The sampling shader must be both perfect and fast. This entire rendering pipeline is based on the fact that limited precision does not actually limit precision if the only values you actually care about are on the dyadic grid.
