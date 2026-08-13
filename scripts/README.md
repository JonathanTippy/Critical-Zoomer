# `scripts/`

Assistant-owned helpers. Not a second test suite. No checked-in PNGs.

| File | Role |
|------|------|
| `full_check.sh` | The lock-step check: cargo check, full release tests, all Criterion benches, Tracey. Log `/tmp/cz_full_check.log`. |
| `xvfb_screenshot_check.sh` | Isolated Xvfb → settled home PNG under `/tmp`. |
| `cz_ctl.sh` / `cz_ctl_lib.sh` | Private harness for that screenshot check. |
| `coverage.sh` | llvm-cov region report. |
| `mutants_core.sh` | scoped cargo-mutants. |
| `README.md` | This. |

Do not add e2e shell suites or commit capture artifacts. Prefer `cargo test` / `cargo bench` for product checks; `full_check.sh` is the one wrapper that runs those.
