# `scripts/`

Assistant-owned helpers. Not a second test suite. No checked-in PNGs.

| File | Role |
|------|------|
| `full_check.sh` | Lock-step: cargo check, tests in pyramid order (unit → integration → e2e) on debug+opt-3, all Criterion benches, Tracey. Log `/tmp/cz_full_check.log`. |
| `screenshot_check.sh` | Isolated Xvfb → settled home PNG under `/tmp`. |
| `screenshot_session.sh` / `screenshot_session_lib.sh` | Private start/send/stop for that screenshot check. |
| `coverage.sh` | llvm-cov region report. |
| `mutants.sh` | scoped cargo-mutants (unit tests only; `.cargo/mutants.toml` applies the same `--lib` filter to raw `cargo mutants`). |
| `README.md` | This. |

Do not add e2e shell suites or commit capture artifacts. Prefer `cargo test` / `cargo bench` for product checks; `full_check.sh` is the one wrapper that runs those.
