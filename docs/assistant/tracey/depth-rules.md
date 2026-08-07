# Depth core rules

These rules cover the isolated perturbation core. Actor/workgroup integration remains a
design gap; see `../design/depth-design.md` and `../issue-stack.md`.

r[cz.depth.c-generator-fails-closed+1]

**Rule.** Objective-coordinate conversion is admitted only when the target compute type keeps
every adjacent screen point distinct. Adjacency is checked at both axis ends (including the
max-magnitude end where float ulp is worst), and generated coordinates reproduce v0.0.9's
top-left, no-half-pixel grid exactly. Relative generation subtracts the reference in exact
IntExp before narrowing.

**Implementation.** `src/assemblies/workgroup/c_generator.rs` — `Mandelbrotable`,
`CGenerator::new`, `new_relative`.

**Verification.** `generator_matches_v009_grid_bit_for_bit`,
`rejects_collapse_at_far_end`, `successful_generator_has_distinct_neighbors`,
`relative_generator_subtracts_before_narrowing`.

r[cz.depth.floatexp-range+1]

**Rule.** Per-pixel deltas and stored reference iterates use a normalized f64 mantissa plus
i64 exponent. Values far below f64's exponent floor remain nonzero; arithmetic agrees with
high-precision rug arithmetic to stored-mantissa precision. Zero has correct ordering against
arbitrarily small positive/negative values.

**Implementation.** `src/floatexp.rs` — `FloatExp`, `ComplexFloatExp`.

**Verification.** `add_and_multiply_agree_with_rug`,
`does_not_underflow_far_beyond_f64`, `zero_is_canonical_and_exact`,
`deep_delta_runs_without_f64_underflow`.

r[cz.depth.reference-low-storage+1]

**Rule.** Reference iterates are computed in depth-appropriate rug precision but stored as
floatexp. Exactly one high-precision tail state is retained so extension never recomputes.
Proven periodic/preperiodic references index indefinitely by wrapping their finite cycle;
escaping/nonperiodic references refuse unknown indices.

**Implementation.** `src/reference.rs` — `ReferenceOrbit::{compute,extend,get}`,
`bits_for_zoom`.

**Verification.** `stored_orbit_matches_full_precision_rounding`,
`extending_matches_one_shot`, `periodic_and_preperiodic_orbits_index_forever`,
`escaping_reference_is_finite_and_honest`.

r[cz.depth.perturb-never-wrong+1]

**Rule.** Delta iteration implements Δz' = 2ZΔz + Δz² + Δc. Missing reference work,
loss-of-significance at the bailout circle, and Pauldelbrot glitches return
`Unfinished`/`Glitch`; they never become guessed Mandelbrot answers.

**Implementation.** `src/perturb.rs` — `iterate_pixel`, `PerturbedOutcome`.

**Verification.** `perturbation_matches_precision_doubling_oracle_for_exteriors`,
`missing_reference_is_unfinished_not_wrong`, `exact_reference_matches_naive_escape_time`.

r[cz.depth.oracle-doubling+1]

**Rule.** The test-only naive oracle starts with enough rug bits to represent the dyadic input
exactly, then doubles precision until two answers agree. Starting at a fixed low precision is
forbidden: two insufficient precisions can agree only because both erased the same deep bit.
A concluded oracle must be matched exactly or the perturbation path must report itself
honestly incomplete.

**Implementation/verification.** `src/perturb.rs` test module —
`doubling_oracle`, `perturbation_matches_precision_doubling_oracle_for_exteriors`,
`deep_delta_runs_without_f64_underflow`.
