# V2V QC score (assistant-owned)

Evaluated 2026-08-01 against V2V skill 1.4. Evidence-only.

## Dimension notes

### Spec — 8.1 (A)
Versioned `r[cz.*+N]` tags across `docs/requirements.md`, `docs/standards.md`, and `docs/design/*.md`. Tracey loads 77 rules. Auth chrome sometimes defers to requirements (“consult requirements”) rather than a full `docs/spec/` tree. No recorded formal red-team log.

### Tracey — 8.0 (A)
`tracey query status`: **76/77 covered**, **0 untested**, **1 uncovered** (`cz.perf.home-10000tps-gpu`, soft-skip / blocked-impl). Stale: none. Bacon Tracey job uses `tracey query validate`. `ImplInTestFile` noise remains in validate output.

### Coverage — 6.8 (C+)
Fresh `cargo llvm-cov` (ignore GUI shells; skip standards hard-bars): **region 71.39%** (938/1314), line 68.55%, function 68.43%. HTML at `target/llvm-cov/html/html/index.html`. Pipeline works after fixing `--summary-only`/`--html` clash. Far from S (≥98%).

### Properties — 7.2 (B)
Proptest present on IntExp, Range, shade GPU↔oracle, e2e_oracle, naive_cpu symmetry, tile_session helpers. Not “majority of correctness logic” at 1000+ cases everywhere.

### SubsystemsDesign — 7.0 (B)
Clear actor split (headgroup / workgroup / publisher). Several production files exceed the skill’s ideal ~1200 lines (`tile_session.rs` ~2001, GPU workers, `gpu_display/mod.rs`).

### DeliveryPipeline — 7.8 (B+)
Bacon jobs (check/test/coverage/mutants/tracey), CI workflow, headed e2e suite with freeze gate, taskset CPU discipline. Bacon Tracey → `tracey query validate`. Coverage script produces HTML + TOTAL. Fresh e2e suite OK on 2026-08-01.

### Fuzz — 6.2 (C)
Fuzz targets exist (`fuzz_coords_parse`, `fuzz_publisher_clamp`, `fuzz_range`, …). Nightly/`cargo fuzz` not exercised this pass (stable toolchain rejects `-Z`).

### Mutants — 5.8 (D+)
`range.rs` scoped run finished (2026-08-01): **178** mutants — **109 caught**, **43 missed**, **26 unviable** (~72% kill on viable). Survivors cluster on `min`/`max`, `is_agnostic`, comparison helpers (`can_*` / `must_*`), `guess_biased`, `get_uuid`. See `docs/assistant-docs/mutants-survivors.md`.

### SystemsDesign — 7.4 (B)
`docs/architecture.md` + design docs + SteadyState actor graph + perturbation/foveation story are coherent and performance-aware. Auth tile_publisher ≥30/s wording still stale vs D-PUB-1 (non-blocking).

## Weighted result

Baseline (60%): (8.1+8.0+6.8+7.2+7.0+7.8)/6 = **7.48**  
S-tier (40%): (6.2+5.3+7.4)/3 = **6.30**  
Combined: 0.6×7.48 + 0.4×6.30 = **7.01** → **B**

## Gate

QC requires **B (≥ 7.0)**. Scored **7.01** at evaluation time.

**DAT (2026-08-01): failed.** See `docs/assistant-docs/issue-stack.md` § DAT failures.

**V2V Tier: B (7.0)**  
**Strongest areas:** Spec, Tracey, DeliveryPipeline  
**Most urgent gaps:** DAT failures (issue-stack); mutants kill-rate; coverage gaps  
**Recommended next actions:**  
- Triage DAT items in issue-stack; fall back to implementation phase per delivery.md.  
- Finish `range.rs` mutants; raise coverage on publisher/gpu paths.
