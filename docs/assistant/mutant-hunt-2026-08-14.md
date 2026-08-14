# Mutant hunt (2026-08-14)

**Status:** active. **Ghost hunt PAUSED** (developer 2026-08-14) — resume from
`ghost-hunt-2026-08-12.md` when this hunt ends or developer redirects.

**Baseline run** (31h wall): 10942 tested — 5509 caught, **4307 missed**, 1006
unviable, 120 timeouts. Artifacts: `mutants.out/` (do not delete mid-hunt).

**House oracle:** `scripts/mutants.sh` + `.cargo/mutants.toml` (unit tier,
`--skip integration_tier` / `e2e_tier`, profile `mutants`).

**Strategy:** workgroup/shadergroup files dominate missed count; many are
charter-heavy. Grind house numeric files first (`range.rs`, `utils.rs`,
`floatexp.rs`), then expand scope deliberately. Document equivalent mutants
(e.g. `min` strict `<` vs `<=` on ties) instead of fake pins.

**Work stack:** `work-stack.md` (assistant queue; this file is hunt log only).

## Loop ticks

| Tick | Swath | Outcome | missed (guess) |
|---|---|---|---|
| 0 | Baseline + pause ghost | Hunt opened; stack in `work-stack.md` | **4307** |
| 1 | `utils` + `range` MUST_INTEGER + `floatexp` add | Pins landed; house mutants re-run in `mutants.out/tick1-house.log` | pending |
