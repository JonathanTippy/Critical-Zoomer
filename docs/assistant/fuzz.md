# Fuzz targets (V2V Fuzz)

Coverage-guided targets under `fuzz/`. Mutants campaigns are separate — do not
fight `mutants.out/` with these builds.

```bash
# requires: cargo install cargo-fuzz  (nightly)
cargo +nightly fuzz run coords_goto -- -max_total_time=60
cargo +nightly fuzz run admission_edges -- -max_total_time=60
```

| target | surface |
|---|---|
| `coords_goto` | `goto_line_is_valid` / `commands_from_goto_line` / `parse_complex` |
| `admission_edges` | `admit_generator` / `pick_stack_admission` |

No panics is the bar. Invalid inputs returning `None`/`false` is success.
