//! Tracey anchors for headed e2e contracts (scripts own the live verifies).
//! Product behavior under test lives in headgroup/workgroup; these ids are the
//! headed interaction layer itself.

// r[impl cz.e2e.harness-stack+1]
// r[impl cz.e2e.controls-bindings+1]
// r[impl cz.e2e.controls-no-jump+1]
// r[impl cz.e2e.perf-home-fill+1]
// r[impl cz.e2e.perf-zoom-simple+1]
// r[impl cz.e2e.perf-zoom-hard+1]
// r[impl cz.e2e.visual-assistant-review+1]
//
// Verifies: scripts/harness_selftest.sh, e2e_controls.sh, e2e_performance.sh,
// e2e_visual.sh, e2e_suite.sh (r[verify …] on those scripts). Oracle code:
// e2e_oracle.rs.
