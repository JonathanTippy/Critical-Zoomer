Two shaders: sampling shader, and shading shader.

sampling shader runs first and constructs a full frame of answers. shading shader determines the colors based on current settings.

shading shader phases:
- edge annotation
- escape
- layered coloring

The sampling shader must be both perfect and fast. This entire rendering pipeline is based on the fact that limited precision does not actually limit precision if the only values you actually care about are on the dyadic grid.
