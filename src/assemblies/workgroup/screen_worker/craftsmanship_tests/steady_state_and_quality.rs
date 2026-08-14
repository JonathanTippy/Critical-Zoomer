// ---------------------------------------------------------------------------
// Steady-state speed path (screen worker alone + workgroup telemetry chain)
// See docs/assistant/testing.md — these are the lifeblood integration tests.
// ---------------------------------------------------------------------------

/// Actor formula: after a workshift, `total_iterations_today` *is* the shift delta
/// (workshift zeros the counter at start). Must stay non-zero across many shifts.
fn shift_iterations_delta(ctx: &WorkContext<f64>) -> u64 {
    ctx.total_iterations_today as u64
}

/// Iterate-only telemetry must still reach the HUD: a shift can burn iterations
/// with zero completions (deep interior) and must not drop `iterations_delta`.
// r[verify cz.depth.gear-hud+2]
#[test]
fn steady_state_ips_delta_sent_without_completions() {
    use crate::assemblies::structs::ViewHud;
    let update = telemetry_update::<f64>(None, vec![], None, 12_345, None);
    assert_eq!(update.iterations_delta, 12_345);
    assert!(update.completed_points.is_empty());
    let hud = ViewHud {
        stack: update.host_stack,
        mode: update.kernel_mode,
        reference: update.reference_status,
        gear: update.active_gear,
        points_delta: update.completed_points.len() as u64,
        iterations_delta: update.iterations_delta,
        packages_dropped: 0,
        ..Default::default()
    };
    let mut ips = RateCounter::default();
    let now = Instant::now();
    ips.record(hud.iterations_delta, now);
    assert!((ips.rate(now) - 12_345.0).abs() < 1e-9);
}

/// Shallow home must stay on DirectKernel (no soft-trial without a usable ref).
/// Guards the post-v0.0.9 workshift policy from silently running pert at home.
// r[verify cz.perf.pps-selected-kernel+1]
#[test]
fn home_workshift_stays_on_direct_kernel_without_ref() {
    run_big_stack_size(|| {
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        assert!(!ctx.coords_are_relative);
        assert!(ctx.latest_reference.is_none());
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, None);
            assert!(
                !ctx.perturbation_kernel_required(),
                "home must not require pert without relative coords or trial floor"
            );
            assert!(
                !ctx.reference_floor_active,
                "home must not soft-trial pert without a published reference"
            );
            assert!(!ctx.last_used_naive_gpu);
            let _ = work_update(&mut ctx);
        }
    });
}

/// First non-empty publish on home must stay within 20% of DirectKernel (guards
/// policy tax on the play-minimize path). If *both* paths are slow vs historical
/// ~39–52 ms Criterion, that is a DirectKernel-path FIX NOW — not a soft floor.
// r[verify cz.perf.play-minimize+1]
#[test]
fn home_workshift_first_publish_within_20pct_of_direct_kernel() {
    run_big_stack_size(|| {
        let fill_first = |use_workshift: bool| {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut shifts = 0u32;
            let mut got = 0usize;
            while got == 0 {
                check_test_budget();
                if use_workshift {
                    workshift(0, 0, 0, 0, &mut ctx, None);
                } else {
                    workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
                }
                got = work_update(&mut ctx).len();
                shifts += 1;
            }
            assert!(
                got > 0,
                "no first publish shifts={shifts} workshift={use_workshift}"
            );
            (t0.elapsed().as_secs_f64(), shifts, got)
        };
        // Sub-30ms samples are scheduler-noisy; take median of 5 so the ≤1.20×
        // bar stays hard without soft-flooring a single cold sample.
        let mut direct_samples = Vec::new();
        let mut via_samples = Vec::new();
        for _ in 0..5 {
            refresh_test_budget();
            direct_samples.push(fill_first(false).0);
            via_samples.push(fill_first(true).0);
        }
        direct_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        via_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let direct_t = direct_samples[2];
        let via_t = via_samples[2];
        let ratio = via_t / direct_t.max(1e-9);
        eprintln!(
            "home first publish median-of-5: direct={direct_t:.4}s workshift={via_t:.4}s ratio={ratio:.2}×"
        );
        assert!(
            ratio <= 1.20,
            "workshift first publish {via_t:.4}s is >20% slower than DirectKernel {direct_t:.4}s (ratio={ratio:.2}×); FIX NOW — do not soften"
        );
    });
}

