# Scripts sprawl archive (2026-08-08)

Former contents of `scripts/`: shell e2e suites, harness selftest, coverage/fuzz/
mutants helpers, precision-wall navigators, and ~27 checked-in PNG captures.

**Why archived.** Developer direction: `scripts/` may only support the isolated
Xvfb screenshot check; correctness belongs in Rust tests; performance in
Criterion benches. See live `scripts/README.md`.

Do not restore these as the default verification path without an explicit
developer request.
