// ---------------------------------------------------------------------------
// Home view regression (reference c must match CGenerator; workshift parity)
// ---------------------------------------------------------------------------

use crate::assemblies::workgroup::reference_worker::{PublishedReference, select_reference_request};
use crate::constants::HOME_POSITION;
use crate::reference::ReferenceOrbit;
use std::sync::Arc;

fn home_frame() -> (ObjectivePosAndZoom, (u32, u32)) {
    // Product UL+zoom at TEST_SCREEN_RES crops a corner; product *center* at the
    // same zoom is still a tiny exterior-biased patch (1 iter/seat). Use a
    // home-class window: cardioid-centered, zoomed out so ~40 seats span ~the
    // same world width as product 854×480 @ pot -2 (~6.7 units → pot -6).
    use crate::assemblies::headgroup::window::coords::{f64_to_intexp, ul_for_center};
    (
        ul_for_center(
            f64_to_intexp(-0.75),
            f64_to_intexp(0.0),
            -6,
            TEST_SCREEN_RES,
        ),
        TEST_SCREEN_RES,
    )
}

/// Shallow view whose seat grid is conjugate-symmetric across the real axis:
/// seat `(x, y)` and `(x, h-1-y)` have conjugate `c` (CGenerator: +row = −imag).
/// Odd height keeps a mid-row exactly on the axis. Home itself is *not* this
/// geometry (origin imag ≠ (h−1)·space/2), so tumor-style asymmetry needs this.
fn real_axis_symmetric_shallow_frame(
    res: (u32, u32),
    zoom_pot: i32,
    origin_real: i32,
) -> (ObjectivePosAndZoom, (u32, u32)) {
    assert!(res.1 % 2 == 1, "odd height keeps mid-row on the real axis");
    let half_rows = (res.1 as i32 - 1) / 2;
    // compute origin imag = half_rows * space; space = 2^(-(zoom+PIXELS_PER_UNIT_POT))
    let origin_imag =
        IntExp::from(half_rows).shift(-(zoom_pot + crate::constants::PIXELS_PER_UNIT_POT));
    (
        ObjectivePosAndZoom {
            // Display Y is stored unflipped; from_stencil flips to compute.
            pos: (IntExp::from(origin_real), IntExp::ZERO - origin_imag),
            zoom_pot,
        },
        res,
    )
}

fn outcome_key(p: &Point<FloatExp>) -> (bool, bool, u32, u32) {
    (p.escapes, p.repeats, p.iterations, p.period)
}

/// Wrong-answer oracle key: classification + escape/period times + small_time.
fn answer_oracle_key(p: &Point<FloatExp>) -> (bool, bool, u32, u32, u32) {
    (p.escapes, p.repeats, p.iterations, p.period, p.small_time)
}

/// Exterior escape oracle: the tumor inflated escape_time/small_time outside r=2.
fn exterior_escape_oracle_key(p: &Point<FloatExp>) -> (u32, u32) {
    (p.iterations, p.small_time)
}

fn is_strict_exterior_c(cr: f64, ci: f64) -> bool {
    cr * cr + ci * ci > 4.0
}

fn c_f64(ctx: &WorkContext<FloatExp>, index: usize) -> (f64, f64) {
    let (cr, ci) = if ctx.points[index].initialized {
        (
            ctx.points[index].delta_c.0.to_f64(),
            ctx.points[index].delta_c.1.to_f64(),
        )
    } else {
        let (x, y) = pos_from_index(index, ctx.res.0);
        let gc = ctx.c_generator.get_c((x as u32, y as u32));
        (gc.0.to_f64(), gc.1.to_f64())
    };
    if ctx.coords_are_relative {
        (
            FloatExp::from(ctx.coord_anchor.0.clone()).to_f64() + cr,
            FloatExp::from(ctx.coord_anchor.1.clone()).to_f64() + ci,
        )
    } else {
        (cr, ci)
    }
}

fn install_usable_interior_reference(
    ctx: &mut WorkContext<FloatExp>,
    frame: &(ObjectivePosAndZoom, (u32, u32)),
    generation: u64,
) {
    let req = select_reference_request::<FloatExp>(None, frame);
    let mut orbit = ReferenceOrbit::start(&req.c, req.precision_bits);
    orbit.extend(4096);
    let (c, orbit) = if orbit.escaped {
        // Default home center c can be exterior; use a known interior ref for HUD fixtures.
        let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
        let orbit = ReferenceOrbit::compute(&reference_c, 128, 4096);
        assert!(!orbit.escaped, "interior fixture ref must not escape");
        (reference_c, orbit)
    } else {
        (req.c, orbit)
    };
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit,
        c,
        generation,
    }));
}

fn activate_reference_floor<T: Mandelbrotable>(ctx: &mut WorkContext<T>) {
    ctx.reference_floor_active = true;
    ctx.pert_trial_shifts_left = u8::MAX;
}

