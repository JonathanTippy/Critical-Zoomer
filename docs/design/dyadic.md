Note: uses old "view" term, new design is tiles. dyadic principles remain the same, and must guide the implementation of the sampling shader.


// Conventions:
// location.2 is magnification which is not the precision exponent.
// magnification goes up as you zoom in.
// Usually, when seat and row go in a tuple together, the order is seat then row.
// This is to align better with the x then y standard order.
// W/H and Width / Height are banned. This project uses seats and rows,
// and anytime both dimensions are together, a tuple called resolution.

// The stencil defines the set of complex points that make up a view.
// It is used with a vec equal to resolution.0 * resolution.1 in length.
// The top left sample of the view is taken exactly at location.0, location.1
// Other samples apply a regular grid,
// following imaginary coordinates: down is negative and right is positive.
// In complex plane terms: +seat moves +real; +row moves −imag.
// Scanning map between vec and pixels is done right then down, like a CRT.
// The points are equally spaced vertically and horizontally.
// The default points per unit is defined by the PIXELS_PER_UNIT_POT constant.
// The zoom level (location.2) is added to the constant to get the current PPU POT.
// The actual spacing distance between points is given by 1/(2^(PPU POT)).

// When filling one View from another, pixels are considered to represent:
// the area from their top left corner (inclusive) to their bottom right corner (limit).
// inexact mappings of larger to smaller are thusly fully defined.
// Optionally, the inexact (less important) values can be determined with a half-offset to
// closer approximate the nearest value and mitigate visual layout shift.

// Importantly, exact values are maintained and checked so that there are always some exact plotted pixels.
// This way, the results are "pixel imperfect": 2x zoom looks the same as a shift right,
// but greater zooms follow the rule that exact pixels don't represent an area,
// but a perfect plotted point. inexact pixels are filled best-effort.
// The best known algorithm for this is nearest with top left bias.
// A .5px bias will be present for the whole frame, which is easily accounted for and not visually noticeable.
// EDIT: unproven; likely to introduce a small error.
// Shelfed this concept because fill_from must yield the same result for a 4x zoom and two 2x zooms for example;
// functions which combine views must be associative.

// The complex plane is effectively divided into squares
// , where every smaller & larger pair where larger contains smaller can map small (choose top left) -> large or large (top left) -> many small

// The method to find at least one exactly mapped pixel if one exists is to check:
// 1. overlap (do the frame areas touch at all?) -> overlapping corner(s)
// 2. compatibility
// (does the relative offset contain units smaller than the smaller space? if so, no exact matches.)
// EDIT: no longer true; precision is folded in. each frame has a precision mapping its magnificaoitn level,
// so if there is overlap, there is compatibility.

// mapping is exact when one mapped exact pixel is identified,
// and the larger pixel step off of that pixel yields pixels still represented in the smaller pixel view.