/// Production `workshift` home fill must stay within 20% of DirectKernel wall time
/// (guards policy/scan tax regressions that made home feel slow post-v0.0.9).
// r[verify cz.perf.min-300m-ips-cpu+2]
#[test]
fn home_workshift_full_frame_within_20pct_of_direct_kernel() {
    let _gpu_guard = super::naive_gpu::lock_gpu_tests();
    run_big_stack_size(|| {
        let fill = |use_workshift: bool| {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut shifts = 0u32;
            while !ctx.points.iter().all(|p| p.delivered) {
                check_test_budget();
                if use_workshift {
                    workshift(0, 0, 0, 0, &mut ctx, None);
                } else {
                    workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
                }
                let _ = work_update(&mut ctx);
                shifts += 1;
            }
            assert!(
                ctx.points.iter().all(|p| p.delivered),
                "home incomplete shifts={shifts} workshift={use_workshift}"
            );
            t0.elapsed().as_secs_f64()
        };
        // Sub-10ms home fills are scheduler-noisy under cargo parallel; median of
        // paired ratios (not ratio-of-medians) keeps ≤1.20× hard.
        let mut ratios = Vec::new();
        let mut last_pair = (0.0f64, 0.0f64);
        for _ in 0..5 {
            refresh_test_budget();
            let direct = fill(false);
            let via_workshift = fill(true);
            last_pair = (direct, via_workshift);
            ratios.push(via_workshift / direct.max(1e-9));
        }
        ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let ratio = ratios[2];
        eprintln!(
            "home wall median-of-5 paired ratios: last direct={:.3}s workshift={:.3}s median_ratio={ratio:.2}×",
            last_pair.0, last_pair.1
        );
        assert!(
            ratio <= 1.20,
            "workshift home median paired ratio {ratio:.2}× exceeds 20% over DirectKernel (last direct={:.3}s workshift={:.3}s); FIX NOW — do not soften",
            last_pair.0,
            last_pair.1
        );
    });
}

/// Series is always-on seat init when a published reference carries coeffs.
/// Hard pin — never `#[ignore]` (quality-doctrine).
// r[verify cz.depth.series-approximation+1]
#[test]
fn series_approximation_wired_into_production_kernels() {
    let pert = include_str!("../perturb_kernel.rs");
    let floatexp = include_str!("../perturb_floatexp.rs");
    assert!(
        pert.contains("apply_series_skip("),
        "perturb_kernel must invoke apply_series_skip on seat init"
    );
    assert!(
        floatexp.contains("apply_series_skip("),
        "perturb_floatexp must invoke apply_series_skip on seat init"
    );
}

/// Production dispatch must never select the test-only Oracle gear.
// r[verify cz.depth.oracle-gear+1]
#[test]
fn production_workshift_never_dispatches_oracle_gear() {
    let workshift = include_str!("../workshift.rs");
    let screen_mod = include_str!("../mod.rs");
    assert!(
        !workshift.contains("OracleKernel") && !screen_mod.contains("OracleKernel"),
        "OracleKernel is test-only; production screen_worker must not reference it"
    );
    assert!(
        workshift.contains("DirectKernel") && workshift.contains("PerturbationKernel"),
        "production must keep DirectKernel + PerturbationKernel dispatch"
    );
}

