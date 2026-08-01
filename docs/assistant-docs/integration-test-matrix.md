# Integration test matrix (assistant-owned)

Assembly contracts from `docs/tracey/integration-rules.md`. Phase gate: ≥3
meaningfully different passing assembly verifies per `cz.int.*` id.

| Contract | Assembly verifies (≥3) | Status |
| --- | --- | --- |
| cz.int.stencil-retarget+1 | pan keeps ref; zoom resets; attention/mag_velocity; minigraph | green |
| cz.int.publish-cadence+1 | incomplete→idle; complete idles; max-Hz / no-work gate | green |
| cz.int.publisher-nores-bias+1 | bias kept through ingest; clamp disproven; NORES outside | green |
| cz.int.memory-bump+1 | required bump; apply bump; protected never pruned | green |
| cz.int.hoard-ingest-sample+1 | pan keys; sparser reject; NORES outside; minigraph | green |
| cz.int.session-pipeline+1 | zoom-in lookahead; zoom-out scredge; progress; mag match | green |

Phase gate: **green**. Next: end-to-end testing.
Auth tile_publisher ≥30/s wording still pending Jonathan (non-blocking; D-PUB-1).
