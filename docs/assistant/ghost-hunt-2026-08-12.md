# Ghost hunt (2026-08-12)

**Standing (developer 2026-08-13):** while this hunt is not done, after the
latest user ask is finished and its checks pass, **continue this hunt**
without waiting to be asked. Interruptions are expected. Rule:
`.cursor/rules/ghost-hunt-continue.mdc`. Loop body:
`ghost-grind-loop-prompt.md`.

Session: [v0.1 product direction interview](interviews/2026-08-12-v01-product-direction.md).
Post-v0.0.9 assistant notes that disagree with **live code** or with that
interview. Correct the note. Do not treat headed bugs as fixed.

| Ghost | Live truth | Action |
|---|---|---|
| `pipeline-refresh-rates.md` Status **implemented**; “never bare `request_repaint`” | `window/mod.rs` bare `request_repaint()` since `351afdf` (“preferred vsync code”) | Doc: content-tier cadence landed; **head present pacing not landed**. CPU open. |
| Issue-stack “OG remains default” under 1080p colorer | `resolved_color_gear()` defaults **GPU** | Strike OG-default line. |
| Issue-stack pipeline gap: `VSYNC=false` + “no code until big plan” | `VSYNC=true` in window; content timers exist; head still bare repaint | Align with live; head CPU still open. |
| “Shelved: worker parks so head CPU is fine” | Wrong actor. Window still immediate-repaint. Developer: 100% CPU at vsync rates. | Unshelve as open; worker park ≠ window idle. |
| Actor-layout interview parking lot “cadence not implemented / no code” | Historical; later code landed then `351afdf` reverted head pacing | Amendment on that interview, do not rewrite the transcript. |
| Depth “finish-line / precision wall green” | Blockiness + post-admit interlayer still open. Collector channel is still `WorkUpdate<f64>`; OG naive may iterate f32 then convert. | Do not call depth-trust done. |
| Dummy-head GPU esc ~60 Hz / “test currently passes” | Snapshot Hz. Pin is dummy-head GPU esc ≥40 / OG ≥15 on **debug+opt-3**. Unoptimized debug misses it; `--release` is not the gate. Headed still unchecked. | Honest `testing.md`. Do not ignore or lower floors. GPU grind paused. |
| Admit-margin “in tree” read as product fix | Mechanism only | Keep “product not verified”; add failure shape (C) post-admit drop. |

GPU compute/escape grind **paused** for v0.1 (interview). This hunt does not
resume it.

**Definition (interview 2026-08-12 evening):** a ghost is an **assistant
misunderstanding** in comments, docs, names, **or code** after v0.0.9.
Implementation ghosts are assistant mistakes while editing — harder to
find; git history is the hunt; they still count in the total. Evolved
developer ideas are candidates — deference to hand-coded
design-via-iteration. Dictator-phase spec in Trash is history, not law.
Commits: `automatic checkpoint` ≈ assistant; `WIP` ≈ developer.

## Loop ticks

