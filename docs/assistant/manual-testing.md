# Assistant-owned manual testing

This procedure is mandatory after changes that can affect rendering, numerical
classification, navigation, scheduling, shaders, or display timing. “Manual”
means the assistant runs and inspects it. The developer is never expected to do
these steps.

## 1. First look: run, capture, inspect

1. Build the current source in release mode using nice priority and the center
   half of the CPUs:

   ```bash
   taskset -c 3-8 nice -n 15 cargo build --release
   ```

2. Prove the isolated capture harness before trusting it:

   ```bash
   CZ_CPUSET=3-8 taskset -c 3-8 nice -n 15 scripts/harness_selftest.sh
   ```

3. Run the app only on an isolated Xvfb display. Use a unique
   `CZ_SESSION_PREFIX`/capture directory, keep `CZ_ALLOW_REAL_DISPLAY` unset, and
   use `scripts/cz_ctl.sh` or an existing `scripts/e2e_*.sh` consumer. Never use
   the developer’s desktop, `DISPLAY=:0`, or a desktop screenshot API.

4. Capture a settled home frame before relying on aggregate assertions.
   Readiness means observable Mandelbrot structure (center/side variance and,
   when available, cropped baseline similarity on consecutive stable frames).
   Do **not** treat “few gray holes” or a fixed sleep alone as settled — a
   transient flat purple frame has zero gray holes and must be rejected as
   not-ready until structure appears or the timeout expires:

   ```bash
   CZ_CPUSET=3-8 taskset -c 3-8 nice -n 15 \
     scripts/capture_naive_baseline.sh /tmp/cz_manual_home
   ```

5. Read the generated PNG directly and inspect it as an image. Confirm that it
   is recognizably the expected Mandelbrot view, not merely “non-black”,
   “non-gray”, or statistically varied. Look specifically for:

   - coherent cardioid/bulb boundaries and expected real-axis symmetry;
   - no giant circles, rectangles, seams, uniform unfinished blocks, or holes;
   - no false black interior or false escaped regions;
   - no stale-frame mixing after navigation;
   - progressive convergence rather than a frozen plausible-looking frame.

   A suspicious screenshot fails this step even if scripted image statistics
   pass.

## 2. Scripted headed regression

Run the visual suite against the freshly built release binary:

```bash
rm -rf /tmp/cz_manual_visual
CZ_CPUSET=3-8 CZ_BIN="$PWD/target/release/critical_zoomer" \
  taskset -c 3-8 nice -n 15 \
  scripts/e2e_visual.sh /tmp/cz_manual_visual
```

Inspect every staged PNG (`vis_home_final.png`, `deep_early.png`,
`pan_post.png`) directly. Script success is necessary but not sufficient.
Script failure is a regression until explained and fixed.

## 3. Targeted interaction

For changes affecting navigation or depth, use the existing isolated scripts to
capture at least:

- home after settle;
- one zoom-in and zoom-out round trip;
- one pan;
- one difficult/deeper location, early and after settling.

Compare the captures for continuity and structural correctness. Numerical truth
comes from the arbitrary-precision oracle; screenshots corroborate that the
truth reaches the display intact.

## 4. Evidence and cleanup

- Record the exact release binary, commands, capture directory, and observed
  result in the work summary.
- Read capture files before deleting them.
- Always stop the isolated session, including on failure. The harness scripts
  install cleanup traps; use `scripts/cz_ctl.sh stop` for manual sessions.
- Never ask the developer to perform this procedure.
