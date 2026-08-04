# Unit-design decisions (developer Q&A)

Recorded from the unit-design closing pass. These override assistant guesses in this tree. They do **not** override authoritative root/`docs/design` content; where they contradict authoritative text, flag and ask.

## Coloring & settings

| Id | Decision |
|----|----------|
| D-COLOR-1 | Default script = requirements list only: escape-time layer; in-filaments black; out-filaments colored as outside with ∞ escape time; nothing else. |
| D-COLOR-2 | Layer fields: source field, normalization, colorizer, base color, inside opacity, outside opacity. |
| D-COLOR-3 | Layers combine by alpha-over in script order; later layers paint on top. |
| D-COLOR-4 | Filament and node highlights are layers in the same ordered list (participate in painting order). |
| D-BAIL-1 | Changing bailout radius recolors from the stored escaping z; never changes inside/outside membership; no rework. |

## Shading

| Id | Decision |
|----|----------|
| D-SHADE-1 | In-filament “hard inversion” threshold: constant; assistant picks by experiment and marks **assumed**. |
| D-SHADE-2 | Node/minibrot smallness threshold: constant; assistant picks by experiment and marks **assumed**. |
| D-SHADE-3 | Out-filament = any period change between neighbors; paint **only the higher-period side** of the edge. |

## Memory / tile manager

| Id | Decision |
|----|----------|
| D-MEM-1 | Bump size = exactly what screen + lookahead requires; no headroom. |
| D-MEM-2 | Bump is visible: settings slider moves to the new value. |
| D-MEM-3 | Tile cost = packed answer bytes from the encoding. |
| D-MEM-4 | Hoard equality = same tile-manager function + same inputs (stencil + limit) ⇒ same keep-set on both sides. |

## Scheduling

| Id | Decision |
|----|----------|
| D-SCH-1 | Lookahead column = tile containing the mouse, at each successive magnification, depth-first, down to 8 bumps. |
| D-SCH-2 | Magnification velocity = EWMA of bumps per second over recent input. |
| D-SCH-3 | Higher-preference intratile phase may interrupt a lower one **immediately** (suspend mid-job). |

## Period / worker / reference

| Id | Decision |
|----|----------|
| D-PER-1 | Twin-test iteration count N: assistant picks by experiment, **assumed**. |
| D-PER-2 | Twin equality = relative epsilon scaled to the active gear’s precision. |
| D-PER-3 | Loop detector = power-of-two iteration-count snapshots (GPU-friendly). |
| D-GEAR-1 | Mid-tile gear escalation is **not a design path**: a gear sufficient to discriminate screen resolution is sufficient for iteration. |
| D-SERIES-1 | Series approximation is **in scope now** (design + implement). |
| D-CANCEL-1 | Tile cancelled because it left the screen: keep partial calibrated work in the hoard (resume if it returns). |
| D-REF-1 | Reference precision = requisite precision for point discrimination **plus 20 bits**, as written. |
| D-REF-2 | Superseded reference dropped when last user finishes/cancels, **or** when live reference count exceeds N (N **assumed** until set). |

## Publisher / stencil / UI

| Id | Decision |
|----|----------|
| D-PUB-1 | Publisher cadence while incomplete is **[20, 100000] Hz** (20 Hz refresh floor, max 100k TPS); idle when complete (0). GPU shader path is required. Matches auth `architecture.md` + `tile_publisher.md`. |
| D-PUB-2 | Calibrated→answer bias: clamp the proximate value into the proven range, field by field. All answer fields treated as numeric (no non-numeric special case). |
| D-PUB-3 | Publisher is **continuity of output only**. Tile completion and TPS must **not** depend on the publisher. |
| D-PUB-4 | GPU-native bypass uses the **same idea as CPU**: worker sends **calibrated tile** → publisher biases with **proximate sampled** hoard tile → Answer. Bypass = GPU-resident calibrated tile (no upload copy); not a different protocol. |
| D-STEN-1 | Stencil carries, beyond homothety + resolution: mouse position, magnification velocity, and a sequence number. |
| D-WORK-1 | Workgroup keys hoarded work by **tile address only**; stencil expresses current desire, not a work key. |
| D-UI-1 | Apply stays enabled even when the coordinate field already equals the current viewport location. |

## GPU-native completion (2026-08-03 closing pass)

Closes the observation half of `cz.pub.gpu-native-work` for home GPU TPS. Does **not** override auth; records developer answers from the delivery loop. Criterion matches CPU; path differs only in where Answers live and how completion is observed.

| Id | Decision |
|----|----------|
| D-GPU-1 | **Completion criterion (CPU and GPU):** a tile is complete when **every point has escaped or repeated**. Same meaning on both paths. |
| D-GPU-2 | **Full Answer / point-buffer readback is banned** on the GPU-native hot path — it breaks GPU nativity and cannot be the home-GPU TPS path. |
| D-GPU-3 | **Completion observation = on-device counter (option A).** A per-tile GPU completion counter (or equivalent) is authoritative for “how many seats are done.” Host may read **only** that tiny signal (not Answers) to learn tile completion / fire TPS. |
| D-GPU-4 | **Counter accuracy invariant:** a seat bumps the completion counter **iff** that seat’s terminal calibrated outcome (escaped or repeated) has been committed to the GPU **calibrated** tile buffer. Prefer same shader/pass as that store; use a per-seat done bit so only the first 0→1 transition counts. Target count = valid seats (edge tiles may be &lt; 4096). Cancel/left-screen discards counter + slot together — no TPS. Publisher still turns calibrated → Answer (D-PUB-4); do not have the worker write headgroup Answers as the completion event. |
| D-GPU-5 | Host seat bitsets may drive **scheduling** (what to arm next) but are **not** completion authority on the GPU-native path. Bout fence alone must not declare seats complete. |
| D-GPU-6 | **Parallel multi-tile on GPU is allowed** (2026-08-03 Jonathan). Multiple production-atlas slots / tiles may be in flight at once. Keep the **same interface**: per-tile on-device counter (D-GPU-3/4), calibrated-tile → publisher bypass (D-PUB-4), no Answer readback (D-GPU-2). Host scheduling may still prefer tenacious focus, but that is not a GPU concurrency ban. |

## Assumed numeric placeholders (pending experiment)

| Id | Value | Notes |
|----|-------|-------|
| A-SHADE-INFIL | Neighbor escape-time slope-angle delta > π/2 counts as hard inversion | Replace after visual experiment |
| A-SHADE-NODE | `min_magnitude` below tile point-spacing (one pixel in complex space at that mag) counts as node seed | Replace after visual experiment |
| A-PER-TWIN-N | 20 twin-test iterations | Code constant `PERIOD_CONFIRMATION_ITERATIONS`; was listed 16 before experiment locked 20 |
| A-REF-MAX-N | Max live references = 3 (current + up to 2 retained) | Replace if PO sets N |
