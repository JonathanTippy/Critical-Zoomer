# Integration Tracey rules (assistant-owned assembly contracts)

Atomic rules for cross-unit / channel contracts. Normative product text remains in
`docs/requirements.md` / design; these tags exist for Tracey linkage of assembly verifies.
Unit verifies alone do not satisfy these ids.

r[cz.int.stencil-retarget+1]

**Normative summary.** Headgroup stencil changes drive workgroup retarget: pan keeps
references; zoom rebuilds mag-sensitive state; attention and mag_velocity propagate.

**Acceptance criteria.**
- [x] Pan same-mag keeps bound reference; zoom changes mag and resets screen seats;
  attention / mag_velocity are applied on retarget (`integration_tests` assembly suite).

r[cz.int.publish-cadence+1]

**Normative summary.** Incomplete work publishes tiles in **[20, 100000] Hz**
(D-PUB-1; 20 Hz refresh floor, max 100k; aligns with architecture + `tile_publisher.md`);
when the stencil is complete, publish cadence goes idle (0).

**Acceptance criteria.**
- [ ] While incomplete, publishes stay within [20, 100000] Hz when work is ready; after complete, cadence idles
  (`tile_publisher::PublishCadence` + GPU publisher path + session assembly verifies).

r[cz.int.publisher-nores-bias+1]

**Normative summary.** Tile publisher converts calibrated answers with proximate bias
via GPU shader; disproven proximate → nearest-in-bounds; no proximate → NORES; never invents Inside
from empty proximate.

**Acceptance criteria.**
- [ ] Bias kept when in bounds; clamped when disproven; NORES when no proximate
  (GPU publisher + bias unit verifies).

r[cz.int.memory-bump+1]

**Normative summary.** When protected tiles alone exceed the memory limit, emit a bump
the headgroup can apply so the limit rises rather than pruning current/lookahead.

**Acceptance criteria.**
- [x] `required_limit_bump` / `apply_memory_bump` raise the limit; `SamplingContext::prune_distant_tiles`
  records the bump; protected tiles are never pruned for memory.

r[cz.int.hoard-ingest-sample+1]

**Normative summary.** GPUTile ingest into the headgroup hoard uses absolute keys,
rejects sparser replacements, and NORES never samples as set-Inside.

**Acceptance criteria.**
- [x] Ingest continuity across pan; sparser reject; NORES stays Outside through ingest
  (assembly + existing hoard verifies).

r[cz.int.session-pipeline+1]

**Normative summary.** TileSession pipeline (scheduler ↔ intratile ↔ worker ↔ reference)
honors mag-velocity order, tenacious tile completion, and reference pending bind rules
across pan / zoom-in / zoom-out.

**Acceptance criteria.**
- [x] Zoom-in starts lookahead column; zoom-out prefers scredge; active tile / progress
  persists across workshifts; published tile mags match stencil (`integration_tests`).