| Tick | Swath | Caught | % guess (caught / all, incl. unknowns) |
|---|---|---|---|
| 0 (pass 1) | Cadence / color default / VSYNC / depth-closed / dummy-head floors | Table above | ~10% |
| 1 | Dummy-head GPU esc **~60 Hz** as standing fact | `shadergroup-virtues.md`, shade-gpu interview living notes, issue-stack charter: that Hz was a snapshot; pin is dummy-head GPU esc ≥40; unoptimized debug can miss; headed still unchecked | **~12%** |
| 2 | HUD `color:` still OG by default | Window init + `ColorerMode`/`ColorerHud` Default were OG after GPU became product default; collector WorkUpdates wiped shade stamps back to OG. Defaults + preserve stamps. Issue-stack “root cause fixed” retitled so it is not headed blockiness. GPU unit tests now take the same wgpu lock as the IPS probe (parallel GPU tests were starving it). | **~14%** |
| 3 | Two GPU locks conflated as one | Colorer/escaper tests named `_wgpu` but take in-process `lock_gpu_tests()`; IPS probe + cadence take `/tmp/cz_wgpu_test.lockdir`. Cadence header still spoke as if `--all-targets` were the house run. `pipeline-refresh-rates` “OG remains default” read as colorer. Split the names/comments. Tick 2’s “same wgpu lock as IPS” was itself a ghost. | **~16%** |
| 4 | `craftsmanship_tests.rs` after the split | Tracey pins, `testing.md`, and AGENTS still named a single file. Tests live in `craftsmanship_tests/` (`mod.rs` + tiers). Paths updated. | **~18%** |
| 5 | bacon `test-lib` skip list | Skipped dead names (`home_800x480`, `standards_perf::`) and **`gpu_ips`**, which still matches `naive_gpu_ips_ratio_probe` — lightning was silently omitting the IPS probe. Now skips `integration_tier` / `e2e_tier` only. | **~20%** |
| 6 | coverage/mutants skip lists | Same dead filters as bacon, including `gpu_ips` matching `naive_gpu_ips_ratio_probe`. Both scripts now skip `integration_tier` / `e2e_tier` only. | **~22%** |
| 8 | mutants default `--file` list | Defaulted to missing `intexp.rs` + tile-era `tile_manager`/`tile_publisher`. Now `utils.rs` / `range.rs` / `floatexp.rs`. Issue-stack charter still said “release pin / debug can miss.” | **~26%** |
| 9 | Admit-margin docs still said “neighbor-only, omits 10 bits” | Live admit (2026-08-13) converts IntExp probe points through `T`, f64 `From<IntExp>` rounds as one value, slider fail-closes. Issue-stack + depth-design caught up. **Headed blockiness still not product-fixed.** | **~28%** |
| 10 | “Tests pin WorkUpdate f64 host” as if no f32 iterate | Channel is still f64 for the collector; OG naive now iterates `Mandelbrotable` f32 when the bit-count gate admits, then converts. Margin default 1. | **~30%** |
| 11 | HUD S-F64 past mag 14 on naive | Pitch `< 1e-7` stamped ScaledF64 on **absolute** shells. Iterate was still f64/f32. Floor is relative/pert only. | **~32%** |
| 12 | Number-stack “~10 bits headroom” + CopyIntExp mul “abs then sign” | Live margin default is **1**. Mul is unsigned `u128` schoolbook. OG naive now uses `CopyIntExp<1>` after absolute f64 (home zoom 42, still on at 49). | **~34%** |
| 13 | Mag-38 black ⇒ bump CopyIntExp at `1e-14` | Assistant hid a location/period question behind a type switch. Admit is bit count; mag 38 f64 still admits and far-exterior escapes. Pitch prefer-relative is pert/GPU. | **~36%** |
| 14 | `critical-zoomer-invariants.mdc` | AGENTS and craftsmanship rules name it as the always-on summary. File was missing from `.cursor/rules/`. Restored from the six typed invariants. | **~38%** |
| 15 | Mag-38 black as i64 / pitch | HUD `gear:F64` `mode:naive` is OG DirectKernel, not the i64 tape. `stack:` is the host. Assistant CopyIntExp bump was the OG regression. Mag-43 grey is i64→f64 collector narrow. | **~40%** |
| 16 | `full_check` still “on repo `target/`” in the isolation rule; AGENTS checkpoints “non-main only” | Hook builds `/tmp/cz_full_check_cargo_target`; agents `/tmp/cz_cursor_cargo_target`. Checkpoints go on the **checked-out** branch (often main). Hook green ≠ hunt done; redundant always-on rules for don’t-stagnate / follow-through / checkpoints. | **~42%** |
| 17 | Mag-43 “flat grey” + collector `to_f64(c)` | **Four quadrants glued to the window while dragging.** Screen-space sign, not objective `c`. Admit fine. Guess: CopyIntExp negative δ / f64 of it. | **~46%** |

Not 100%. Implementation ghosts still mostly unknown. Headed 2×2 grey / head CPU still open.
