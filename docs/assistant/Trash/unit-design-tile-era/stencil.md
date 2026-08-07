# New unit: Stencil

No authoritative unit file. Closes architecture “Stencil” assembly-API gap. Non-authoritative.

## Role (UD-STEN-ROLE-1) — inferred

Headgroup → workgroup desire message: which screen is urgent and how to prioritize.

## Fields (UD-STEN-FIELDS-1) — D-STEN-1

| Field | Meaning |
|-------|---------|
| Homothety | `(IntExp, IntExp, i32 mag_pot)` — top-left / location basis per dyadic conventions |
| Resolution | `(seats, rows)` — never W/H |
| Mouse position | Seat/row in stencil space (hover); used for foveation |
| Magnification velocity | EWMA bumps/s (D-SCH-2) |
| Sequence number | Monotonic; bumps on any stencil change |

## Send rules (UD-STEN-SEND-1) — inferred

Send when any field changes. Moving → up to ~60/s; still → 0 (architecture flows).

## Non-keys (UD-STEN-KEY-1) — D-WORK-1

Stencil is not a hoard key. Workgroup indexes tiles by address; stencil only retargets scheduling.
