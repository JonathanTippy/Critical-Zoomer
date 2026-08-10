# Answer vs defaults — running record

Track whether PO answers match the assistant’s stated defaults, and whether a bare `defaults` would have carried the **same** information.

Columns:
- **Align?** yes / partial / no
- **`defaults` enough?** yes = same info as answering; no = answer added binding detail

| Batch | Q | Default stated | PO answer | Align? | `defaults` enough? | Notes |
|-------|---|----------------|-----------|--------|--------------------|-------|
| A | 1 tile edge | (asked open; plan later used pot=6) | `TILE_EDGE_LENGTH_POT` in `constants.rs` = 6, use const everywhere | — | no | PO specified name + mechanism, not just “64” |
| A | 2 Answer | keep existing Answer | keep; will need derivative-direction fields; GPU variant / 32-bit later | partial | no | Affirmed keep + forward requirements |
| A | 3 Phase 1 visibility | A temporary bridge (after clarify) | A if adapter explicitly temporary | yes | no | Added “explicitly temporary” constraint |
| B1 | 1 derivative | orbit multiplier along iterate | “thats fine” | yes | yes | Pure accept |
| B1 | 2 where first | replace/share workcore `PeriodicityDetector` | replace `PeriodicityDetector` | yes | yes | Same intent |
| B1 | 3 confirmation | short fixed N after candidate | const in `constants.rs`; **20** | yes | no | Fixed-N aligned; value + const placement are extra |

## Tallies (update each batch)

- Questions scored: **6** (A×3 + B1×3; A1 “align” left as — because default wasn’t a numbered choice)
- Align yes: **3** (B1.1, B1.2, B1.3 on shape; A3)
- Align partial: **1** (A2)
- Align no: **0**
- `defaults` would have been enough: **2** (B1.1, B1.2)
- Answer added detail beyond defaults: **4** (A1, A2, A3, B1.3)

Batch B (OrbitId / zero id / seek) still unanswered — score when answered.