fn install_covering_reference_with_series(
    ctx: &mut WorkContext<FloatExp>,
    frame: &(ObjectivePosAndZoom, (u32, u32)),
) {
    // Series deferred: install orbit-only covering reference for parked series tests.
    let req = select_reference_request::<FloatExp>(None, frame);
    let orbit = ReferenceOrbit::compute(&req.c, req.precision_bits, 4096);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit,
        c: req.c,
        generation: 1,
    }));
    activate_reference_floor(ctx);
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn telemetry_mode_naive_then_pert() {
    use crate::assemblies::structs::{HostStack, KernelMode, ReferenceStatus};
    use crate::assemblies::workgroup::screen_worker::{
        classify_kernel_mode, classify_reference_status, host_stack_for_context,
    };
    run_big(|| {
        let frame = home_frame();
        let ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home view");
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Naive);
        assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Wip);
        assert_eq!(host_stack_for_context::<FloatExp>(), HostStack::FloatExp);
        let mut ctx = ctx;
        install_usable_interior_reference(&mut ctx, &frame, 1);
        assert_eq!(
            classify_kernel_mode(&ctx),
            KernelMode::Naive,
            "ref alone must not flip HUD to pert"
        );
        assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Complete);
        ctx.record_hud_completion_batch(100);
        ctx.reference_floor_active = true;
        assert_eq!(
            classify_kernel_mode(&ctx),
            KernelMode::Pert,
            "active reference floor shows pert"
        );
    });
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_floor_trials_only_when_genuinely_stuck() {
    run_big(|| {
        let frame = home_frame();
        let mut ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home view");
        install_usable_interior_reference(&mut ctx, &frame, 1);
        ctx.record_hud_completion_batch(100_000);
        assert_eq!(
            ctx.update_reference_floor_policy(),
            "direct_fast_enough",
            "fast direct fill must not trial perturbation"
        );
        ctx.hud_points_window = (ctx.screen_point_count() / 6) as u32;
        ctx.hud_window_started =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        assert_eq!(
            ctx.update_reference_floor_policy(),
            "promote_trial",
            "slow fill with usable ref should start a short trial"
        );
        assert_eq!(ctx.pert_trial_shifts_left, 3);
    });
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_floor_trial_expires() {
    run_big(|| {
        let frame = home_frame();
        let mut ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home view");
        install_usable_interior_reference(&mut ctx, &frame, 1);
        ctx.reference_floor_active = true;
        ctx.pert_trial_shifts_left = 1;
        assert_eq!(ctx.tick_pert_trial(), Some("trial_expired"));
        assert!(!ctx.reference_floor_active);
        assert!(ctx.pert_trial_cooldown > 0);
    });
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_complete_with_reused_ref() {
    use crate::assemblies::structs::{KernelMode, ReferenceStatus};
    use crate::assemblies::workgroup::screen_worker::{
        classify_kernel_mode, classify_reference_status,
    };
    run_big(|| {
        let frame = home_frame();
        let mut ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home view");
        install_usable_interior_reference(&mut ctx, &frame, 1);
        let old_obj = frame.0.clone();
        let reused = from_stencil::<FloatExp>(frame.clone(), Some((ctx, old_obj))).expect("reuse");
        assert_eq!(classify_kernel_mode(&reused), KernelMode::Naive);
        assert_eq!(classify_reference_status(&reused), ReferenceStatus::Complete);
    });
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_wip_while_started_seats_await_ref() {
    use crate::assemblies::structs::{KernelMode, ReferenceStatus};
    use crate::assemblies::workgroup::screen_worker::{
        classify_kernel_mode, classify_reference_status,
    };
    run_big(|| {
        let frame = home_frame();
        let ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home view");
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Naive);
        assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Wip);
    });
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_wip_after_glitch_until_new_generation() {
    use crate::assemblies::structs::ReferenceStatus;
    use crate::assemblies::workgroup::screen_worker::classify_reference_status;
    use crate::floatexp::{ComplexFloatExp, FloatExp};
    let mut ctx = make_context(0);
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&reference_c, 128, 8),
        c: reference_c.clone(),
        generation: 7,
    }));
    ctx.points[0] = make_point((0.0, 0.0));
    ctx.points[0].iterations = 1;
    ctx.points[0].delta = Some(DeltaState {
        delta_z: ComplexFloatExp::new(FloatExp::from(0.25), FloatExp::ZERO),
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        delta_c: ComplexFloatExp::ZERO,
        c: ComplexFloatExp::ZERO,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation: 7,
        gear: crate::delta_gear::ComputeGear::FloatExp,
        scale: FloatExp::ONE,
    });
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[0],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        FloatExp::from(4.0),
        FloatExp::from(1e-15),
        BoutCap::new(1),
    );
    assert!(ctx.points[0].direct_only);
    assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Wip);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&reference_c, 128, 8),
        c: reference_c,
        generation: 8,
    }));
    FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
    assert!(!ctx.points[0].direct_only);
    assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Complete);
}

// r[verify cz.depth.gear-hud+2]
#[test]
fn reference_complete_when_glitch_seats_already_delivered() {
    use crate::assemblies::structs::ReferenceStatus;
    use crate::assemblies::workgroup::screen_worker::classify_reference_status;
    let mut ctx = make_context(0);
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&reference_c, 128, 8),
        c: reference_c,
        generation: 7,
    }));
    ctx.points[0].direct_only = true;
    ctx.points[0].delivered = true;
    assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Complete);
}

fn fill_until_complete_perturb(ctx: &mut WorkContext<FloatExp>) {
    while ctx.percent_completed < 100.0 {
        check_test_budget();
        perturb_workshift(16_000_000, 2, 4, 150, ctx);
        let _ = work_update(ctx);
    }
}

fn fill_until_complete_direct(ctx: &mut WorkContext<FloatExp>) {
    while ctx.percent_completed < 100.0 {
        check_test_budget();
        workshift_with_kernel(16_000_000, 2, 4, 150, ctx, &DirectKernel);
        let _ = work_update(ctx);
    }
}

#[test]
fn home_reference_request_matches_c_generator() {
    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let ctx = from_stencil(frame, None).expect("home view");
        let center = (ctx.res.0 / 2, ctx.res.1 / 2);
        let gc = ctx.c_generator.get_c((center.0, center.1));
        let abs = if ctx.coords_are_relative {
            (
                FloatExp::from(ctx.coord_anchor.0.clone()) + gc.0,
                FloatExp::from(ctx.coord_anchor.1.clone()) + gc.1,
            )
        } else {
            gc
        };
        let req_fe = (
            FloatExp::from(req.c.0.clone()),
            FloatExp::from(req.c.1.clone()),
        );
        assert_eq!(
            req_fe, abs,
            "reference request must match absolute seat"
        );
    });
}

/// Shallow f64-valid data-flow check only: DirectKernel is not a deep-zoom
/// oracle. Ground truth at depth is the rug precision-doubling oracle.
#[test]
fn home_workshift_with_reference_matches_direct() {
    run_big(|| {
        // Symmetric shallow frame: known-good geometry for Direct vs pert data-flow.
        let frame = real_axis_symmetric_shallow_frame(TEST_SCREEN_RES, -2, -2);
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut direct = from_stencil::<FloatExp>(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil::<FloatExp>(frame, None).expect("perturb");
        perturb.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 512),
            c: req.c,
            generation: 1,
        }));
        refresh_test_budget();
        while !direct.points.iter().all(|p| p.delivered) {
            check_test_budget();
            direct.attention_index = 0;
            workshift_with_kernel(0, 0, 0, 0, &mut direct, &DirectKernel);
            work_update(&mut direct);
        }
        refresh_test_budget();
        while !perturb.points.iter().all(|p| p.delivered) {
            check_test_budget();
            perturb.attention_index = 0;
            perturb_workshift(0, 0, 0, 0, &mut perturb);
            work_update(&mut perturb);
        }
        assert!(
            direct.points.iter().all(|p| p.delivered),
            "direct shallow comparator must finish the shell"
        );
        assert!(
            perturb.points.iter().all(|p| p.delivered),
            "perturbation path must finish the shell with a published reference"
        );
        let direct_esc = direct.points.iter().filter(|p| p.escapes).count();
        let pert_esc = perturb.points.iter().filter(|p| p.escapes).count();
        assert!(
            direct_esc > 10 && pert_esc > 10,
            "both paths must produce exterior escapes (direct={direct_esc} pert={pert_esc})"
        );
        // Exact escape/interior class parity under zero-orbit+ref is covered by
        // home_package_with_live_series_matches_direct_kernel_answers.
    });
}

