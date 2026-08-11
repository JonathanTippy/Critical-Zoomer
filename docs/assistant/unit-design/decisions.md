# Unit-design decisions (developer Q&A)

Recorded from the unit-design closing pass. These override assistant guesses in this tree. They do **not** override authoritative `docs/authoritative/` content; where they contradict authoritative text, flag and ask.

> **Do not implement from suspended tile-era rows.** Memory / tile manager,
> Scheduling (mag columns), tile-keyed work, and tile publisher topology below
> are **archive of Q&A** — not a live design target. Live GPU/depth work follows
> `docs/assistant/design/workgroup-virtues.md`, `naive-gpu-design.md`, and
> `docs/assistant/quality-doctrine.md`. Tile-era prose that used to live under
> `design/Stale/` is in `docs/assistant/Trash/design-Stale-tile-era/`.

> **2026-08-06 post-revert status.** The codebase is back at v0.0.9 (e6a0560); the tile machine is gone from the live tree. Sections below split:
> - **Standing (product/process, still binding):** Design fallbacks rule; Coloring & settings; Shading; D-UI-1; D-PER-3/4/6 as algorithm preferences (v0.0.9's every-iteration loop check is consistent with them); D-REF precision principles for when perturbation returns; **Naive GPU view port** (`docs/assistant/design/naive-gpu-design.md`) and the D-NGPU-* rows below.
> - **Suspended (tile-machine-specific):** Memory / tile manager; Scheduling (tile columns); D-STEN-1 fields beyond v0.0.9's PointStencil; D-WORK-1 (tile keys); Publisher topology D-PUB-2..6 as *tile* topology; most of the GPU-native completion block as *tile* wording; D-PLAY-TICK as written (v0.0.9's ~10ms wall-clock workshift is the golden cadence — see `docs/assistant/design/workgroup-virtues.md`; re-derive any tick design from it). Policies that still apply to the view port are restated under D-NGPU-* / `naive-gpu-design.md`.
> - Any GPU/depth re-implementation must satisfy the seven invariants in `docs/assistant/design/workgroup-virtues.md` before its own design concerns.

## Design fallbacks (standing rule — 2026-08-04 Jonathan)

Contingent alternatives recorded in this file (cross-tile WIP, period two-pass, on-device intratile, single GPU conductor, multi-tile serial break, etc.) are **design fallbacks**, not mechanical/runtime switches.

- Keep them as options the assistant may **suggest** when evidence appears.
- Do **not** oversuggest them; prefer implementing and tuning the **default** path.
- **None may be implemented without explicit approval.** Measurement alone does not authorize a switch.

