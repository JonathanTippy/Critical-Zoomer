# Phase 6 — additive numeric types (assistant notes)

Non-authoritative. Batch F locked in `he-said/batch-F.md`. Batch G defaults in `he-said/batch-G.md`.

## Landed

- `StackedIntExp<const STACKS>` in `src/stacked_intexp.rs` — `[i64; STACKS]` + shared `i32` exp; add/sub/shift with limb carry; mul via IntExp boundary; `From`/`Into` IntExp; default `STACKED_INTEXP_STACKS = 4`
- `FloatExp` in `src/floatexp.rs` — thin `f64` mantissa + `i32` exp; Add/Sub/Mul; `Mandelbrotable` impl
- `RugReferenceFloat` type alias (`rug::Float`) stub for reference-build path wiring
- Screenspace use of StackedIntExp forbidden until PO approval (Batch F.3)

## Remaining

- Full schoolbook mul staying on-stack (`i128` carry across wide limbs) without IntExp detour
- Rug `Float` reference-orbit build path (alias only; not wired into `ReferenceCollection` yet)
- `f32+i32` FloatExp variant for later GPU parity
- Any screenspace StackedIntExp opt-in (blocked by Batch F.3)