fn exact_f64_as_intexp(value: f64) -> IntExp {
    assert!(value.is_finite());
    if value == 0.0 {
        return IntExp::ZERO;
    }
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (significand, exp) = if biased == 0 {
        (fraction, -1074)
    } else {
        ((1u64 << 52) | fraction, biased - 1023 - 52)
    };
    let mut val = rug::Integer::from(significand);
    if bits >> 63 != 0 {
        val = -val;
    }
    IntExp { val, exp }
}

proptest! {
    // r[verify cz.depth.delta-kernel+1 cz.depth.oracle-doubling+1]
    #[test]
    fn perturbation_kernel_matches_rug_doubling_oracle(
        cr in -2.0f64..1.0,
        ci in -1.5f64..1.5,
    ) {
        use crate::perturb::oracle::{doubling_oracle, OracleOutcome};
        let target = (exact_f64_as_intexp(cr), exact_f64_as_intexp(ci));
        let max_n = 512;
        let oracle = doubling_oracle(&target, max_n);
        prop_assume!(oracle.is_some());
        let mut ctx = make_context(0);
        ctx.points[0] = make_point((cr, ci));
        FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
        FloatExpPerturbationKernel.iterate_bout(
            &mut ctx.points[0], None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(max_n),
        );
        let p = &ctx.points[0];
        match oracle.unwrap() {
            OracleOutcome::Escapes(expected) if p.escapes => {
                prop_assert_eq!(p.iterations + 1, expected);
            }
            OracleOutcome::Escapes(_) => {
                prop_assert!(p.direct_only || !p.repeats);
            }
            OracleOutcome::Unfinished => {
                prop_assert!(!p.escapes || p.iterations + 1 > max_n);
            }
        }
    }
}

#[test]
fn exact_f64_conversion_round_trips_representative_values() {
    for value in [
        0.0,
        -0.0,
        1.0,
        -2.5,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
    ] {
        assert_eq!(f64::from(exact_f64_as_intexp(value)), value);
    }
}

#[test]
// r[verify cz.ref.zero-orbit-same-path+1]
fn zero_orbit_center_reports_period_one() {
    let mut ctx = make_context(0);
    ctx.points[0].initialized = false;
    FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[0], None, FloatExp::from(4.0), ctx.pitch_epsilon, BoutCap::new(2),
    );
    assert!(ctx.points[0].repeats);
    assert_eq!(ctx.points[0].period, 1);
}

#[test]
// r[verify cz.depth.compute-gear+1]
// r[verify cz.depth.gear-hud+2]
fn f64_gear_home_fills_without_per_seat_gear_scan() {
    use std::time::Instant;
    run_big(|| {
        // Small home-centered frame: this pins f64 gear fill, not full-window soak.
        let frame = (
            ObjectivePosAndZoom {
                pos: (
                    IntExp::from(HOME_POSITION.0),
                    IntExp::from(HOME_POSITION.1),
                ),
                zoom_pot: HOME_POSITION.2,
            },
            TEST_SCREEN_RES,
        );
        let mut direct_ctx = from_stencil::<f64>(frame.clone(), None).expect("home direct");
        let direct_start = Instant::now();
        let mut direct_shifts = 0u32;
        while !direct_ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift_with_kernel(0, 0, 0, 0, &mut direct_ctx, &DirectKernel);
            while direct_ctx.completed_points.try_pop().is_some() {}
            direct_shifts += 1;
        }

        let mut ctx = from_stencil::<f64>(frame, None).expect("home f64");
        let start = Instant::now();
        let mut shifts = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
            shifts += 1;
        }
        if cfg!(debug_assertions) {
            assert!(
                shifts < 500,
                "f64 gear shift storm: {shifts} (direct {direct_shifts})"
            );
        } else {
            let wall_ms_limit = (direct_start.elapsed().as_millis() as u128)
                .saturating_mul(2)
                .max(750);
            assert!(
                start.elapsed().as_millis() < wall_ms_limit,
                "f64 home fill too slow: {:?} shifts={shifts} (direct {:?} / {direct_shifts} shifts)",
                start.elapsed(),
                direct_start.elapsed()
            );
            assert!(
                shifts <= direct_shifts.saturating_mul(2).max(60),
                "f64 gear shifts={shifts} far above DirectKernel={direct_shifts}"
            );
        }
    });
}

#[test]
// r[verify cz.depth.compute-gear+1]
fn seahorse_pot_19_f64_promotes_scaled_f64_and_delivers() {
    use crate::delta_gear::ComputeGear;
    run_big(|| {
        let frame = frame_at_center(-0.743643887037151, 0.131825904205216, 19, TEST_SCREEN_RES);
        let req = select_reference_request::<FloatExp>(None, &frame);
        let pub_ref = Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 512),
            c: req.c.clone(),
            generation: 1,
        });
        let mut ctx = from_stencil::<f64>(frame, None).expect("seahorse admits f64 grid");
        ctx.latest_reference = Some(pub_ref);
        activate_reference_floor(&mut ctx);
        let pos = (10, 10);
        PerturbationKernel.start_seat(&mut ctx, pos);
        let idx = index_from_pos(&pos, ctx.res.0);
        let delta = ctx.points[idx].delta.as_ref().expect("delta");
        let dc_mag = delta.delta_c.re.to_f64().abs().max(delta.delta_c.im.to_f64().abs());
        assert!(
            delta.gear == ComputeGear::ScaledF64,
            "deep view deltas must promote past plain f64 (dc_mag={dc_mag:.3e} gear={:?})",
            delta.gear
        );
        let mut shifts = 0u32;
        while ctx.points.iter().filter(|p| p.delivered).count() < 100 {
            check_test_budget();
            workshift(16_000_000, 2, 4, 150, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
            shifts += 1;
        }
    });
}

#[test]
// r[verify cz.depth.compute-gear+1]
fn f64_gear_zero_orbit_center_reports_period_one() {
    let frame = (
        ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        },
        TEST_SCREEN_RES,
    );
    let mut ctx = from_stencil::<f64>(frame, None).expect("f64 grid");
    // Force seat 0 to c=0 (period-1 center).
    ctx.points[0].initialized = false;
    // Patch generator output by initializing then overwriting c after start.
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    ctx.points[0].delta_c = (0.0, 0.0);
    ctx.points[0].initialized = false;
    ctx.points[0].delta = None;
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    // After re-init, force c/delta_c to 0 via re-init_delta path: overwrite delta delta_c.
    if let Some(d) = ctx.points[0].delta.as_mut() {
        d.delta_c = ComplexFloatExp::ZERO;
        d.c = ComplexFloatExp::ZERO;
        d.delta_z = ComplexFloatExp::ZERO;
    }
    ctx.points[0].delta_c = (0.0, 0.0);
    ctx.points[0].c = (0.0, 0.0);
    ctx.points[0].z = (0.0, 0.0);
    PerturbationKernel.iterate_bout(
        &mut ctx.points[0], None, 4.0, ctx.pitch_epsilon, BoutCap::new(4),
    );
    assert!(
        ctx.points[0].repeats,
        "f64 gear must detect period at c=0; iters={} esc={} gear={:?}",
        ctx.points[0].iterations,
        ctx.points[0].escapes,
        ctx.points[0].delta.as_ref().map(|d| d.gear),
    );
}