/// RCA probe for GH #34: GPU vs CPU class mismatches (false interior → black).
/// Prints counts; fails if GPU marks Escapes-as-Repeats or leaves undelivered.
#[test]
fn rca_naive_gpu_vs_cpu_class_parity_home_and_boundary() {
    run_big_stack_size(|| {
        let Some(mut shared) = super::naive_gpu::SharedGpu::acquire() else {
            eprintln!("rca_naive_gpu_vs_cpu_class_parity: no GPU — skipped");
            return;
        };
        refresh_test_budget();

        let frames = [
            ("home", home_frame()),
            ("antenna", {
                use crate::assemblies::headgroup::window::coords::{f64_to_intexp, ul_for_center};
                (
                    ul_for_center(f64_to_intexp(-1.0), f64_to_intexp(0.0), 2, TEST_SCREEN_RES),
                    TEST_SCREEN_RES,
                )
            }),
        ];

        for (label, frame) in frames {
            let mut cpu = from_stencil::<f64>(frame.clone(), None).expect(label);
            while !cpu.points.iter().all(|p| p.delivered) {
                check_test_budget();
                workshift_with_kernel(0, 0, 0, 0, &mut cpu, &DirectKernel);
                let _ = work_update(&mut cpu);
            }

            let mut gpu = from_stencil::<f64>(frame, None).expect(label);
            gpu.manual_gear = Some(crate::assemblies::structs::KernelMode::NaiveGpu);
            while !gpu.points.iter().all(|p| p.delivered) {
                check_test_budget();
                workshift(0, 0, 0, 0, &mut gpu, Some(shared.ctx()));
                let _ = work_update(&mut gpu);
            }

            let n = cpu.points.len();
            assert_eq!(n, gpu.points.len());
            let mut false_interior = 0usize;
            let mut false_exterior = 0usize;
            let mut both_interior = 0usize;
            let mut both_exterior = 0usize;
            for i in 0..n {
                let c_in = cpu.points[i].repeats;
                let c_out = cpu.points[i].escapes;
                let g_in = gpu.points[i].repeats;
                let g_out = gpu.points[i].escapes;
                assert!(
                    cpu.points[i].delivered && gpu.points[i].delivered,
                    "{label} seat {i} undelivered cpu={} gpu={}",
                    cpu.points[i].delivered,
                    gpu.points[i].delivered
                );
                match (c_in, g_in, c_out, g_out) {
                    (true, true, _, _) => both_interior += 1,
                    (false, false, true, true) => both_exterior += 1,
                    (false, true, true, _) => false_interior += 1,
                    (true, false, _, true) => false_exterior += 1,
                    _ => {}
                }
            }
            eprintln!(
                "rca {label}: both_in={both_interior} both_out={both_exterior} false_in={false_interior} false_out={false_exterior} gear={:?}",
                shared.ctx().precision
            );
            assert_eq!(
                false_interior, 0,
                "{label}: GPU false interiors (CPU escape → GPU repeat) = {false_interior} — black splotch class"
            );
        }
    });
}

/// v0.0.9-era naive f64 home fill: counted iteration budget scales with seat
/// count at fixed pitch. Product identity was 10_302_563 @ 854×480; at
/// TEST_SCREEN_RES the accepted identity is the measured DirectKernel total
/// for the centered test home (re-pin if home_frame pitch/center changes).
// r[verify cz.perf.min-300m-ips-cpu+2]
#[test]
fn naive_f64_direct_kernel_home_preserves_v009_iteration_budget() {
    let _gpu_guard = super::naive_gpu::lock_gpu_tests();
    run_big_stack_size(|| {
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut shifts = 0u32;
        let mut iters = 0u64;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            iters += shift_iterations_delta(&ctx);
            let _ = work_update(&mut ctx);
            shifts += 1;
        }
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "DirectKernel home must complete (v0.0.9 baseline); shifts={shifts}"
        );
        // Centered TEST_SCREEN_RES home-class view @ pot -6 (cardioid).
        const TEST_HOME_DIRECT_ITERS: u64 = 10_362;
        assert_eq!(
            iters, TEST_HOME_DIRECT_ITERS,
            "DirectKernel home iteration budget drifted from TEST_SCREEN_RES accepted identity; iters={iters} shifts={shifts}"
        );
        assert!(
            !ctx.perturbation_kernel_required(),
            "shallow home must remain legal for naive DirectKernel (not forced pert)"
        );
    });
}