**Exception — required, not a fallback:** in-fill two-phase field work (D-GPU-11): move-on without min-magnitude/small-time, then catch-up those fields. That is forced by boundary-tracing optimization.

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
| D-PER-3 | **Loop equality every iteration (2026-08-04 Jonathan).** Contender check runs **every iteration** as in the prescribed algorithm (not POT-only). Cost is almost always one or two extra comparisons beside the escape check. Auth still allows tortoise-and-hare or POT as detector *family*; live choice is every-iteration equality + twin-test. |
| D-PER-4 | **Integrated period on the main path (2026-08-04 Jonathan).** Prefer **one** iterate path: loop check + twin-test run with ordinary work (auth: no separate stalling period phase). When sure of interior but not of period, calibrated may emit **in, period unknown**; certain period only after twin-test. Period work must not delay escape / play / publisher notify of partial truth. Hypothesis: full period determination is only slightly slower than in-determination alone. |
| D-PER-5 | **Period two-pass fields = design fallback only.** Membership/escape/angle then period/small-time *as a period strategy* — suggest with evidence; **no impl without approval**. **Not** the same as required in-fill two-phase (D-GPU-11). |
| D-PER-6 | **GPU twin-test adaptation.** Branching twin-test is allowed on GPU; when contenders are sparse, compact them and run confirm on that set so the main bout stays uniform. Never invent a period. Period-edge trace / same-period fill may use **certain** periods only; unknown-inside remains valid for early veto. |
| D-GEAR-1 | Mid-tile gear escalation is **not a design path**: a gear sufficient to discriminate screen resolution is sufficient for iteration. |
| D-SERIES-1 | Series approximation is **in scope** (design + implement). Production remains deferred until membership pins + performance contract (`r[cz.depth.series-approximation+1]`). |
| D-SERIES-2 | **Always on (2026-08-11 Jonathan).** No enable/disable / worth-it branch. When a covering reference is published, SA runs as part of seat init. Shallow no-op must be nearly free so branching is pointless. |
| D-SERIES-3 | **Skip is init, not bout work (2026-08-11 Jonathan).** Discovery must be so cheap it is free: binary search over the orbit (O(log N) evals). Easy cases ≈ one access per point. Must not steal iterate-bout budget; that framing does not apply. |
| D-SERIES-4 | **Win = deep zoom (2026-08-11 Jonathan).** Large skips are the payoff. Overhead when skip is useless must stay in the noise vs seat start / tick feel. |
| D-SERIES-5 | **Coeffs fused into reference (2026-08-11 Jonathan).** One series step per reference iterate in the same loop/worker — not a separate process. Extra math only as necessary; no extra big-O beyond the recurrence; airtight performance mindfulness from the start (v0.0.9 hot-path habits). |
| D-SERIES-6 | **Gear promote ≠ restart (2026-08-11 Jonathan).** Mid-orbit gear promotion does not restart seats or re-run series init. Restart stays on reference generation / glitch / unbound paths. |
| D-CANCEL-1 | Tile cancelled because it left the screen: keep partial calibrated work in the hoard (resume if it returns). |
| D-REF-1 | Reference precision = requisite precision for point discrimination **plus 20 bits**, as written. |
| D-REF-2 | Superseded reference dropped when last user finishes/cancels, **or** when live reference count exceeds N (N **assumed** until set). |

## Publisher / stencil / UI

| Id | Decision |
|----|----------|
| D-PUB-1 | Publisher cadence while incomplete is **[20, 100000] Hz** (20 Hz refresh floor, max 100k **publish Hz**); idle when complete (0). GPU shader path is required. Publish Hz ≠ TPS. Matches auth `architecture.md` + `tile_publisher.md`. |
| D-PUB-2 | Calibrated→answer bias: clamp the proximate value into the proven range, field by field. All answer fields treated as numeric (no non-numeric special case). |
| D-PUB-3 | Publisher is **continuity of output only**. Tile completion and TPS must **not** depend on the publisher. |
| D-PUB-4 | GPU-native path: worker exposes a **GPU-resident calibrated tile**; publisher binds it **directly** (uploader bypass) and biases with proximate → GPU Answer. Same calibrated→Answer idea as CPU; residency differs, protocol does not. Uploader exists only for CPU-completed calibrated tiles. |
| D-PUB-5 | **Publisher wake = notify (2026-08-04 Jonathan).** After a bout commits GPU calibrated updates, the worker notifies the publisher (Steady State message/event). Publisher does not rely on polling a silent timer alone to discover new calibrated work. Cadence [20, 100000] still caps how fast it may run; notify is the wake source. |
| D-PUB-6 | **Publisher topology (2026-08-04 Jonathan).** Publisher sits after the **calibrated source**: CPU path = uploader → publisher; GPU-native = worker → publisher (bypass). Auth “between uploader and headgroup” is read as the CPU path; bypass is also auth. |
| D-STEN-1 | Stencil carries, beyond homothety + resolution: mouse position, magnification velocity, and a sequence number. |
| D-WORK-1 | Workgroup keys hoarded work by **tile address only**; stencil expresses current desire, not a work key. |
| D-UI-1 | Apply stays enabled even when the coordinate field already equals the current viewport location. |

## GPU-native completion (2026-08-03 closing pass)

Closes the observation half of `cz.pub.gpu-native-work` for home GPU TPS. Does **not** override auth; records developer answers from the delivery loop. Criterion matches CPU; path differs in residency and how completion is observed (calibrated on GPU; Answers from publisher).