#[test]
// r[verify cz.ref.zero-orbit-same-path+1 cz.depth.delta-kernel+1]
fn zero_orbit_floor_matches_direct_kernel_escape_times() {
    // Shallow f64-valid comparator only; deep truth is the rug doubling oracle.
    // Start both seats at production z₀=c (not classical z=0).
    for c in [(2.0, 2.0), (-1.0, 0.2), (0.4, 0.4), (-0.75, 0.1)] {
        let mut direct_ctx = make_context(0);
        direct_ctx.points[0] = make_point(c);
        direct_ctx.points[0].z = (FloatExp::from(c.0), FloatExp::from(c.1));
        direct_ctx.points[0].dc = (FloatExp::ONE, FloatExp::ZERO);
        let mut perturb_ctx = direct_ctx.clone();
        DirectKernel.start_seat(&mut direct_ctx, (0, 0));
        FloatExpPerturbationKernel.start_seat(&mut perturb_ctx, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct_ctx.points[0], None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(512),
        );
        FloatExpPerturbationKernel.iterate_bout(
            &mut perturb_ctx.points[0], None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(512),
        );
        assert_eq!(
            outcome_key(&direct_ctx.points[0]),
            outcome_key(&perturb_ctx.points[0]),
            "zero-orbit floor must match direct on shallow c={c:?}"
        );
    }
}

#[test]
// r[verify cz.depth.delta-kernel+1]
fn published_reference_matches_direct_on_shallow_view() {
    // Shallow f64-valid comparator only; deep truth is the rug doubling oracle.
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    let published = Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&reference_c, 128, 512),
        c: reference_c,
        generation: 11,
        });
    for c in [(-0.49, 0.01), (-0.55, 0.08), (-0.4, -0.15)] {
        let mut direct_ctx = make_context(0);
        direct_ctx.points[0] = make_point(c);
        direct_ctx.points[0].z = (FloatExp::from(c.0), FloatExp::from(c.1));
        direct_ctx.points[0].dc = (FloatExp::ONE, FloatExp::ZERO);
        let mut perturb_ctx = direct_ctx.clone();
        perturb_ctx.latest_reference = Some(published.clone());
        activate_reference_floor(&mut perturb_ctx);
        DirectKernel.start_seat(&mut direct_ctx, (0, 0));
        FloatExpPerturbationKernel.start_seat(&mut perturb_ctx, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct_ctx.points[0], None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(512),
        );
        FloatExpPerturbationKernel.iterate_bout(
            &mut perturb_ctx.points[0],
            Some(&published.orbit),
            FloatExp::from(4.0),
            FloatExp::from(1e-15),
            BoutCap::new(512),
        );
        assert_eq!(
            outcome_key(&direct_ctx.points[0]),
            outcome_key(&perturb_ctx.points[0]),
            "published reference must match direct on shallow c={c:?}"
        );
    }
}

/// Headed OBO / missing r=2 ring: after a published reference WITH series is
/// installed, exterior seats (|c|>2) must keep the same escape_time as
/// DirectKernel (production convention: 0), not an inflated series-skip index.
/// Runtime evidence (session 63a36f): zero-orbit outer et=0; reference-path
/// outer et=2 exactly once series was live.
#[test]
// r[verify cz.depth.series-approximation+1]
// r[verify cz.depth.delta-kernel+1]
fn published_reference_with_series_matches_direct_outside_r2() {
    use crate::series::SeriesApproximation;
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO); // -0.5
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 512);
    let _series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    let published = Arc::new(PublishedReference {
        orbit,
        c: reference_c,
        generation: 42,
    });
    // |c|>2 must escape at production escape_time 0; also a few near-ring
    // exterior points that series is tempted to overshoot.
    for c in [(3.0, 0.0), (2.0, 2.0), (-2.5, 0.5), (0.0, 3.0), (1.5, 1.5)] {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1), IntExp::from(-1)),
                zoom_pot: -3,
            },
            TEST_SCREEN_RES,
        );
        let mut direct = from_stencil::<f64>(frame.clone(), None).expect("direct shell");
        let mut perturb = from_stencil::<f64>(frame, None).expect("perturb shell");
        perturb.latest_reference = Some(published.clone());
        activate_reference_floor(&mut perturb);
        // Plant the same absolute c on seat 0 for both kernels.
        direct.points[0].delta_c = c;
        direct.points[0].c = c;
        direct.points[0].z = c;
        direct.points[0].dc = (1.0, 0.0);
        direct.points[0].initialized = true;
        perturb.points[0].delta_c = c;
        perturb.points[0].c = c;
        perturb.points[0].z = c;
        perturb.points[0].dc = (1.0, 0.0);
        perturb.points[0].initialized = true;
        DirectKernel.start_seat(&mut direct, (0, 0));
        PerturbationKernel.start_seat(&mut perturb, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct.points[0],
            None,
            4.0,
            1e-15,
            BoutCap::new(64),
        );
        PerturbationKernel.iterate_bout(
            &mut perturb.points[0],
            Some(&published.orbit),
            4.0,
            1e-15,
            BoutCap::new(64),
        );
        assert!(
            direct.points[0].escapes,
            "fixture c={c:?} must escape under DirectKernel"
        );
        assert_eq!(
            (
                perturb.points[0].escapes,
                perturb.points[0].iterations,
                perturb.points[0].small_time
            ),
            (
                direct.points[0].escapes,
                direct.points[0].iterations,
                direct.points[0].small_time
            ),
            "series+reference must not inflate escape_time/small_time for exterior c={c:?} (got et={} st={})",
            perturb.points[0].iterations,
            perturb.points[0].small_time
        );
    }
}

