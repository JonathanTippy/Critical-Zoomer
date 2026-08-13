# WorkUpdate payload (standard)

`WorkUpdate` is **not** parameterized by the iterate type. Iterate (`CopyIntExp`,
f64, …) stays inside the worker. Completions that leave the worker are already
narrowed to the publish standard:

| Field kind | Width | Why |
|---|---|---|
| Recontinuable `z` (and other later-step orbit state) | **f32** | Escape already happened; `|z|` is around bailout (~2). Later shade/escape steps do not need the iterate tape. |
| Scalars (`smallness`, times, period, …) | **f64** | Counts and magnitudes, not neighbor `c`. |
| Seat `c` | **not emitted** | Not required for rendering. Color uses escape time / interior / smallness. The worker must not send `start_location` / `c`. |

`Mandelbrotable` exposing `Into`/`to` f32 and f64 is **this** conversion (z → f32,
scalars → f64), not “the collector iterates f64.”

WIP vs published is a hard boundary: unfinished seats stay on the worker tape;
Dummy on the collector grid is “no answer yet,” not a low-precision `c`.

**Live mismatch (2026-08-13):** `WorkUpdate<T>` / `CompletedPoint<T>` still carry
`start_location` and `work_update_to_f64` maps every `T` through `to_f64`. That
is not the standard. Do not use that interlayer as the headed-grey RCA (c is
not a render input). Main-spike recontinuation at the bailout circle is a
separate unsolved issue.
