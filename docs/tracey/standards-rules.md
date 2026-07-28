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

r[cz.perf.min-300m-ips-cpu+1]

**Normative summary.** ≥300M iterations/s on CPU perturbation path (honest IPS fixtures).

**Acceptance criteria.**
- [x] Release hard-assert ≥300e6 IPS on ≥3 exterior fixtures/gears
- [x] Counts bout iterations only (same definition as standards IPS)

r[cz.perf.min-30b-ips-gpu+1]

**Normative summary.** ≥30B iterations/s on GPU perturbation path. No adapter ⇒ fail.

**Acceptance criteria.**
- [x] Release hard-assert ≥30e9 IPS on ≥3 fixtures
- [x] Missing GPU adapter fails the verify

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