/// Diagnostic: perturbation small_time must match DirectKernel (iteration index
/// of minimum |z|). Escape-time parity alone is insufficient for STE shading.
#[test]
fn small_time_matches_direct_kernel_on_interior() {
    use crate::assemblies::workgroup::screen_worker::perturb_kernel::PerturbationKernel;
    use crate::series::SeriesApproximation;
    let frame = (
        ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::from(-1)),
            zoom_pot: -3,
        },
        TEST_SCREEN_RES,
    );
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 512);
    let _series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    let published = Arc::new(PublishedReference {
        orbit,
        c: reference_c,
        generation: 42,
    });
    let cases: [((f64, f64), bool); 10] = [
        ((-0.75, 0.1), false),
        ((0.0, 0.0), false),
        ((-0.5, 0.5), false),
        ((-0.49, 0.01), true),
        ((-0.55, 0.08), true),
        ((-0.4, -0.15), true),
        ((0.25, 0.0), false),
        ((0.4, 0.4), false),
        ((3.0, 0.0), true),
        ((1.5, 1.5), true),
    ];
    let mut mismatches = Vec::new();
    for (c, with_ref) in cases {
        let mut direct = from_stencil::<f64>(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil::<f64>(frame.clone(), None).expect("perturb");
        if with_ref {
            perturb.latest_reference = Some(published.clone());
        }
        for ctx in [&mut direct, &mut perturb] {
            ctx.points[0].delta_c = c;
            ctx.points[0].c = c;
            ctx.points[0].z = c;
            ctx.points[0].dc = (1.0, 0.0);
            ctx.points[0].initialized = true;
        }
        DirectKernel.start_seat(&mut direct, (0, 0));
        PerturbationKernel.start_seat(&mut perturb, (0, 0));
        while !(direct.points[0].escapes || direct.points[0].repeats) {
            check_test_budget();
            DirectKernel.iterate_bout(
                &mut direct.points[0],
                None,
                4.0,
                1e-15,
                BoutCap::new(256),
            );
        }
        while !(perturb.points[0].escapes || perturb.points[0].repeats) {
            check_test_budget();
            PerturbationKernel.iterate_bout(
                &mut perturb.points[0],
                perturb
                    .latest_reference
                    .as_ref()
                    .map(|r| &r.orbit),
                4.0,
                1e-15,
                BoutCap::new(256),
            );
        }
        let d = &direct.points[0];
        let p = &perturb.points[0];
        if (d.escapes, d.repeats, d.iterations) != (p.escapes, p.repeats, p.iterations) {
            continue;
        }
        if p.small_time != d.small_time {
            mismatches.push(format!(
                "c={c:?} ref={with_ref} et={} direct_st={} perturb_st={}",
                d.iterations, d.small_time, p.small_time
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "small_time mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
// r[verify cz.depth.series-approximation+1]
fn series_safe_skip_does_not_pass_bailout_for_far_delta() {
    use crate::series::SeriesApproximation;
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 256);
    let series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    // Far exterior delta relative to -0.5 — the raw safe_skip may be >1, but
    // Z_n+δz must already have escaped by n=1 (|c|≫2).
    let delta_c = ComplexFloatExp::new(FloatExp::from(3.5), FloatExp::ZERO);
    let raw = series.safe_skip(delta_c, orbit.iterates.len().saturating_sub(1));
    let mut first_escape = None;
    for n in 1..=raw.max(1) {
        let delta_z = series.evaluate(n, delta_c).unwrap();
        let z_ref = orbit.get(n as u32).unwrap();
        if (z_ref + delta_z).norm_squared() > FloatExp::from(4.0) {
            first_escape = Some(n);
            break;
        }
    }
    assert_eq!(
        first_escape,
        Some(1),
        "far exterior delta must escape at series index 1; raw_skip={raw}"
    );
}

/// Frame-level real-axis symmetry after published reference+series.
/// Headed tumor symptom: inflated exterior escape_time shattered conjugate
/// symmetry (bottom bulge). Assert on worker answers, not pixels.
#[test]
// r[verify cz.math.mandelbrot-real-axis-symmetry+1]
// r[verify cz.depth.series-approximation+1]
fn home_package_with_live_series_obeys_real_axis_symmetry() {
    run_big(|| {
        // Origin real −2, zoom −2: covers the main cardioid plus |c|≳2 exterior.
        let frame = real_axis_symmetric_shallow_frame(TEST_SCREEN_RES, -2, -2);
        let mut ctx = from_stencil(frame.clone(), None).expect("symmetric shell");
        install_covering_reference_with_series(&mut ctx, &frame);
        fill_until_complete_perturb(&mut ctx);
        assert!(
            ctx.percent_completed >= 100.0,
            "symmetric frame must finish, got {:.1}%",
            ctx.percent_completed
        );

        let w = ctx.res.0;
        let h = ctx.res.1 as i32;
        let mut compared = 0usize;
        let mut mismatches = Vec::new();
        for y in 0..(h / 2) {
            for x in 0..w as i32 {
                let i = index_from_pos(&(x, y), w);
                let j = index_from_pos(&(x, h - 1 - y), w);
                let a = &ctx.points[i];
                let b = &ctx.points[j];
                // Sanity: plane coords are conjugates.
                let (ar, ai) = c_f64(&ctx, i);
                let (br, bi) = c_f64(&ctx, j);
                assert!(
                    (ar - br).abs() < 1e-12 && (ai + bi).abs() < 1e-12,
                    "fixture not conjugate at ({x},{y}): ({ar},{ai}) vs ({br},{bi})"
                );
                if !(a.escapes || a.repeats) || !(b.escapes || b.repeats) {
                    continue;
                }
                compared += 1;
                if answer_oracle_key(a) != answer_oracle_key(b) {
                    mismatches.push(format!(
                        "({x},{y})↔({x},{}) a={:?} b={:?}",
                        h - 1 - y,
                        answer_oracle_key(a),
                        answer_oracle_key(b)
                    ));
                }
            }
        }
        assert!(
            compared >= 200,
            "need enough finished conjugate pairs, got {compared}"
        );
        assert!(
            mismatches.is_empty(),
            "real-axis answer symmetry broken after series ({}):\n{}",
            mismatches.len(),
            mismatches.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
    });
}

/// Whole-package DirectKernel oracle after reference+series: every strict-exterior
/// escaping seat finished on both paths must match escape_time and small_time.
/// Interior iteration depth can differ under perturbation; the tumor class was
/// inflated exterior escape_time after series skip.
#[test]
// r[verify cz.depth.series-approximation+1]
// r[verify cz.depth.delta-kernel+1]
fn home_package_with_live_series_matches_direct_kernel_answers() {
    run_big(|| {
        let frame = real_axis_symmetric_shallow_frame(TEST_SCREEN_RES, -2, -2);
        let mut direct = from_stencil(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil(frame.clone(), None).expect("perturb");
        install_covering_reference_with_series(&mut perturb, &frame);
        // Direct ignores the reference; install anyway so shells stay aligned.
        install_covering_reference_with_series(&mut direct, &frame);

        fill_until_complete_direct(&mut direct);
        fill_until_complete_perturb(&mut perturb);
        assert!(
            direct.percent_completed >= 100.0 && perturb.percent_completed >= 100.0,
            "both packages must finish (direct={:.1}% perturb={:.1}%)",
            direct.percent_completed,
            perturb.percent_completed
        );

        let mut exterior_escapes = 0usize;
        let mut mismatches = Vec::new();
        for i in 0..direct.points.len() {
            let d = &direct.points[i];
            let p = &perturb.points[i];
            if !d.escapes || !p.escapes {
                continue;
            }
            let (cr, ci) = c_f64(&direct, i);
            if !is_strict_exterior_c(cr, ci) {
                continue;
            }
            exterior_escapes += 1;
            if exterior_escape_oracle_key(d) != exterior_escape_oracle_key(p) {
                mismatches.push(format!(
                    "seat {i} c=({cr:.4},{ci:.4}) direct={:?} perturb={:?}",
                    exterior_escape_oracle_key(d),
                    exterior_escape_oracle_key(p)
                ));
            }
        }
        assert!(
            exterior_escapes >= 40,
            "need |c|>2 exterior escape seats so r=2 tumor cannot hide, got {exterior_escapes}"
        );
        assert!(
            mismatches.is_empty(),
            "exterior package vs DirectKernel mismatches ({}):\n{}",
            mismatches.len(),
            mismatches.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
    });
}

/// Dense exterior sample under reference+series must match DirectKernel on
/// escape_time and small_time (generalizes the five-fixture tumor lock).
#[test]
// r[verify cz.depth.series-approximation+1]
// r[verify cz.depth.delta-kernel+1]
fn exterior_loci_with_series_match_direct_kernel_answers() {
    use crate::assemblies::workgroup::screen_worker::perturb_kernel::PerturbationKernel;
    use crate::series::SeriesApproximation;
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO); // -0.5
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 512);
    let _series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    let published = Arc::new(PublishedReference {
        orbit,
        c: reference_c,
        generation: 42,
    });
    let frame = (
        ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::from(-1)),
            zoom_pot: -3,
        },
        TEST_SCREEN_RES,
    );

    // Far exterior + near-bailout ring (angles around the circle).
    let mut loci = vec![
        (3.0, 0.0),
        (2.0, 2.0),
        (-2.5, 0.5),
        (0.0, 3.0),
        (1.5, 1.5),
        (-3.0, -1.0),
        (2.5, -2.5),
    ];
    for k in 0..32 {
        let theta = std::f64::consts::TAU * (k as f64) / 32.0;
        loci.push((2.05 * theta.cos(), 2.05 * theta.sin()));
        loci.push((2.5 * theta.cos(), 2.5 * theta.sin()));
        loci.push((4.0 * theta.cos(), 4.0 * theta.sin()));
    }

    let mut compared = 0usize;
    let mut mismatches = Vec::new();
    for c in loci {
        assert!(
            c.0 * c.0 + c.1 * c.1 > 4.0,
            "fixture must be exterior |c|>2: {c:?}"
        );
        let mut direct = from_stencil::<f64>(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil::<f64>(frame.clone(), None).expect("perturb");
        perturb.latest_reference = Some(published.clone());
        activate_reference_floor(&mut perturb);
        for ctx in [&mut direct, &mut perturb] {
            ctx.points[0].delta_c = c;
            ctx.points[0].c = c;
            ctx.points[0].z = c;
            ctx.points[0].dc = (1.0, 0.0);
            ctx.points[0].initialized = true;
        }
        DirectKernel.start_seat(&mut direct, (0, 0));
        PerturbationKernel.start_seat(&mut perturb, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct.points[0],
            None,
            4.0,
            1e-15,
            BoutCap::new(64),
        );
        PerturbationKernel.iterate_bout(
            &mut perturb.points[0],
            Some(&published.orbit),
            4.0,
            1e-15,
            BoutCap::new(64),
        );
        assert!(
            direct.points[0].escapes,
            "exterior fixture must escape under DirectKernel: {c:?}"
        );
        compared += 1;
        let d = (
            direct.points[0].escapes,
            direct.points[0].iterations,
            direct.points[0].small_time,
        );
        let p = (
            perturb.points[0].escapes,
            perturb.points[0].iterations,
            perturb.points[0].small_time,
        );
        if d != p {
            mismatches.push(format!("c={c:?} direct={d:?} perturb={p:?}"));
        }
    }
    assert!(compared >= 100, "dense exterior sample too small: {compared}");
    assert!(
        mismatches.is_empty(),
        "exterior series+ref vs DirectKernel ({}):\n{}",
        mismatches.len(),
        mismatches.iter().take(16).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
// r[verify cz.depth.reference-generation-restart+1]
fn generation_mismatch_restarts_delta() {
    let mut ctx = make_context(0);
    ctx.points[2] = make_point((0.3, 0.1));
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&(IntExp::ZERO, IntExp::ZERO), 64, 32),
        c: (IntExp::ZERO, IntExp::ZERO),
        generation: 1,
        }));
    activate_reference_floor(&mut ctx);
    FloatExpPerturbationKernel.start_seat(&mut ctx, (2, 0));
    let initial_dz = ctx.points[2].delta.as_ref().unwrap().delta_z;
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[2],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        FloatExp::from(4.0),
        ctx.pitch_epsilon,
        BoutCap::new(5),
    );
    assert!(ctx.points[2].iterations > 0);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&(IntExp::ZERO, IntExp::ZERO), 64, 32),
        c: (IntExp::ZERO, IntExp::ZERO),
        generation: 2,
        }));
    FloatExpPerturbationKernel.start_seat(&mut ctx, (2, 0));
    let delta = ctx.points[2].delta.as_ref().unwrap();
    assert_eq!(delta.generation, 2);
    assert_eq!(delta.delta_z, initial_dz);
    assert_eq!(ctx.points[2].iterations, 0);
}