| Id | Decision |
|----|----------|
| D-GPU-1 | **Move-on done / TPS (2026-08-04 Jonathan):** A seat is **move-on done** when all fields are determined **except** the in-fill exception: **min-magnitude and small-time need not be present** when in-fill applied those seats — the worker **must** be allowed to move to the next tile. **HUD TPS fires on move-on done** (tile complete when every valid seat is move-on done), **not** after Phase 2 min-magnitude/small-time catch-up. Per-bout partial calibrated writes (D-GPU-7) are not move-on done. **min-magnitude / small-time remain required work** in Phase 2 (auth). Same CPU/GPU meaning. |
| D-GPU-2 | **Full calibrated / Answer / point-buffer readback is banned** on the GPU-native hot path — it breaks GPU nativity and cannot be the home-GPU TPS path. |
| D-GPU-3 | **Completion observation = on-device counter.** A per-tile GPU completion counter is authoritative for how many seats are **move-on done** (D-GPU-1). Host may read **only** that tiny signal to learn tile completion / **fire TPS**. Phase 2 catch-up does not re-fire TPS for the same tile. |
| D-GPU-4 | **Counter accuracy invariant:** a seat bumps the completion counter **iff** that seat is **move-on done** (D-GPU-1) and that outcome has been committed to the GPU **calibrated** tile buffer. Prefer same shader/pass as that store; use a per-seat done bit so only the first 0→1 transition counts. Target count = valid seats (edge tiles may be &lt; 4096). Cancel/left-screen discards counter + slot together — no TPS. Publisher turns calibrated → Answer (D-PUB-4); worker must not write headgroup Answers as the completion event. |
| D-GPU-5 | Host seat bitsets may drive **scheduling** (what to arm next) but are **not** completion authority on the GPU-native path. Bout fence alone must not declare seats complete. |
| D-GPU-6 | **Tile tenacity = serial by default (2026-08-04 Jonathan).** Finish the current tile’s **move-on** work before starting the next (auth tile_worker), subject to D-GPU-1 in-fill exception (may leave before min-magnitude/small-time). **Design fallback:** multi-tile GPU in flight only if serial is **conclusively proven untenable**. Suggest with evidence; **no impl without approval**. |
| D-GPU-7 | **Per-bout calibrated commit (2026-08-04 Jonathan).** After every bout, **every progressed seat** writes updated **GPU-resident calibrated** state (ranges / partial truth). Publisher is notified and consumes that buffer GPU-natively (D-PUB-4/5). Device write, not host download. Fresh partial truth so hard cases do not look stalled. This is **not** move-on done (D-GPU-1). Counter bumps only when a seat becomes done enough (D-GPU-4). |
| D-GPU-8 | **Dense WIP, same-tile refill (2026-08-04 Jonathan).** Do not keep iterating seats that are already done enough. After a bout, remove those seats from WIP and replace slots with not-yet-done seats **from the same tile**. WIP is separate from the calibrated tile buffer; refill must not erase committed calibrated seats. Refill a WIP slot only after that seat’s done-enough commit + counter bump (D-GPU-4). **Default:** same-tile only. **Design fallback:** cross-tile WIP refill — suggest with evidence; **no impl without approval**. |
| D-GPU-9 | **Spiral on GPU; intratile CPU control plane (2026-08-04 Jonathan).** The GPU executes the default spiral and dense WIP refill on its own. Intratile scheduler may remain on CPU as a **control plane**: indices, phase, and do/don’t — not point payloads, not a per-bout chaperone. **Design fallback:** on-device intratile — suggest with evidence; **no impl without approval**. |
| D-GPU-10 | **GPU IPS bar — superseded for the view port (2026-08-08 Jonathan).** Classic wording was GPU IPS ≈ **20 × best CPU IPS** as a stand-in for one hardware class. **Current bar:** measure CPU single-core FLOPs and GPU total FLOPs on the same machine; GPU IPS / CPU IPS must track that FLOP ratio within about **±20%** on iterate-heavy full-stack naive work. See D-NGPU-5 and `docs/assistant/design/naive-gpu-design.md`. Auth absolute billions-IPS figures remain unresolved/suspended until the port exists. |
| D-GPU-11 | **Two-phase field work forced by in-fill (2026-08-04 Jonathan).** Boundary-tracing / in-fill optimization **requires** a two-phase design: **Phase 1** — determine membership, period (when certain), escape, angles, etc., apply in-fill, allow **move on** to the next tile without min-magnitude/small-time on in-filled seats (D-GPU-1). **Phase 2** — compute **min-magnitude and small-time** (and any other still-missing stats) on seats that still need them, including in-filled points (auth). This is **default / required**, not a design fallback. Distinct from period two-pass (D-PER-5), which remains fallback-only. |
| D-GPU-IDEA-1 | **Single workgroup GPU conductor (design fallback / idea).** Recorded for suggestion only. **Do not implement without explicit approval.** Steady State actor graph remains the plan. |
| D-PERF-HOME-1 | **Home TPS (2026-08-04 Jonathan).** Auth addendum CPU ≥150 / GPU ≥3000 is the hard bar. Older “~100 TPS” home line was approximate; not a competing requirement. |
| D-PLAY-TICK | **Sync play tick (2026-08-06 Jonathan).** Pipeline stays **fully sync**. Tick period ≈ **50 ms**. Each tick runs an **exactly bounded** iteration (or bout/seat work-unit) count **N**, with **N adapted from a GPU speed probe** (IPS-class): weak GPUs do less work per tick, not longer ticks. Explicit fence / on-device completion Wait **inside** the tick is allowed so work cannot silently queue behind. **Payload readback remains banned** on the GPU-native hot path (D-GPU-2); host syncs only tiny completion signals. Same period on 1× and 200× GPU FLOPS — continuous visible output every tick. Non-auth; tensions with live 1 ms `PLAY_INPUT_POLL_MS` / wall-time `workshift_budget_ms`. |

