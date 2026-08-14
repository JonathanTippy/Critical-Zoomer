# `scripts/`

Assistant-owned helpers. Not a second test suite. No checked-in PNGs.

| File | Role |
|------|------|
| `zoomer_groove_check.sh` | **Zoomer-groove standard checker** (skill § Checker Script): tracey check → cargo check → unit → integration → e2e → manual screenshot. Log `/tmp/cz_groove_check.log`. Target `/tmp/cz_groove_cargo_target`. Park hook: `.cursor/hooks/groove_check_on_stop.sh`. |
| `full_check.sh` | CZ extension after groove tier: pipeline cadence + Criterion benches + tracey re-validate. Skips groove if stop hook just ran it. `/tmp/cz_full_check_cargo_target`. |
| `screenshot_check.sh` | Isolated Xvfb → settled home PNG under `/tmp`. |
| `screenshot_session.sh` / `screenshot_session_lib.sh` | Private start/send/stop for that screenshot check. |
| `coverage.sh` | llvm-cov region report. |
| `mutants.sh` | scoped cargo-mutants (unit tests only; `.cargo/mutants.toml` applies `--lib` skip + `profile = "mutants"` so raw `cargo mutants` does not opt-3). |
| `README.md` | This. |

Do not add e2e shell suites or commit capture artifacts. Prefer `cargo test` / `cargo bench` for product checks; `full_check.sh` is the one wrapper that runs those.