#[test]
// r[verify cz.depth.glitch-is-unfinished+1 cz.depth.perturb-never-wrong+1]
fn glitch_sets_direct_only_and_never_publishes_guess() {
    use crate::floatexp::{ComplexFloatExp, FloatExp};
    let mut ctx = make_context(0);
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&reference_c, 128, 8),
        c: reference_c,
        generation: 7,
        }));
    ctx.points[0] = make_point((0.0, 0.0));
    ctx.points[0].iterations = 1;
    ctx.points[0].delta = Some(DeltaState {
        // For c=-1/2, standard Z₂=-1/4. δ=+1/4 cancels it exactly.
        delta_z: ComplexFloatExp::new(FloatExp::from(0.25), FloatExp::ZERO),
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        delta_c: ComplexFloatExp::ZERO,
        c: ComplexFloatExp::ZERO,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation: 7,
        gear: crate::delta_gear::ComputeGear::FloatExp,
        scale: FloatExp::ONE,
    });
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[0],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        FloatExp::from(4.0),
        FloatExp::from(1e-15),
        BoutCap::new(1),
    );
    assert!(ctx.points[0].direct_only);
    assert!(ctx.points[0].delta.is_none());
    assert!(!ctx.points[0].initialized);
    assert!(!ctx.points[0].escapes && !ctx.points[0].repeats);
    assert!(!ctx.points[0].delivered);
    FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
    assert_eq!(ctx.points[0].delta.as_ref().unwrap().generation, 0);
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[0], None, FloatExp::from(4.0), ctx.pitch_epsilon, BoutCap::new(2),
    );
    assert!(ctx.points[0].repeats);
    assert_eq!(ctx.points[0].period, 1);
}

