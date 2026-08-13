# Assistant-owned manual testing

This procedure is mandatory after changes that can affect rendering, numerical
classification, navigation, scheduling, shaders, or display timing. “Manual”
means the assistant runs and inspects it. The developer is never expected to do
these steps.

**Scripts policy:** `scripts/` holds `full_check.sh`, `screenshot_check.sh`,
coverage, and mutants (see `scripts/README.md`). Correctness and performance
live in `cargo test` and `cargo bench`. Do not add new shell e2e suites.

## 1. First look: build, capture, inspect

1. Build the current source in release mode using nice priority and the center
   quarter of the CPUs:

   ```bash
   taskset -c 4-11 nice -n 15 cargo build --release
   ```

2. Run the isolated Xvfb screenshot check. Keep `CZ_ALLOW_REAL_DISPLAY` unset.
   Never use the developer’s desktop, `DISPLAY=:0`, or a desktop screenshot API.
   Write PNGs under `/tmp` (or another out dir) — never commit them into
   `scripts/`.

   ```bash
   CZ_CPUSET=4-11 taskset -c 4-11 nice -n 15 \
     scripts/screenshot_check.sh /tmp/cz_manual_home
   ```

3. Read the generated PNG directly and inspect it as an image. Confirm that it
   is recognizably the expected Mandelbrot view, not merely “non-black”,
   “non-gray”, or statistically varied. Look specifically for:

   - coherent cardioid/bulb boundaries and expected real-axis symmetry;
   - no giant circles, rectangles, seams, uniform unfinished blocks, or holes;
   - no false black interior or false escaped regions;
   - no stale-frame mixing after navigation;
   - progressive convergence rather than a frozen plausible-looking frame.

   A suspicious screenshot fails this step even if scripted image statistics
   pass.

## 2. Targeted interaction (when navigation/depth changed)

Prefer Rust tests for numerical truth. When a headed frame is still needed,
use `screenshot_session.sh` only as support for `screenshot_check.sh` (or the same
harness with an explicit out dir under `/tmp`), capture home / one zoom / one
pan as needed, and inspect those PNGs. Do not revive deleted `e2e_*.sh`
suites.

## 3. Afterward

Stop the session, confirm no leftover `critical_zoomer` / `Xvfb` processes, and
leave no capture junk in the repo tree. Agent sessions also run
`.cursor/hooks/kill-test-zombies.sh` automatically before/after `cargo test`,
`cargo bench`, and `screenshot_check`, and on agent stop (see
`.cursor/hooks.json`). Manual sweep: `.cursor/hooks/kill-test-zombies.sh`.
