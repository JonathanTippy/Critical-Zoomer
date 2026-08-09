# `scripts/` — Xvfb screenshot check only

**Policy (2026-08-08).** This directory exists for **one** product purpose: run
the app under an isolated Xvfb display, capture a PNG, and let the assistant
inspect that image. It must not grow into a second test suite.

## Allowed here

| File | Role |
|------|------|
| `xvfb_screenshot_check.sh` | **The** entry point: release binary → isolated Xvfb → settled home PNG under `/tmp` (or a caller-supplied out dir). |
| `cz_ctl.sh` / `cz_ctl_lib.sh` | Private harness for that entry point (start/stop/send/capture). Not a place to hang new product checks. |
| `README.md` | This policy. |

## Forbidden here

- Checked-in PNGs, PPMs, or other capture artifacts (write under `/tmp` or a
  gitignored out dir; never commit them into `scripts/`).
- Shell “e2e suites,” performance/fitness probes, coverage, fuzz, mutants,
  precision-wall navigators, or other stand-ins for `cargo test` / `cargo bench`.
- New `.sh` files “just this once” for something that can be a Rust unit,
  integration, or Criterion benchmark.

## Where work belongs instead

- **Correctness / craftsmanship / GPU smoke** → `cargo test` (`src/**`, pinned
  craftsmanship tests).
- **Performance / IPS / FLOP probes** → `cargo bench` (`benches/`).
- **Visual corroboration after render-affecting edits** → build release, run
  `xvfb_screenshot_check.sh`, **Read the PNG** (`docs/assistant/manual-testing.md`).

## History

The previous scripts zoo (dozens of `e2e_*.sh`, harness selftests, and ~27
checked-in PNGs) was moved to
`docs/assistant/Trash/scripts-sprawl-2026-08-08/`. Do not revive it without an
explicit developer request.