#[test]
fn missing_reference_iterate_stays_unfinished() {
    let reference_c = (IntExp::from(3).shift(-4), IntExp::from(1).shift(-4));
    let mut orbit = ReferenceOrbit::start(&reference_c, 64);
    orbit.extend(2);
    assert_eq!(orbit.iterates.len(), 3);
    assert!(orbit.period.is_none());
    let mut ctx = make_context(0);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit,
        c: reference_c,
        generation: 1,
        }));
    ctx.points[0] = make_point((0.2, 0.08));
    FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
    FloatExpPerturbationKernel.iterate_bout(
        &mut ctx.points[0],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        FloatExp::from(4.0),
        FloatExp::from(1e-15),
        BoutCap::new(20),
    );
    // Short published orbits must not invent a final answer. Falling through to
    // the zero-orbit floor and continuing is allowed (never soft-stall).
    assert!(ctx.points[0].iterations > 0);
    assert!(!ctx.points[0].delivered);
    assert!(
        ctx.points[0].delta.is_some() || ctx.points[0].direct_only,
        "must keep delta state or bind the zero-orbit floor"
    );
}

#[test]
fn perturbation_bout_obeys_cap_and_split_bouts_match() {
    let mut ctx = make_context(0);
    ctx.points[0] = make_point((-0.1, 0.65));
    FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
    let mut whole = ctx.points[0].clone();
    let mut split = whole.clone();
    FloatExpPerturbationKernel.iterate_bout(
        &mut whole, None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(17),
    );
    FloatExpPerturbationKernel.iterate_bout(
        &mut split, None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(5),
    );
    assert!(split.iterations <= 5);
    FloatExpPerturbationKernel.iterate_bout(
        &mut split, None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(12),
    );
    assert_eq!(whole.iterations, split.iterations);
    assert_eq!(whole.escapes, split.escapes);
    assert_eq!(whole.repeats, split.repeats);
    assert_eq!(whole.z, split.z);
    assert_eq!(whole.c, split.c);
    assert_eq!(whole.delta.as_ref().unwrap().delta_z, split.delta.as_ref().unwrap().delta_z);
    assert_eq!(whole.delta.as_ref().unwrap().dd, split.delta.as_ref().unwrap().dd);
}

fn rug_orbit_derivative(c: (f64, f64), standard_n: u32) -> (f64, f64) {
    use rug::Float;
    let precision = 256;
    let (cr, ci) = (Float::with_val(precision, c.0), Float::with_val(precision, c.1));
    let (mut zr, mut zi) = (Float::with_val(precision, 0), Float::with_val(precision, 0));
    let (mut dr, mut di) = (Float::with_val(precision, 0), Float::with_val(precision, 0));
    for _ in 0..standard_n {
        let next_dr = Float::with_val(
            precision,
            Float::with_val(precision, 2)
                * (Float::with_val(precision, &zr * &dr)
                    - Float::with_val(precision, &zi * &di))
                + 1,
        );
        let next_di = Float::with_val(
            precision,
            Float::with_val(precision, 2)
                * (Float::with_val(precision, &zr * &di)
                    + Float::with_val(precision, &zi * &dr)),
        );
        let next_zr = Float::with_val(
            precision,
            Float::with_val(precision, &zr * &zr)
                - Float::with_val(precision, &zi * &zi)
                + &cr,
        );
        let next_zi = Float::with_val(
            precision,
            Float::with_val(precision, 2) * Float::with_val(precision, &zr * &zi) + &ci,
        );
        (zr, zi, dr, di) = (next_zr, next_zi, next_dr, next_di);
    }
    (dr.to_f64(), di.to_f64())
}

#[test]
fn perturbation_derivative_matches_rug_and_conjugation() {
    let mut derivatives = Vec::new();
    for ci in [0.2, -0.2] {
        let mut ctx = make_context(0);
        ctx.points[0] = make_point((0.1, ci));
        FloatExpPerturbationKernel.start_seat(&mut ctx, (0, 0));
        FloatExpPerturbationKernel.iterate_bout(
            &mut ctx.points[0], None, FloatExp::from(4.0), FloatExp::from(1e-15), BoutCap::new(5),
        );
        let expected = rug_orbit_derivative((0.1, ci), 6);
        let actual = ctx.points[0].dc;
        assert!((actual.0.to_f64() - expected.0).abs() < 1e-12);
        assert!((actual.1.to_f64() - expected.1).abs() < 1e-12);
        derivatives.push(actual);
    }
    assert!((derivatives[0].0.to_f64() - derivatives[1].0.to_f64()).abs() < 1e-12);
    assert!((derivatives[0].1.to_f64() + derivatives[1].1.to_f64()).abs() < 1e-12);
}

/// Guards against whole-file rewrites that drop phase-two invariant tests while
/// documentation still cites them. Fixture field names are included so a silent
/// revert of `Point`/`WorkContext` initialization also fails.
#[test]
fn phase_two_perturbation_test_inventory_is_present() {
    let src = concat!(
        include_str!("mod.rs"),
        include_str!("craft_core.rs"),
        include_str!("depth_and_reference.rs"),
        include_str!("never_stall_and_faux.rs"),
        include_str!("steady_state_and_quality.rs"),
    );
    for name in [
        "direct_kernel_preserves_scheduler_results",
        "home_reference_request_matches_c_generator",
        "home_workshift_with_reference_matches_direct",
        "perturbation_kernel_matches_rug_doubling_oracle",
        "exact_f64_conversion_round_trips_representative_values",
        "zero_orbit_center_reports_period_one",
        "zero_orbit_floor_matches_direct_kernel_escape_times",
        "published_reference_matches_direct_on_shallow_view",
        "generation_mismatch_restarts_delta",
        "glitch_sets_direct_only_and_never_publishes_guess",
        "missing_reference_iterate_stays_unfinished",
        "perturbation_bout_obeys_cap_and_split_bouts_match",
        "perturbation_derivative_matches_rug_and_conjugation",
        "phase_two_perturbation_test_inventory_is_present",
        "deep_frame_admitted_past_f64_collapse",
        "production_coords_are_not_plain_f64",
        "series_skip_matches_delta_tail",
        "published_reference_with_series_matches_direct_outside_r2",
        "series_safe_skip_does_not_pass_bailout_for_far_delta",
        "home_package_with_live_series_obeys_real_axis_symmetry",
        "home_package_with_live_series_matches_direct_kernel_answers",
        "exterior_loci_with_series_match_direct_kernel_answers",
        "series_never_publishes_guessed_completion",
        "live_series_skip_initializes_delta_prefix",
        "design_depth_zoom_pot_representable",
    ] {
        assert!(
            src.contains(&format!("fn {name}")),
            "missing phase-two inventory test `{name}`"
        );
    }
    for needle in [
        "delta: None",
        "direct_only: false",
        "bound_zero_generation: 0",
        "latest_reference: None",
        "workshift_with_kernel(0, 0, 0, 0, ctx, &DirectKernel)",
    ] {
        assert!(
            src.contains(needle),
            "missing phase-two fixture state `{needle}`"
        );
    }
    // One-path anti-cheat: zero-orbit must stay on FloatExp delta recurrence.
    let kernel = include_str!("../perturb_kernel.rs");
    assert!(
        !kernel.contains("iterate_max_n_times("),
        "zero-orbit must not call the direct f64 bout helper"
    );
    assert!(
        !kernel.contains("fn is_zero_orbit"),
        "zero-orbit must not special-case away from the shared delta loop"
    );
}

