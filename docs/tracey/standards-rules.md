# Standards Tracey rules (assistant-owned mapping of docs/standards.md hard bars)

Not authoritative text. Hard asserts only — no ignore waives.

r[cz.perf.foveation-half-time+1]

**Normative summary.** Half of available working time fills the current stencil; half goes to lookahead.

**Acceptance criteria.**
- [x] Scheduler/session accounts work time 50/50 current vs lookahead (±tolerance in verifies)
- [x] Mag-velocity order policy still applies inside each half (`cz.seamless.foveated-mag-velocity+1`)
- [x] ≥3 unit verifies

r[cz.perf.home-100tps+1]

**Normative summary.** Home view averages ~100 TPS (~5s to complete at default 800×480).

**Acceptance criteria.**
- [x] Release hard-assert: ≥95% home fill within 5s
- [x] Reported tile completion rate supports ~100 TPS class
- [x] Cross-linked with `cz.e2e.perf-home-fill+1`

r[cz.perf.min-300m-ips-cpu+2]

**Normative summary.** ≥300M iterations/s on single-core CPU perturbation path in
real plausible workgroup situations (TileSession workshifts), including all
scheduling overhead. Also keep bout microbenches as a diagnostic suite (math vs
scheduling).

**Acceptance criteria.**
- [x] Release hard-assert ≥300e6 IPS on ≥3 full-stack fixtures spanning both classes:
  easy tiles outside r=2 (scheduling-worst) and longer work inside the set (iteration-best)
- [x] Bout/microbench suite present and passing (diagnostic only; not a substitute for full-stack)

r[cz.perf.min-30b-ips-gpu+1]

**Normative summary.** ≥30B iterations/s on GPU perturbation path in real plausible
workgroup situations (TileSession workshifts), including all scheduling overhead.
Also keep bout microbenches as a diagnostic suite (math vs scheduling).
No adapter ⇒ fail.

**Acceptance criteria.**
- [x] Release hard-assert ≥30e9 IPS on ≥3 full-stack fixtures spanning both classes:
  easy tiles outside r=2 (scheduling-worst) and longer work inside the set (iteration-best)
- [x] Missing GPU adapter fails the verify
- [x] Bout/microbench suite present and passing (diagnostic only; not a substitute for full-stack)

r[cz.perf.optimal-ipp+1]

**Normative summary.** Iterations per point equal optimal (out → escape time; in → preperiod + period).

**Acceptance criteria.**
- [x] ≥3 loci: exterior escape IPP matches escape time
- [x] Interior period-known IPP matches preperiod+period when available

r[cz.perf.headgroup-shaders-2ms+1]

**Normative summary.** Headgroup shaders together ≤2ms frametime at 1080p. No adapter ⇒ fail.

**Acceptance criteria.**
- [x] Release hard-assert sample+shade path ≤2ms @1080p (≥3 scripts/resolutions)
- [x] Missing GPU adapter fails the verify

r[cz.perf.headgroup-vsync+1]

**Normative summary.** Vsync / PresentMode::Fifo enabled; no janky forced FPS cap.

**Acceptance criteria.**
- [x] Native/wgpu present path uses Fifo
- [x] VSYNC const true; ≥3 verifies

r[cz.ctrl.zoom-in-homothety+1]

**Normative summary.** Zoom-in: magnification pot += 1; location ← (L − P)/2 + P (pointer-fixed).

**Acceptance criteria.**
- [x] Complex under pointer invariant across one zoom-in bump
- [x] pot increases by 1; zoom-out inverse (≥3)

r[cz.ctrl.scroll-up-zooms-in+1]

**Normative summary.** Scroll up ⇒ zoom in ⇒ magnification pot +1.

**Acceptance criteria.**
- [x] Scroll step polarity maps to pot +1 for zoom-in direction (≥3)

r[cz.perf.play-minimize+1]

