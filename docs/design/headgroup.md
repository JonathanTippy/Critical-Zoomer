headgroup contains shaders

The headgroup is a single actor for speed / pacing reasons. Its responsibility is to be extremely responsive to user changes, so it has to have a stable framerate and good feeling controls.

It must recieve tiles from the workgroup, and heap them into its tile collection.
Each frame, it must sample its collection to construct a frame of answers, then shade those answers and display them. both shaders must be extremely fast.

To avoid wasting GPU working time, the fps of the headgroup must be limited to 60fps.

The headgroup must have a settings struct containing all the required settings

The headgroup handles its location by using a stencil.
The stencil is transformed when zooming by using a homothety.
The headgroup also stores a location for drag in full intexp for dragging then zooming out then zooming back in.
When zooming out, the actual screen stencil always loses one bit of precision per mouse bump.
The hybrid window actor must properly handle controls for the preceeding frame by accounting for elapsed time and debt gaps:
elapsed time for pan and debt gaps for scroll zoom.
The basis for fast and exact location storage is intexp, which is a rug integer glued to an i32 for the exponent. It is used for all homotheties across the program.

Consult requirements for specific shader filters and chrome bits and bobs.

hud elements: egui text element, top left
other buttuns and such: widgets, top right
settings: widgets, deferred view, opened by gear button
shaders: layer colors according to coloring script in the settings struct. should be straightforward and lightweight.
settigns screen:
widgets for each setting, drag-n-drop area for layering of coloring layers, and can select each to configure.

off screen = r=2 circle is off screen
mostly off screen = r=2 circle is within 10% to being fully off screen
too small = r=2 circle is 1px or smaller
mostly too small = r=2 circle is smaller than 10% of screen