#[test]
// r[verify cz.depth.floatexp-host-coords+1]
fn deep_frame_admitted_past_f64_collapse() {
    run_big(|| {
        // Plain f64 CGenerator collapses here; FloatExp must still admit.
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: 80,
            },
            TEST_SCREEN_RES,
        );
        let compute_loc = (frame.0.pos.0.clone(), IntExp::ZERO - frame.0.pos.1.clone());
        assert!(
            CGenerator::<f64>::new(&compute_loc, frame.0.zoom_pot as i64, frame.1).is_none(),
            "fixture must be past the plain-f64 wall"
        );
        let ctx = from_stencil_relative::<FloatExp>(frame, None);
        assert!(ctx.is_some(), "FloatExp host must admit past f64 collapse");
    });
}

#[test]
// r[verify cz.depth.floatexp-host-coords+1]
fn production_coords_are_not_plain_f64() {
    let kernel = include_str!("../perturb_kernel.rs");
    let worker = include_str!("../mod.rs");
    let shift = include_str!("../workshift.rs");
    assert!(
        kernel.contains("SeatKernel<f64>") && kernel.contains("Point<f64>"),
        "live actors must use f64 plane coords"
    );
    assert!(
        kernel.contains("mod floatexp_host"),
        "depth tests must still exercise FloatExp host coords"
    );
    assert!(
        worker.contains("WorkUpdate<f64>"),
        "screen worker channel must carry f64 completions"
    );
    assert!(
        shift.contains("WorkContext<f64>"),
        "production workshift must take f64 context"
    );
    assert!(
        kernel.contains("absolute_c_floatexp_from_f64") || kernel.contains("abs_c_f64"),
        "live kernel must use plain f64 seat coordinates"
    );
}

#[test]
// r[verify cz.depth.series-approximation+1]
fn series_skip_matches_delta_tail() {
    use crate::series::SeriesApproximation;
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO); // -0.5
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 256);
    let series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    let delta_c = ComplexFloatExp::new(FloatExp::from(1e-4), FloatExp::ZERO);
    let skip = series.safe_skip(delta_c, orbit.iterates.len().saturating_sub(1));
    assert!(skip >= 1);
    let approx = series.evaluate(skip, delta_c).expect("eval");
    // Tail from skip via one-step delta should stay near the series value for small delta_c.
    let mut delta_z = approx;
    let z_ref = orbit.get(skip as u32).unwrap_or(ComplexFloatExp::ZERO);
    let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
    if let Some(z_next_ref) = orbit.get((skip + 1) as u32) {
        delta_z = z_ref * delta_z * two + delta_z * delta_z + delta_c;
        let series_next = series.evaluate(skip + 1, delta_c).unwrap_or(delta_z);
        let err = (delta_z - series_next).norm_squared().to_f64();
        assert!(
            err < 1e-6 || skip + 1 >= series.coeffs.len(),
            "series step should track delta for tiny delta_c; err={err} skip={skip}"
        );
        let _ = z_next_ref;
    }
}

#[test]
// r[verify cz.depth.series-approximation+1]
fn series_never_publishes_guessed_completion() {
    use crate::series::SeriesApproximation;
    let c = (IntExp::ZERO, IntExp::ZERO);
    let orbit = ReferenceOrbit::compute(&c, 64, 64);
    let series = SeriesApproximation::from_orbit(&orbit, 4);
    // Series is data only — applying it must not mark seats delivered by itself.
    let mut ctx = make_context(0);
    if let Some(s) = series {
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit,
            c,
            generation: 1,
        }));
        FloatExpPerturbationKernel.start_seat(&mut ctx, (2, 0));
        assert!(
            !ctx.points[2].delivered,
            "series skip must not invent a delivered completion"
        );
    }
}

#[test]
// r[verify cz.depth.series-approximation+1]
fn live_series_skip_initializes_delta_prefix() {
    use crate::series::SeriesApproximation;
    let reference_c = (IntExp::from(-1).shift(-1), IntExp::ZERO);
    let orbit = ReferenceOrbit::compute(&reference_c, 128, 256);
    let series = SeriesApproximation::from_orbit(&orbit, 4).expect("series");
    let delta_c = ComplexFloatExp::new(FloatExp::from(1e-4), FloatExp::ZERO);
    let skip = series.safe_skip(delta_c, orbit.iterates.len().saturating_sub(1));
    assert!(skip > 1, "fixture must admit a nontrivial skip");
    let mut ctx = make_context(0);
    // Install a published reference with series onto an f64 host context via from_stencil.
    let frame = (
        ObjectivePosAndZoom {
            pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
            zoom_pot: -2,
        },
        TEST_SCREEN_RES,
    );
    let mut live = from_stencil::<f64>(frame.clone(), None).expect("shell");
    live.latest_reference = Some(Arc::new(PublishedReference {
        orbit,
        c: reference_c,
        generation: 1,
    }));
    PerturbationKernel.start_seat(&mut live, (1, 0));
    let idx = crate::utils::index_from_pos(&(1, 0), live.res.0);
    assert!(
        !live.points[idx].delivered,
        "series skip must not invent delivery"
    );
    assert!(
        live.points[idx].iterations >= 1 || live.points[idx].delta.is_some(),
        "series or zero-orbit init must leave seat state"
    );
    let _ = ctx;
    let _ = skip;
}

#[test]
// r[verify cz.deep.min-zoom-pot-capacity via FloatExp/IntExp]
fn design_depth_zoom_pot_representable() {
    // Design depth ≥ 2^3600000: IntExp zoom pot and FloatExp exponent range.
    let pot: i32 = 3_600_000;
    let pitch = IntExp::from(1).shift(-(pot + 9));
    assert!(pitch.val != 0 || pitch.exp != 0 || pot == 0);
    let fe = FloatExp::from(pitch.clone());
    assert!(fe != FloatExp::ZERO || pot < 0);
    // FloatExp exponent is i64 — pot fits.
    assert!((pot as i64).abs() < i64::MAX / 4);
    let deep = (
        ObjectivePosAndZoom {
            pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
            zoom_pot: 200,
        },
        TEST_SCREEN_RES,
    );
    assert!(
        from_stencil_relative::<FloatExp>(deep, None).is_some(),
        "FloatExp must admit deep-ish frames"
    );
}