**Normative summary.** Aggressively minimize play: no or very small initialization
phases; continuous delivery of work so far.

**Acceptance criteria.**
- [x] After retarget/gesture, publishable work appears without a long idle init
  (`standards_perf` play_* verifies)
- [x] Noop retarget does not invent a deferred play stall (≥3)
- [x] Cross-linked with `cz.perf.play-8bump-100ms+1`

r[cz.perf.play-8bump-100ms+1]

**Normative summary.** After the user zooms in 8 bumps at a time, some new work
must be visible within 100ms of the last bump of the gesture.

**Acceptance criteria.**
- [x] Release hard-assert: visible publish within 100ms after 8 zoom-in bumps
  (`standards_perf` play_eight_* verifies; home + exterior + per-bump)
- [x] ≥3 unit verifies

r[cz.play.actor-poll+1]

**Normative summary.** Each actor checks its input channel at a quick pace at the
start of its loop.

**Acceptance criteria.**
- [ ] Actor loops re-check inputs before sleep/idle (≥3 actors / paths)
- [ ] No long blocked wait that skips channel poll

r[cz.play.actor-drain+1]

**Normative summary.** Each actor fully drains its channel when anything is there.

**Acceptance criteria.**
- [ ] Nonempty channel ⇒ drain to empty before returning to idle (≥3)
- [ ] Partial drain under load is a fail

r[cz.play.latest-wins+1]

**Normative summary.** Actors immediately prioritize the most recent work over
previous work. Exception: the headgroup must ingest all unique new tiles —
neither dropping tiles nor getting behind are acceptable.

**Acceptance criteria.**
- [ ] Non-headgroup actors apply latest-wins / preempt (≥3)
- [ ] Headgroup ingest keeps all unique tiles (no drop / no behind); distinct from
  `cz.int.hoard-ingest-sample+1` key/NORES rules

r[cz.perf.home-10000tps-gpu+1]

**Normative summary.** Home view at default resolution on GPU must average
TPS ≥ 10000.

**Acceptance criteria.**
- [ ] Release hard-assert GPU home TPS ≥10000 (≥3)
- [ ] No adapter ⇒ fail (same class as other GPU hard bars)
- [ ] Cross-linked with `cz.perf.home-100tps+1` (CPU class)

r[cz.math.perturbation-naive-oracle+1]

**Normative summary.** Perturbation answers must exactly match a trusted naive
oracle (doubling precision until stable) at home view and several well-known
sites. Compare the entire Answer, not merely the result class.

**Acceptance criteria.**
- [ ] Naive doubling-precision oracle procedure implemented and proven
- [ ] Exact Answer parity at home + ≥2 known sites (neck locus −0.75+0i useful
  for period/infill stress — fixture, not a separate bar)
- [ ] Distinct from headed visual `cz.e2e.visual-oracle+1`

r[cz.ref.zero-orbit-same-path+1]

**Normative summary.** Reference data handles looping points; a const zero big-Z
orbit exists; when no better reference is available that const orbit is used on
the same code path (no alternate algorithm branch).

**Acceptance criteria.**
- [ ] Const zero orbit present and used on fallback
- [ ] Looping points handled correctly
- [ ] Same per-point path with/without better reference (≥3)

r[cz.pub.gpu-native-work+1]

**Normative summary.** Completed work remains GPU-native through the publisher
path so easy full-screen cases keep throughput.

**Acceptance criteria.**
- [ ] GPU-native handoff / no forced CPU round-trip on publish path (≥3;
  aligns with D-PUB-1 / `gpu_tile` handoff)

r[cz.perf.headgroup-stable-path+1]

**Normative summary.** Sample+shade take one path every frame; no frametime
change when panning vs stationary; sampling shader always the same path.

**Acceptance criteria.**
- [ ] No branch that changes sample/shade work for pan vs idle (≥3)
- [ ] Frametime class stable across pan vs stationary samples when measurable
