# Ghost hunt (2026-08-12)

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
| Depth “finish-line / precision wall green” | Blockiness + post-admit f64 interlayer still open. Tests **pin** `WorkUpdate<f64>` host. | Do not call depth-trust done. |
| Dummy-head GPU esc ~60 Hz / “test currently passes” | **Tests fail (2026-08-12 debug):** OG mean esc ~10 (floor 15), GPU ~23 (floor 40). Headed still unchecked. | Honest `testing.md`. Do not ignore or lower floors. GPU grind paused. |
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
| 1 | Dummy-head GPU esc **~60 Hz** as standing fact | `shadergroup-virtues.md`, shade-gpu interview living notes, issue-stack charter: that Hz was a snapshot; pin is release ≥40; debug can miss; headed still unchecked | **~12%** |
| 2 | HUD `color:` still OG by default | Window init + `ColorerMode`/`ColorerHud` Default were OG after GPU became product default; collector WorkUpdates wiped shade stamps back to OG. Defaults + preserve stamps. Issue-stack “root cause fixed” retitled so it is not headed blockiness. GPU unit tests now take the same wgpu lock as the IPS probe (parallel GPU tests were starving it). | **~14%** |
| 4 | `craftsmanship_tests.rs` after the split | Tracey pins, `testing.md`, and AGENTS still named a single file. Tests live in `craftsmanship_tests/` (`mod.rs` + tiers). Paths updated. | **~18%** |

Not 100%. Implementation ghosts still mostly unknown.
