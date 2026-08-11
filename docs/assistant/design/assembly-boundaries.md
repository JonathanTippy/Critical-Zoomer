# Assembly boundaries (binding)

Status: **enshrined 2026-08-11 interview** — separation of concerns is what makes
the project workable. Do not blur these jobs to win a local optimization.

## Who does what

| Assembly | Job | Not its job |
|---|---|---|
| **Workgroup** | Produce **answers** (escape / period / fields needed for paint) under never-stall workshifts | Coloring, bailout-radius animation, window IO |
| **Shadergroup** | Short bailout **tail** + convert answers → **colors** (escaper then colorer) | Mandelbrot membership grind, stencil ownership |
| **Headgroup** | Present colors (sample/upload/draw), stencils/goto/HUD, settings UI | Re-deriving answers; inventing a second color math |

The headgroup “also produces stencils” is real but beside the paint point: once
answers exist, headgroup responsibility for the image is **transforming /
displaying colors**, not recomputing the set.

## Why this is load-bearing

Crossing these lines is how the project falls into slop: a “faster colorer”
that changes results, a worker that paints, a window that re-iterates. The
year of manual v0.0.9 work encodes hundreds of design decisions that are not
all listed in docs — **study that code** before rewriting. New work must live
up to that quality; interviews and virtues docs capture what can be stated,
but the old implementation remains the richest source of tweaks.

## Color gear (when GPU color exists)

Mirror the screen-worker **manual gear** pattern: a settings switch between

- **OG colorer** — today’s CPU `color` path (default / golden look)
- **GPU colorer** — future honest rewrite with feature parity + tests

Automatic gearbox must not silently replace OG with GPU. Manual select for
compare/rollback. See `shadergroup-virtues.md` parity bar.