## Naive GPU view port (standing — 2026-08-08 Jonathan)

Authoritative prose: `docs/assistant/design/naive-gpu-design.md`. First GPU compute
path on the restored view machine (not tiles).

| Id | Decision |
|----|----------|
| D-NGPU-1 | **API / adapters:** wgpu compute; standard Linux support (Vulkan/Mesa etc.). No NVIDIA-only requirement. |
| D-NGPU-2 | **Precision:** F32 is the always-on GPU baseline. If the adapter offers `SHADER_F64`, adapt and use an F64 pipeline as a precision gear; otherwise remain on F32. Prefer F64 for honesty of representation, not as a speed default. |
| D-NGPU-3 | **Authority stays serial; arithmetic goes wide.** One live view / `LiveTarget`, drain-to-newest, pivot flush-then-announce, provisional ≠ final, wall-clock interruptible shifts. Parallelism is inside a `BoutCap`-bounded **wave of seats** behind the kernel seam — not a second live world or a multi-view default. |
| D-NGPU-4 | **Queues = host control plane.** Existing scheduling queues/attention arm **indices** into dense WIP; they do not stream point payloads and are not expected to dominate the FLOP→IPS bar when waves are wide. Hot-path killers to avoid: tiny WIP, full buffer readback, per-seat sync chaperoning. Host arming is not completion authority. |
| D-NGPU-5 | **IPS tracks measured FLOPs (±20%).** Measure CPU single-core FLOPs and GPU total FLOPs; require `IPS_gpu / IPS_cpu ≈ FLOPs_gpu / FLOPs_cpu_single` within about ±20% on iterate-heavy full-stack naive runs. Supersedes the fixed “≈20×” wording of D-GPU-10 for this port. Measure ratio on iterate-heavy work; treat shallow-flood scheduling fraction as a separate overhead/play check. |
| D-NGPU-6 | **Residency:** keep progressed/calibrated view state on device across bouts; ban full point/calibrated/answer readback on the hot path. Host may read only tiny done signals. |
| D-NGPU-7 | **Play on slow GPUs:** keep user-felt shift cadence; scale work units per shift from a speed probe. Do not “catch up” by lengthening blocking GPU ownership of the worker. |

## Assumed numeric placeholders (pending experiment)

| Id | Value | Notes |
|----|-------|-------|
| A-SHADE-INFIL | Neighbor escape-time slope-angle delta > π/2 counts as hard inversion | Replace after visual experiment |
| A-SHADE-NODE | `min_magnitude` below tile point-spacing (one pixel in complex space at that mag) counts as node seed | Replace after visual experiment |
| A-PER-TWIN-N | 20 twin-test iterations | Code constant `PERIOD_CONFIRMATION_ITERATIONS`; was listed 16 before experiment locked 20 |
| A-REF-MAX-N | Max live references = 3 (current + up to 2 retained) | Replace if PO sets N |
