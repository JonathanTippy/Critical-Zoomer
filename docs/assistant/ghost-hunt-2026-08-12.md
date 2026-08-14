# Ghost hunt (2026-08-12)

**PAUSED (developer 2026-08-14):** mutant grind is active —
`docs/assistant/mutant-hunt-2026-08-14.md`, `docs/assistant/work-stack.md`,
loop body `mutant-grind-loop-prompt.md`. Do not resume this hunt until mutant
hunt ends or developer says otherwise.

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
| Head window ~100% CPU / bare `request_repaint` | `window/mod.rs` since `351afdf`; worker park is wrong actor | Open product issue; docs aligned (ticks 25–27). |
| Depth product-trust / headed blockiness | Finish-line **unit** gates landed; rectangular blockiness + post-admit drop open. Collector still `WorkUpdate<f64>`; iterate may be f32 then convert (`work-update.md`). | Do not call depth-trust done. |
| Implementation ghosts in git history | Unknown count | Hunt in history; not 100%. |
| Mag 44 i64 `\|c\|<1` full black | Pinned; CopyIntExp latch candidate | Headed not developer-confirmed (tick 23). |

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
| 18 | Signed/center screen-δ as the grey RCA | Developer: screen is UL, +right/+down, seats always ≥ 0. Drag still means UL seat/row, not objective `c`. Retract negative offsets. Handoff `recontinuation-i64-grey.md`. | **~48%** |
| 19 | Mag-43 grey = 2×2 / collector / signed δ | **Measured then fixed:** `From` sign steal, unsigned add, f64 relative `c`, `Words=1` mul `>>64`. Developer confirmed headed 2026-08-13. | **~58%** |

| 20 | HUD `stack:` + taskset `0-7`/`4-11` | Overlay is `type:`. Pin is middle-half cores (`3-8` on 12). Mag-44 grey closed. | **~60%** |
| 21 | Idle HUD `ctrl:` 0 | Controller woke ~50 ms and only `Replace`d on stencil change; stamps aged out of the 1 s counter. Now content-period + `Pace` (no remap). Drain merge keeps Replace over Pace. Headed `ctrl:` vs `pub:` still for the developer. | **~62%** |
| 22 | Pace as `frame_info: Some` | Collector remap path. Headed black, `pub:`/`ctrl:` 0, shade still emitting Dummy. Pace is `None` + empty seats. | **~64%** |
| 23 | Mag 44 i64 full black | DirectKernel checkpoint stayed origin; pert already latches `(z,0)`. CopyIntExp `0-ε` → -1, ipp:1 all repeats. Assistant Ord/shr rewrite reverted. | **~66%** |
| 24 | Work pacing / shallow-mag lag | Collector beat-only publish + stall→1 Hz + controller stencil-before-send. `content-beat-publish+3`, Hz floor, drain-to-newest+2. Developer headed 2026-08-13. | **~68%** |
| 25 | Cadence docs vs live timers | `shadergroup-virtues` still said 8 ms shade + ~20 Hz publish; actor-layout parking lot still “not implemented”. Updated to `resolved_content_period` / content landed / head open. | **~70%** |
| 26 | `collected-wisdom` escaper 60 Hz | Line read as current perf; live pin is dummy-head ≥15/≥40 on debug+opt-3. Aspiration vs pin separated. Loop restarted (old sleeper not wired to this chat). | **~71%** |
| 27 | Depth “finish-line green” + shelved head CPU | `depth-design` SA acceptance read as product done; worker park still confused with window idle. Status + acceptance bullets + `headgroup-charter` open pacing section. Top table trimmed to open ghosts only. | **~72%** |
| 28 | `HEADED_I64_BLACK` comment | Line 18 bundled mag 44 black under “Headed 2026-08-13”. Split: grey headed-closed; black pinned not headed-confirmed (`constants.rs`). | **~73%** |
| 29 | `testing.md` mag 44 wording | Read mag 44 as grey headed-closed. Grey vs black split; black pinned only. | **~74%** |
| 30 | `copy-intexp.md` mag 44 mul note | “headed mag 44 escape-at-7” read as closed. Pinned `HEADED_I64_BLACK_*`; headed not confirmed. | **~75%** |
| 31 | Issue-stack mag 44 black | “Headed not re-checked” on pinned black locus. Pinned, not headed-confirmed. | **~76%** |

Not 100%. Implementation ghosts still mostly unknown. Headed i64 grey is closed.
Mag 44 `|c|<1` black is pinned, not headed-confirmed. Work pacing closed headed.
