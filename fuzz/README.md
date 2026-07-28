# Fuzz targets (coverage-guided via libFuzzer / cargo-fuzz)

Requires nightly + `cargo fuzz`.

```bash
taskset -c 4-11 cargo +nightly fuzz run fuzz_target_1 -- -max_total_time=60
```

`fuzz_target_1` exercises `IntExp` add commutativity and shift/`<<`/`>>` exponent rules.
