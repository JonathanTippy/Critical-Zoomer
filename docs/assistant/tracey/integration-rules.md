# Integration Tracey rules (assistant-owned assembly contracts)

Atomic rules for cross-unit / channel contracts. Normative product text remains in
`docs/authoritative/requirements.md` / design; these tags exist for Tracey linkage of assembly verifies.
Unit verifies alone do not satisfy these ids.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). Every acceptance block in this
> file cited tile-machine machinery (tile sessions, tile publisher, GPU tile ingest, sampling
> context); those symbols are gone. The **product intent** of each rule is restated in v0.0.9
> terms below, with the golden-mechanism reference in `docs/assistant/design/workgroup-virtues.md`.
> Checked boxes from the tile era are cleared; each rule needs re-verification against the
> restored assemblies (`workgroup/` = screen worker + work controller + work collector).

r[cz.int.stencil-retarget+1]

**Normative summary.** Headgroup stencil changes drive workgroup retarget: pan keeps
references; zoom rebuilds mag-sensitive state; attention and mag_velocity propagate.

**Acceptance criteria (v0.0.9 terms).**
- [ ] A new `PointStencil` reaches the work controller and becomes a single `Replace` command
  to the screen worker (coalesced — only the latest target matters).
- [ ] The work collector remaps the previous package through the shared
  (center_x, center_y, mag) transform on every retarget — pan and zoom alike — before new
  work lands on top (`work_collector.rs` `sample_old_values`).
- [ ] Attention (click/drag point) reaches the worker's queue ordering.
- Revert note: v0.0.9 has no per-mag reference binding; "zoom rebuilds mag-sensitive state"
  is just the remap + new iterate epsilon. Pan-keeps-references becomes a depth-era rule again
  when perturbation returns (see suspended decisions in `docs/assistant/unit-design/decisions.md`).

r[cz.int.publish-cadence+1]

**Normative summary.** Incomplete work publishes promptly and continuously; when the
stencil is complete, publish cadence goes idle.

**Acceptance criteria (v0.0.9 terms).**
- [ ] While incomplete, every ingested workshift produces a whole-snapshot publish — cadence
  is the workrate, with no gate, timer, or batch window (virtues doc: "single publish path").
- [ ] When the screen is complete, the worker stops finding seats and publishing goes quiet.
- Revert note: the tile-era numeric band was a patch over batching jitter. v0.0.9's bar is
  simpler and stronger: latest work is always already published (regressions-in-progress are
  impossible by construction).

r[cz.int.publisher-nores-bias+1]

**Normative summary.** Do not rebuild a single calibrated-answer biasing system
(tile-era proximate clamp). The product target is **PPS through the publish
path**. Bias would feel better if the pipe were slow; the gain is too small to
pay for the complexity. When PPS is already flowing, bias would not be noticed.
Unset seats stay `Dummy` (outside-looking), never invented Inside. Provisional
edge answers stay provisional.

**Acceptance criteria (v0.0.9 terms).**
- [ ] Remap sampling (`sample_old_values`) carries old answers into the new frame;
  unset pixels remain `CompletedPoint::Dummy{}` (outside-looking), never interior black.
- [ ] Provisional screen-edge answers are overwritten by real work, never frozen as final.
- Do not add a calibrated→answer bias shader/pass: not a substitute for PPS, and not
  worth the complexity for a slow-pipe nicety.

r[cz.int.memory-bump+1]

**Normative summary.** When protected work alone exceeds the memory limit, raise the limit
rather than pruning current/lookahead work.

**Acceptance criteria.**
- **Suspended at v0.0.9**: the truth store is one screen package plus one hoard package;
  there is no pruning path and nothing to bump. The product rule stands for any future
  multi-screen hoard (collected-wisdom: memory floor depends only on screen size; on-screen
  work is never evicted).

r[cz.int.hoard-ingest-sample+1]

**Normative summary.** Hoard ingest keeps continuity across pan, rejects sparser
replacements, and placeholder answers never sample as set-Inside.

**Acceptance criteria (v0.0.9 terms).**
- [ ] Zoom-out restores the kept previous package instead of recomputing (one hoard slot,
  restored through the same remap transform).
- [ ] `Dummy` placeholders never render as Inside anywhere in the sample/shade path.
- Revert note: tile-era absolute-key ingest and sparser-reject logic are gone; the v0.0.9
  hoard is deliberately a single previous screen (virtues doc: "one package"). Multi-slot
  hoarding is future work and must extend this discipline, not replace it.

r[cz.int.session-pipeline+1]

**Normative summary.** The workgroup pipeline (controller → worker → collector) honors
latest-target-wins, tenacious completion, and honest-incomplete display across
pan / zoom-in / zoom-out / resize.

**Acceptance criteria (v0.0.9 terms).**
- [ ] Pivot mid-shift: worker finishes its current ≤10ms bout, then takes the replace;
  no work is computed for a dead target afterward.
- [ ] Every seat on a live target eventually gets iterated to completion (bailout or loop);
  no pixel is starved by queue ordering (hard-seat rotation).
- [ ] Pan, zoom-in, zoom-out, and resize each leave the collector with exactly one coherent
  package id — no mixed-target output is ever publishable.
- Revert note: this rule replaces the tile-era session pipeline contract wholesale; the
  mechanisms cited above are the golden ones documented in `docs/assistant/design/workgroup-virtues.md`.
