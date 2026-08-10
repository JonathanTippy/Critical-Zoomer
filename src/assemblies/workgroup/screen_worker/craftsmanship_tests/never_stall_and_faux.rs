// ---------------------------------------------------------------------------
// Never-stall: unfinished frames must show progress every workshift
// r[verify cz.craft.wall-clock-law+1]
// r[verify cz.craft.emergent-cadence+1]
// ---------------------------------------------------------------------------

fn frame_unfinished(ctx: &WorkContext<FloatExp>) -> bool {
    ctx.points.iter().any(|p| !p.delivered)
}

fn seat_iter_sum(ctx: &WorkContext<FloatExp>) -> u64 {
    ctx.points.iter().map(|p| p.iterations as u64).sum()
}

fn shift_made_progress(ctx: &WorkContext<FloatExp>, before_sum: u64, before_completed: usize) -> bool {
    ctx.total_iterations_today > 0
        || seat_iter_sum(ctx) != before_sum
        || ctx.completed_points.len > before_completed
        || ctx.total_points_today > 0
}

/// Synthetic unfinished-heavy fixture: hard seats must keep advancing.
#[test]
fn unfinished_synthetic_workshift_never_stalls() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.attention_index = 0;
        set_attention(&mut ctx, Some((3, 0)));
        ctx.attention_current = Some((3, 0));
        // Seed queues so non-attention rotation slots still have unfinished work
        // (empty queues would exit the shift with zero progress — that is starvation,
        // not a valid unfinished-heavy fixture).
        for y in 0..ctx.res.1 as i32 {
            for x in 0..ctx.res.0 as i32 {
                let i = index_from_pos(&(x, y), ctx.res.0);
                if !ctx.points[i].delivered {
                    ctx.out_queue.push_back(((x, y), 0));
                    ctx.edge_queue.push_back(((x, y), 0));
                }
            }
        }

        let mut zero_streak = 0u32;
        const MAX_ZERO: u32 = 2;
        // Keep the shift count modest: release builds can burn through the
        // near-cusp SLOW fixture's period-detection budget if attention holds
        // one seat for too many 10ms shifts.
        for _ in 0..12 {
            if !frame_unfinished(&ctx) {
                break;
            }
            let before = seat_iter_sum(&ctx);
            let before_done = ctx.completed_points.len;
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            if shift_made_progress(&ctx, before, before_done) {
                zero_streak = 0;
            } else {
                zero_streak += 1;
            }
            assert!(
                zero_streak < MAX_ZERO,
                "zero-progress streak={zero_streak} on unfinished synthetic frame"
            );
            let _ = work_update(&mut ctx);
            // Keep unfinished seats visible to the rotation after drains/rotates.
            if ctx.out_queue.is_empty() {
                for y in 0..ctx.res.1 as i32 {
                    for x in 0..ctx.res.0 as i32 {
                        let i = index_from_pos(&(x, y), ctx.res.0);
                        if !ctx.points[i].delivered {
                            ctx.out_queue.push_back(((x, y), 0));
                        }
                    }
                }
            }
        }
        assert!(
            ctx.points.iter().skip(3).any(|p| !p.delivered),
            "SLOW seats must remain unfinished"
        );
    });
}

/// Home view under production FloatExpPerturbationKernel (zero-orbit floor).
#[test]
fn unfinished_home_workshift_never_stalls() {
    run_big(|| {
        let mut ctx = from_stencil(home_frame(), None).expect("home");
        let mut zero_streak = 0u32;
        const MAX_ZERO: u32 = 2;
        for _ in 0..8 {
            assert!(frame_unfinished(&ctx), "home must still be unfinished early");
            let before = seat_iter_sum(&ctx);
            let before_done = ctx.completed_points.len;
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            if shift_made_progress(&ctx, before, before_done) {
                zero_streak = 0;
            } else {
                zero_streak += 1;
            }
            assert!(
                zero_streak < MAX_ZERO,
                "home zero-progress streak={zero_streak}"
            );
            let _ = work_update(&mut ctx);
        }
    });
}

/// Installing a published reference mid-fill may restart deltas, but must not
/// open a multi-shift zero-progress window.
#[test]
fn reference_install_mid_fill_keeps_shift_progress() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let frame = home_frame();
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        for _ in 0..3 {
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(frame_unfinished(&ctx));

        let req = select_reference_request::<FloatExp>(None, &frame);
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        }));

        let mut zero_streak = 0u32;
        const MAX_ZERO: u32 = 2;
        for _ in 0..8 {
            if !frame_unfinished(&ctx) {
                break;
            }
            let before = seat_iter_sum(&ctx);
            let before_done = ctx.completed_points.len;
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            if shift_made_progress(&ctx, before, before_done) {
                zero_streak = 0;
            } else {
                zero_streak += 1;
            }
        assert!(
            zero_streak < MAX_ZERO,
            "reference install mid-fill created zero-progress window (streak={zero_streak})"
        );
            let _ = work_update(&mut ctx);
        }
    });
}

/// Developer repro: `-0.161913425661 + 1.035546905361i mag 2^20`.
/// Classic perturbation glitch blobs when an uncovered sticky reference from a
/// prior view is carried across the zoom path (dead-reckon goto is fine).
fn hard_minibrot_frame(res: (u32, u32)) -> (ObjectivePosAndZoom, (u32, u32)) {
    use crate::assemblies::headgroup::window::coords::{f64_to_intexp, ul_for_center};
    let loc = ul_for_center(
        f64_to_intexp(-0.161913425661),
        f64_to_intexp(1.035546905361),
        20,
        res,
    );
    (loc, res)
}

fn frame_at_center(re: f64, im: f64, pot: i32, res: (u32, u32)) -> (ObjectivePosAndZoom, (u32, u32)) {
    use crate::assemblies::headgroup::window::coords::{f64_to_intexp, ul_for_center};
    (ul_for_center(f64_to_intexp(re), f64_to_intexp(im), pot, res), res)
}

fn fill_until<K: SeatKernel<FloatExp>>(ctx: &mut WorkContext<FloatExp>, kernel: &K, shifts: u32) {
    for _ in 0..shifts {
        if ctx.points.iter().all(|p| p.delivered) {
            break;
        }
        ctx.attention_index = 0;
        workshift_with_kernel(0, 0, 0, 0, ctx, kernel);
        let _ = work_update(ctx);
    }
}

fn disagree_rate(a: &WorkContext<FloatExp>, b: &WorkContext<FloatExp>) -> (usize, usize) {
    let n = a.points.len().min(b.points.len());
    let mut disagree = 0usize;
    let mut compared = 0usize;
    for i in 0..n {
        if !a.points[i].delivered || !b.points[i].delivered {
            continue;
        }
        compared += 1;
        if outcome_key(&a.points[i]) != outcome_key(&b.points[i]) {
            disagree += 1;
        }
    }
    (disagree, compared)
}

/// Faux-user zoom path to hard minibrot (IntExp). Uncovered sticky from a
/// home-class prior must be dropped; forced uncovered sticky reproduces
/// glitch-blob disagreement vs DirectKernel.
#[test]
// r[verify cz.depth.reference-coverage+1]
// r[verify cz.ui.goto-absolute-center+1]
fn faux_user_zoom_to_hard_minibrot_matches_direct() {
    run_big(|| {
        use crate::assemblies::headgroup::window::coords::{
            commands_from_goto_line, ul_for_center, viewport_center,
        };
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;

        let res = (48u32, 32u32);
        // Dead-reckon: IntExp goto line applied through the headgroup command path.
        let goto = "-0.161913425661 + 1.035546905361i mag 2^20";
        let cmds = commands_from_goto_line(goto).expect("goto");
        let mut dead_nav = SamplingContext {
            screen: None,
            screen_size: res,
            location: ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, res),
            updated: false,
            mouse_drag_start: None,
        };
        transform(cmds, &mut dead_nav);
        let hard = (dead_nav.location.clone(), res);
        let (dre, dim) = viewport_center(&hard.0, res);

        // Zoom path: stepwise IntExp mags toward the same center (not f64 centers).
        let mut zoom_nav = SamplingContext {
            screen: None,
            screen_size: res,
            location: ul_for_center(IntExp::from(-2), IntExp::from(-2), -2, res),
            updated: false,
            mouse_drag_start: None,
        };
        for pot in [0, 4, 8, 12, 16, 20] {
            let step = format!(
                "{} {}i mag 2^{}"
                , crate::assemblies::headgroup::window::coords::format_intexp_readout(&dre)
                , {
                    let s = crate::assemblies::headgroup::window::coords::format_intexp_readout(&dim);
                    if s.starts_with('-') { s } else { format!("+ {s}") }
                }
                , pot
            );
            // format_location_readout style: re ± imi
            let line = crate::assemblies::headgroup::window::coords::format_location_readout(
                &dre, &dim, pot,
            );
            let _ = step;
            transform(
                commands_from_goto_line(&line).expect("zoom step"),
                &mut zoom_nav,
            );
        }
        assert_eq!(zoom_nav.location.zoom_pot, 20);
        let (zre, zim) = viewport_center(&zoom_nav.location, res);
        assert!(
            (f64::from(zre.clone()) - f64::from(dre.clone())).abs() < 1e-9
                && (f64::from(zim.clone()) - f64::from(dim.clone())).abs() < 1e-9,
            "IntExp zoom path must land on the same center as dead-reckon goto"
        );

        // Blob repro prior: a wide home-class view whose reference `c` lies
        // outside the hard minibrot viewport. Same-center shallow zoom would
        // still cover (center ∈ hard frame) — that is not the uncovered-sticky bug.
        let prior_frame = (
            ul_for_center(IntExp::from(-2), IntExp::from(-2), -2, res),
            res,
        );
        let mut prior = from_stencil(prior_frame.clone(), None).expect("prior");
        let prior_req = select_reference_request::<FloatExp>(None, &prior_frame);
        let uncovered = Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&prior_req.c, prior_req.precision_bits, 256),
            c: prior_req.c.clone(),
            generation: 3,
        });
        prior.latest_reference = Some(uncovered.clone());
        assert!(
            !crate::assemblies::workgroup::reference_worker::reference_c_covers_frame(
                &uncovered.c,
                &hard,
            ),
            "home-class reference must not cover the hard view (blob repro precondition)"
        );

        let mut clean = from_stencil(hard.clone(), Some((prior, prior_frame.0)))
            .expect("hard");
        assert!(
            clean.latest_reference.is_none(),
            "uncovered sticky reference must not install into the hard view"
        );

        let mut direct = from_stencil(hard.clone(), None).expect("hard direct");
        fill_until(&mut direct, &DirectKernel, 40);
        fill_until(&mut clean, &FloatExpPerturbationKernel, 40);
        let (disagree, compared) = disagree_rate(&direct, &clean);
        assert!(
            compared > 0,
            "expected some delivered seats on the hard fixture"
        );
        assert!(
            disagree * 100 / compared.max(1) < 5,
            "clean zero-orbit path diverged: {disagree}/{compared}"
        );

        // Hazardous short covering center ref (old length-wall publish): still
        // can disagree with DirectKernel — why production publishes only
        // period/escape, never truncated orbits.
        let hard_req = select_reference_request::<FloatExp>(None, &hard);
        let short_covering = Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&hard_req.c, hard_req.precision_bits, 64),
            c: hard_req.c.clone(),
            generation: 4,
        });
        assert!(
            crate::assemblies::workgroup::reference_worker::reference_c_covers_frame(
                &short_covering.c,
                &hard,
            ),
            "short center ref must cover the hard view"
        );
        assert!(
            !short_covering.orbit.escaped && short_covering.orbit.period.is_none(),
            "fixture needs an incomplete truncated orbit"
        );
        let mut blob = from_stencil(hard.clone(), None).expect("blob");
        blob.latest_reference = Some(short_covering);
        activate_reference_floor(&mut blob);
        fill_until(&mut blob, &FloatExpPerturbationKernel, 40);
        let (blob_disagree, blob_compared) = disagree_rate(&direct, &blob);
        assert!(
            blob_compared > 0 && blob_disagree * 100 / blob_compared.max(1) >= 5,
            "short covering incomplete ref should disagree vs Direct; got {blob_disagree}/{blob_compared}"
        );

        // Dead-reckon control: fresh shell with no sticky prior (goto semantics).
        let mut dead = from_stencil(hard, None).expect("dead reckon");
        fill_until(&mut dead, &FloatExpPerturbationKernel, 40);
        let (dead_disagree, dead_compared) = disagree_rate(&direct, &dead);
        assert!(
            dead_compared > 0 && dead_disagree * 100 / dead_compared.max(1) < 5,
            "dead-reckon (no sticky) diverged: {dead_disagree}/{dead_compared}"
        );
    });
}

/// PPS/progress flatline: unfinished frame must not run shifts with zero
/// completions and zero iterations (the headed "pps drops to 0" stall).
#[test]
fn unfinished_frame_never_zero_pps_streak() {
    run_big(|| {
        let mut ctx = from_stencil(home_frame(), None).expect("home");
        let mut zero_pps = 0u32;
        for _ in 0..12 {
            if !frame_unfinished(&ctx) {
                break;
            }
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            let ppsish = ctx.total_points_today + ctx.total_iterations_today;
            if ppsish == 0 {
                zero_pps += 1;
            } else {
                zero_pps = 0;
            }
            assert!(
                zero_pps < 2,
                "pps-like progress dropped to 0 for {zero_pps} shifts while unfinished"
            );
            let _ = work_update(&mut ctx);
        }
    });
}



#[test]
// r[verify cz.depth.c-generator-fails-closed+1]
fn zoom_past_f64_absolute_wall_admits_replace() {
    use crate::assemblies::workgroup::c_generator::admit_generator;
    run_big(|| {
        let compute_loc = (IntExp::from(-1).shift(-1), IntExp::ZERO);
        let res = (1280u32, 720u32);
        let zoom_pot = 50i64;
        let view_center = view_center_compute(&compute_loc, zoom_pot as i32, res);
        assert!(
            CGenerator::<f64>::new(&compute_loc, zoom_pot, res).is_none(),
            "absolute f64 must collapse past the precision wall"
        );
        assert!(
            admit_generator::<f64>(&compute_loc, zoom_pot, res, None, &view_center).is_some(),
            "stencil admission gate must admit relative fallback"
        );
    });
}

#[test]
// r[verify cz.depth.c-generator-fails-closed+1]
fn from_stencil_carried_ref_anchors_to_ref_c() {
    use crate::assemblies::workgroup::reference_worker::{
        reference_c_covers_frame, select_reference_request, PublishedReference,
    };
    use std::sync::Arc;
    use crate::reference::ReferenceOrbit;
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 50,
            },
            (1280u32, 720u32),
        );
        let mut shell = from_stencil::<f64>(frame.clone(), None).expect("deep shell");
        let ref_req = select_reference_request::<f64>(None, &frame);
        assert!(reference_c_covers_frame(&ref_req.c, &frame));
        let orbit = ReferenceOrbit::compute(&ref_req.c, ref_req.precision_bits, 4096);
        shell.latest_reference = Some(Arc::new(PublishedReference {
            orbit,
            c: ref_req.c.clone(),
            generation: 11,
        }));
        let carried =
            from_stencil(frame.clone(), Some((shell, frame.0.clone()))).expect("carried");
        assert!(carried.coords_are_relative);
        assert_eq!(carried.coord_anchor, ref_req.c);
        assert_eq!(carried.generator_generation, 11);
    });
}

#[test]
// r[verify cz.depth.c-generator-fails-closed+1]
fn reference_install_rebuilds_c_generator() {
    use crate::assemblies::workgroup::reference_worker::{
        reference_c_covers_frame, select_reference_request, PublishedReference,
    };
    use crate::reference::ReferenceOrbit;
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 50,
            },
            (1280u32, 720u32),
        );
        let mut ctx = from_stencil::<f64>(frame.clone(), None).expect("shell");
        let ref_req = select_reference_request::<f64>(None, &frame);
        assert!(reference_c_covers_frame(&ref_req.c, &frame));
        let published = PublishedReference {
            orbit: ReferenceOrbit::compute(&ref_req.c, ref_req.precision_bits, 4096),
            c: ref_req.c.clone(),
            generation: 42,
        };
        let compute_loc = (
            frame.0.pos.0.clone(),
            IntExp::ZERO - frame.0.pos.1.clone(),
        );
        assert!(rebuild_generator_for_reference(
            &mut ctx,
            &compute_loc,
            frame.0.zoom_pot as i64,
            frame.1,
            &published,
        ));
        assert_eq!(ctx.coord_anchor, ref_req.c);
        assert_eq!(ctx.generator_generation, 42);
    });
}

#[test]
// r[verify cz.perf.pps-selected-kernel+1]
fn deep_relative_shell_hard_bumps_to_pert() {
    use crate::assemblies::structs::KernelMode;
    use crate::assemblies::workgroup::screen_worker::classify_kernel_mode;
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 50,
            },
            (1280u32, 720u32),
        );
        let ctx = from_stencil::<f64>(frame, None).expect("deep shell");
        assert!(ctx.coords_are_relative);
        assert!(!ctx.reference_floor_active);
        assert_eq!(
            classify_kernel_mode(&ctx),
            KernelMode::Pert,
            "relative f64 admission must hard-bump to perturbation"
        );
    });
}

#[test]
// r[verify cz.depth.compute-gear+1]
fn relative_shell_init_uses_f64_gear_not_floatexp() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::delta_gear::ComputeGear;
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 50,
            },
            (64u32, 64u32),
        );
        let mut ctx = from_stencil::<f64>(frame, None).expect("relative shell");
        assert!(ctx.coords_are_relative);
        let center = ((ctx.res.0 / 2) as i32, (ctx.res.1 / 2) as i32);
        PerturbationKernel.start_seat(&mut ctx, center);
        let index = index_from_pos(&center, ctx.res.0);
        let gear = ctx.points[index]
            .delta
            .as_ref()
            .expect("delta")
            .gear;
        assert_eq!(
            gear,
            ComputeGear::F64,
            "bootstrapped relative reference must use f64 gear at depth"
        );
        assert!(
            ctx.latest_reference.is_some(),
            "relative shell must bootstrap view-center reference"
        );
        assert!(ctx.perturbation_reference_active());
    });
}

#[test]
// r[verify cz.depth.gear-hud+2]
// r[verify cz.depth.compute-gear+1]
fn deep_view_gear_floor_stays_scaled_after_fill() {
    use crate::delta_gear::ComputeGear;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    run_big(|| {
        let res = (64u32, 48u32);
        // Issue #5 locus past absolute collapse.
        let frame = (
            ul_for_center(
                decimal_str_to_intexp("-0.164757401886").unwrap(),
                decimal_str_to_intexp("1.039500696795").unwrap(),
                48,
                res,
            ),
            res,
        );
        let mut ctx = from_stencil::<f64>(frame, None).expect("shell");
        assert!(ctx.coords_are_relative);
        assert_eq!(
            ctx.view_gear,
            ComputeGear::ScaledF64,
            "deep relative view_gear floor must be ScaledF64, not F64"
        );
        for _ in 0..80 {
            if ctx.points.iter().all(|p| p.delivered) {
                break;
            }
            workshift(16_000_000, 2, 4, 150, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
        }
        assert!(
            ctx.points.iter().filter(|p| p.delivered).count() > res.0 as usize,
            "need delivered seats"
        );
        // Extra idle shifts used to snap HUD back to F64 via refresh_active_gear.
        for _ in 0..5 {
            workshift(16_000_000, 2, 4, 150, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
        }
        assert_ne!(
            ctx.active_gear,
            ComputeGear::F64,
            "completed deep frame must not report gear:F64 (got {:?})",
            ctx.active_gear
        );
        assert!(
            matches!(
                ctx.active_gear,
                ComputeGear::ScaledF64 | ComputeGear::FloatExp | ComputeGear::Mixed
            ),
            "active gear must stay at/above ScaledF64 floor (got {:?})",
            ctx.active_gear
        );

        // Pot 43 is still absolute-admissible but pitch≈ulp — prefer relative.
        let near = (
            ul_for_center(
                decimal_str_to_intexp("-0.164757401886").unwrap(),
                decimal_str_to_intexp("1.039500696795").unwrap(),
                43,
                res,
            ),
            res,
        );
        let near_ctx = from_stencil::<f64>(near, None).expect("pot43");
        assert!(
            near_ctx.coords_are_relative,
            "pot43 must prefer relative before hard absolute collapse"
        );
        assert_eq!(near_ctx.view_gear, ComputeGear::ScaledF64);
    });
}

#[test]
// r[verify cz.depth.floatexp-host-coords+1]
// r[verify cz.depth.delta-kernel+1]
/// Pin A: deep exterior must escape, never false "in"/repeats (flat black).
/// Fails when zero-orbit/soft-continue puts generator delta_c in the absolute-c slot.
fn pin_exterior_not_marked_in_at_zoom_52() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    run_big(|| {
        let res = (8u32, 8u32);
        let frame = (
            ul_for_center(
                decimal_str_to_intexp("0.747115302704").unwrap(),
                decimal_str_to_intexp("0.562484784463").unwrap(),
                52,
                res,
            ),
            res,
        );
        let mut ctx = from_stencil::<f64>(frame, None).expect("shell");
        assert!(ctx.coords_are_relative);
        assert!(
            ctx.latest_reference.is_some() && ctx.perturbation_reference_active(),
            "relative shell must bootstrap a reference (escaped ok)"
        );
        // Production path: bounded workshifts, not a single uncapped bout.
        for _ in 0..8_000 {
            if ctx.points.iter().all(|p| p.delivered || p.escapes || p.repeats) {
                break;
            }
            workshift(16_000_000, 2, 4, 150, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
        }
        let mut escapes = 0usize;
        let mut repeats = 0usize;
        let mut unfinished = 0usize;
        let mut bad = Vec::new();
        for (idx, p) in ctx.points.iter().enumerate() {
            if p.repeats {
                repeats += 1;
                bad.push(format!(
                    "seat{idx} repeats iters={} (false interior)",
                    p.iterations
                ));
            } else if p.escapes {
                escapes += 1;
            } else {
                unfinished += 1;
            }
        }
        assert!(
            escapes + repeats >= (res.0 * res.1) as usize / 2,
            "need finished exterior seats, got escapes={escapes} repeats={repeats} unfinished={unfinished}"
        );
        assert!(
            bad.is_empty(),
            "exterior seats marked in/repeats (flat black):\n{}",
            bad.join("\n")
        );
        assert_eq!(
            repeats, 0,
            "no seat may be interior at this exterior locus (got {repeats} repeats, {escapes} escapes)"
        );
        // Keep a direct-kernel bout check too: soft-continue δc slot must not be generator δc.
        let held_ref = ctx.latest_reference.clone();
        let mut bout_bad = Vec::new();
        for y in 0..res.1 as i32 {
            for x in 0..res.0 as i32 {
                let pos = (x, y);
                let idx = index_from_pos(&pos, res.0);
                // Fresh seat iterate snapshot for soft-continue invariant.
                let mut seat = ctx.points[idx].clone();
                seat.delivered = false;
                seat.escapes = false;
                seat.repeats = false;
                seat.iterations = 0;
                seat.delta = None;
                seat.direct_only = false;
                ctx.points[idx] = seat;
                PerturbationKernel.start_seat(&mut ctx, pos);
                PerturbationKernel.iterate_bout(
                    &mut ctx.points[idx],
                    held_ref.as_ref().map(|r| &r.orbit),
                    4.0,
                    ctx.pitch_epsilon,
                    BoutCap::new(4096),
                );
                let p = &ctx.points[idx];
                if p.repeats || !p.escapes {
                    bout_bad.push(format!(
                        "({x},{y}) escapes={} repeats={} iters={}",
                        p.escapes, p.repeats, p.iterations
                    ));
                }
            }
        }
        assert!(
            bout_bad.is_empty(),
            "kernel bout marked exterior as in:\n{}",
            bout_bad.join("\n")
        );
    });
}

#[test]
// r[verify cz.depth.floatexp-host-coords+1]
// r[verify cz.depth.delta-kernel+1]
/// Pin B: at mag 2^49, generator delta_c stays per-seat and membership is not one shared blob.
fn pin_not_blocky_delta_c_at_zoom_49() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    run_big(|| {
        let res = (8u32, 8u32);
        let frame = (
            ul_for_center(
                decimal_str_to_intexp("0.360069520505").unwrap(),
                decimal_str_to_intexp("0.613443210714").unwrap(),
                49,
                res,
            ),
            res,
        );
        let mut ctx = from_stencil::<f64>(frame, None).expect("shell");
        assert!(ctx.coords_are_relative);
        assert!(
            ctx.latest_reference.is_some() && ctx.perturbation_reference_active(),
            "relative shell must bootstrap a view-center reference (escaped ok)"
        );
        let mut delta_c_bits = std::collections::HashSet::new();
        let seats = (res.0 * res.1) as usize;
        for y in 0..res.1 as i32 {
            for x in 0..res.0 as i32 {
                let pos = (x, y);
                let idx = index_from_pos(&pos, res.0);
                PerturbationKernel.start_seat(&mut ctx, pos);
                let lc = ctx.points[idx].delta_c;
                delta_c_bits.insert((lc.0.to_bits(), lc.1.to_bits()));
            }
        }
        assert!(
            delta_c_bits.len() >= seats * 3 / 4,
            "generator delta_c must stay per-seat ({} unique of {seats})",
            delta_c_bits.len()
        );
        for _ in 0..8_000 {
            if ctx.points.iter().all(|p| p.delivered || p.escapes || p.repeats) {
                break;
            }
            workshift(16_000_000, 2, 4, 150, &mut ctx, None);
            while ctx.completed_points.try_pop().is_some() {}
        }
        let mut membership = std::collections::HashSet::new();
        let mut escape_z_bits = std::collections::HashSet::new();
        for p in &ctx.points {
            if p.escapes || p.repeats {
                membership.insert((p.escapes, p.repeats, p.iterations));
            }
            if p.escapes {
                escape_z_bits.insert((p.z.0.to_bits(), p.z.1.to_bits()));
            }
        }
        assert!(
            membership.len() > 1 || escape_z_bits.len() >= 4,
            "degenerate membership/blocky: membership_classes={} escape_z={}",
            membership.len(),
            escape_z_bits.len()
        );
        assert!(
            !membership.is_empty(),
            "no finished seats — kernel did not run"
        );
    });
}

#[test]
// r[verify cz.depth.floatexp-host-coords+1]
fn c_intexp_add_distinct_per_seat_at_user_zoom_49() {
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    use super::workshift::{from_stencil, c_for_seat_f64, DirectKernel, SeatKernel};
    run_big(|| {
        let res = (64u32, 64u32);
        let zoom_pot = 49i32;
        let center_re = decimal_str_to_intexp("0.360069520505").unwrap();
        let center_im = decimal_str_to_intexp("0.613443210714").unwrap();
        let frame = (ul_for_center(center_re, center_im, zoom_pot, res), res);
        let mut ctx = from_stencil::<f64>(frame, None).expect("shell");
        assert!(ctx.coords_are_relative);

        let mut naive = std::collections::HashSet::<(u64, u64)>::new();
        let mut distinct = std::collections::HashSet::<(u64, u64)>::new();
        for y in 0..8i32 {
            for x in 0..8i32 {
                let pos = (x, y);
                DirectKernel.start_seat(&mut ctx, pos);
                let idx = crate::utils::index_from_pos(&pos, ctx.res.0);
                let lc = ctx.points[idx].delta_c;
                let bad = (
                    f64::from(ctx.coord_anchor.0.clone()) + lc.0,
                    f64::from(ctx.coord_anchor.1.clone()) + lc.1,
                );
                let good = c_for_seat_f64(&ctx, lc);
                naive.insert((bad.0.to_bits(), bad.1.to_bits()));
                distinct.insert((good.0.to_bits(), good.1.to_bits()));
            }
        }
        assert_eq!(
            naive.len(),
            1,
            "naive f64 anchor+delta_c collapses seats ({} unique)",
            naive.len()
        );
        assert!(
            distinct.len() <= 4,
            "f64 c cannot resolve per-seat pitch at zoom 49 ({} unique)",
            distinct.len()
        );

        let mut fe_distinct = std::collections::HashSet::<(u64, i64)>::new();
        let mut delta_c_distinct = std::collections::HashSet::<(u64, u64)>::new();
        for y in 0..8i32 {
            for x in 0..8i32 {
                let pos = (x, y);
                DirectKernel.start_seat(&mut ctx, pos);
                let idx = crate::utils::index_from_pos(&pos, ctx.res.0);
                let lc = ctx.points[idx].delta_c;
                let fe = super::workshift::c_floatexp_from_delta_c(lc, &ctx.coord_anchor);
                fe_distinct.insert((fe.re.mantissa.to_bits(), fe.re.exponent));
                fe_distinct.insert((fe.im.mantissa.to_bits(), fe.im.exponent));
                delta_c_distinct.insert((lc.0.to_bits(), lc.1.to_bits()));
            }
        }
        assert!(
            delta_c_distinct.len() >= 48,
            "generator delta_c must vary per seat at zoom 49 ({} unique)",
            delta_c_distinct.len()
        );
        assert!(
            fe_distinct.len() >= 2,
            "FloatExp c mantissa/exponent must vary at zoom 49 ({} unique pairs)",
            fe_distinct.len()
        );
    });
}

#[test]
// r[verify cz.depth.delta-kernel+1]
// r[verify cz.perf.pps-selected-kernel+1]
fn relative_perturb_matches_direct_at_user_zoom_49() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    use crate::reference::ReferenceOrbit;
    use std::sync::Arc;
    run_big(|| {
        let res = (32u32, 32u32);
        let zoom_pot = 49i32;
        let center_re = decimal_str_to_intexp("0.360069520505").unwrap();
        let center_im = decimal_str_to_intexp("0.613443210714").unwrap();
        let frame = (ul_for_center(center_re, center_im, zoom_pot, res), res);
        let mut direct = from_stencil::<f64>(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil::<f64>(frame, None).expect("perturb");
        assert!(direct.coords_are_relative);
        let anchor = (
            direct.coord_anchor.0.clone(),
            direct.coord_anchor.1.clone(),
        );
        let published = Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&anchor, 128, 4096),
            c: anchor,
            generation: 1,
        });
        direct.latest_reference = Some(published.clone());
        perturb.latest_reference = Some(published);

        let mut compared = 0usize;
        let mut mismatches = Vec::new();
        for y in 0..res.1 as i32 {
            for x in 0..res.0 as i32 {
                let pos = (x, y);
                let idx = index_from_pos(&pos, res.0);
                DirectKernel.start_seat(&mut direct, pos);
                PerturbationKernel.start_seat(&mut perturb, pos);
                DirectKernel.iterate_bout(
                    &mut direct.points[idx],
                    None,
                    4.0,
                    direct.pitch_epsilon,
                    BoutCap::new(512),
                );
                PerturbationKernel.iterate_bout(
                    &mut perturb.points[idx],
                    perturb.latest_reference.as_ref().map(|r| &r.orbit),
                    4.0,
                    perturb.pitch_epsilon,
                    BoutCap::new(512),
                );
                let d = &direct.points[idx];
                let p = &perturb.points[idx];
                if !(d.escapes || d.repeats) || !(p.escapes || p.repeats) {
                    continue;
                }
                compared += 1;
                if (d.escapes, d.iterations, d.repeats, d.small_time)
                    != (p.escapes, p.iterations, p.repeats, p.small_time)
                {
                    mismatches.push(format!(
                        "({x},{y}) direct=({},{},{},{}) perturb=({},{},{},{})",
                        d.escapes,
                        d.iterations,
                        d.repeats,
                        d.small_time,
                        p.escapes,
                        p.iterations,
                        p.repeats,
                        p.small_time,
                    ));
                }
            }
        }
        assert!(compared >= 50, "need finished seats, got {compared}");
        assert!(
            mismatches.is_empty(),
            "perturb vs direct at user zoom 49 with ref ({}):\n{}",
            mismatches.len(),
            mismatches.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
    });
}

#[test]
// r[verify cz.perf.pps-selected-kernel+1]
// r[verify cz.depth.delta-kernel+1]
// r[verify cz.depth.oracle-gear+1]
/// Headed report: (0.95703125, −0.08984375i) at 2^74 went black on escaping seats.
/// f64 DirectKernel is not the deep membership oracle — compare pert to Oracle gear.
fn deep_relative_exterior_not_instant_black_at_reported_location() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    use crate::floatexp::FloatExp;
    use crate::gearbox::oracle::{OracleAnswer, OracleKernel};
    use super::workshift::c_floatexp_from_delta_c;
    run_big(|| {
        let res = (32u32, 32u32);
        let zoom_pot = 74i32;
        let center_re = decimal_str_to_intexp("0.95703125").unwrap();
        let center_im = decimal_str_to_intexp("-0.08984375").unwrap();
        let obj = ul_for_center(center_re, center_im, zoom_pot, res);
        let frame = (obj, res);
        let mut perturb = from_stencil::<f64>(frame, None).expect("perturb shell");
        assert!(
            perturb.coords_are_relative,
            "zoom 74 must use relative f64 admission"
        );
        let held_ref = perturb.latest_reference.clone();
        assert!(
            held_ref.is_some() && perturb.perturbation_reference_active(),
            "relative shell must bootstrap a reference"
        );

        let mut exterior_compared = 0usize;
        let mut instant_black = Vec::new();
        let mut mismatches = Vec::new();
        let oracle = OracleKernel;
        let r2 = FloatExp::from(4.0);
        let eps = FloatExp::from(1e-30);
        for y in 0..res.1 as i32 {
            for x in 0..res.0 as i32 {
                let pos = (x, y);
                let idx = index_from_pos(&pos, res.0);
                PerturbationKernel.start_seat(&mut perturb, pos);
                let c_fe = c_floatexp_from_delta_c(
                    perturb.points[idx].delta_c,
                    &perturb.coord_anchor,
                );
                let oracle_ans = oracle.conclude((c_fe.re, c_fe.im), r2, eps, 512);
                let oracle_escapes = matches!(oracle_ans, OracleAnswer::Escapes { .. });
                let oracle_et = match oracle_ans {
                    OracleAnswer::Escapes { escape_time } => escape_time,
                    OracleAnswer::Repeats { iterations } => iterations,
                    OracleAnswer::Unfinished { iterations, .. } => iterations,
                };
                if !oracle_escapes {
                    continue;
                }
                PerturbationKernel.iterate_bout(
                    &mut perturb.points[idx],
                    held_ref.as_ref().map(|r| &r.orbit),
                    4.0,
                    perturb.pitch_epsilon,
                    BoutCap::new(512),
                );
                let p = &perturb.points[idx];
                exterior_compared += 1;
                if oracle_et > 2 && p.escapes && p.iterations <= 2 {
                    instant_black.push(format!(
                        "({x},{y}) oracle_et={oracle_et} perturb_et={}",
                        p.iterations
                    ));
                }
                if !p.escapes || p.iterations != oracle_et {
                    mismatches.push(format!(
                        "({x},{y}) oracle_et={oracle_et} perturb={:?}",
                        (p.escapes, p.iterations, p.repeats)
                    ));
                }
            }
        }
        assert!(
            exterior_compared >= 20,
            "need escaping exterior seats in fixture, got {exterior_compared}"
        );
        assert!(
            instant_black.is_empty(),
            "perturb must not instant-black exterior (escape_time≤2):\n{}",
            instant_black.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
        assert!(
            mismatches.is_empty(),
            "perturb vs Oracle mismatches ({}):\n{}",
            mismatches.len(),
            mismatches.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
    });
}

#[test]
// r[verify cz.perf.pps-selected-kernel+1]
fn relative_shell_perturbation_center_is_interior_at_depth() {
    use super::perturb_kernel::PerturbationKernel;
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 50,
            },
            (64u32, 64u32),
        );
        let mut ctx = from_stencil::<f64>(frame, None).expect("relative shell");
        assert!(ctx.coords_are_relative);
        let center = ((ctx.res.0 / 2) as i32, (ctx.res.1 / 2) as i32);
        PerturbationKernel.start_seat(&mut ctx, center);
        let index = index_from_pos(&center, ctx.res.0);
        let held = ctx.latest_reference.take();
        let orbit = None;
        PerturbationKernel.iterate_bout(
            &mut ctx.points[index],
            orbit,
            4.0,
            ctx.pitch_epsilon,
            BoutCap::new(4096),
        );
        ctx.latest_reference = held;
        let p = &ctx.points[index];
        assert!(
            p.repeats || p.iterations > 0,
            "relative perturbation must not instant-black-exterior at depth; escapes={} repeats={} iters={}",
            p.escapes,
            p.repeats,
            p.iterations
        );
        if p.escapes {
            assert!(
                p.iterations > 2,
                "collapsed c escapes at iteration 0–2 (black); got {}",
                p.iterations
            );
        }
    });
}

#[test]
fn f64_deep_zoom_admits_relative_stencil() {
    use crate::assemblies::workgroup::screen_worker::workshift::f64_stencil_admits;
    use crate::delta_gear::ComputeGear;
    run_big(|| {
        let compute_loc = (IntExp::from(-1).shift(-1), IntExp::ZERO);
        let res = (1280u32, 720u32);
        let zoom_pot = 50i64;
        assert!(
            f64_stencil_admits(&compute_loc, zoom_pot, res),
            "deep f64 stencil must admit via relative fallback"
        );
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: zoom_pot as i32,
            },
            res,
        );
        let ctx = from_stencil::<f64>(frame, None).expect("deep f64 shell");
        assert!(
            ctx.coords_are_relative,
            "past-absolute-collapse frames must use relative f64 samples"
        );
        // Deep relative admission floors view_gear at ScaledF64 (precision wall /
        // gear:F64 HUD snap fix) — never claim plain F64 as the deep view gear.
        assert_eq!(
            ctx.view_gear,
            ComputeGear::ScaledF64,
            "deep relative shell must advertise ScaledF64 view_gear floor, got {:?}",
            ctx.view_gear
        );
    });
}

#[test]
fn relative_abs_matches_absolute_generator_home() {
    run_big(|| {
        let frame = home_frame();
        let ctx = from_stencil::<FloatExp>(frame.clone(), None).expect("home");
        // Home admits absolute FloatExp — relative is only the deep fallback.
        assert!(
            !ctx.coords_are_relative,
            "home FloatExp shell must prefer absolute coords"
        );
        let deep = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-1).shift(-1), IntExp::ZERO),
                zoom_pot: 200,
            },
            (8u32, 8u32),
        );
        let deep_ctx = from_stencil::<FloatExp>(deep, None).expect("deep");
        assert!(
            deep_ctx.coords_are_relative,
            "past-f64 frames must fall back to relative FloatExp samples"
        );
        // Relative samples reconstruct via anchor at the view center.
        let center = (deep_ctx.res.0 / 2, deep_ctx.res.1 / 2);
        let rel = deep_ctx.c_generator.get_c(center);
        let abs = absolute_c(rel, &deep_ctx.coord_anchor);
        let anchor_fe = (
            FloatExp::from(deep_ctx.coord_anchor.0.clone()),
            FloatExp::from(deep_ctx.coord_anchor.1.clone()),
        );
        // Center relative should be ~0; absolute ≈ anchor.
        assert!(
            (abs.0 - anchor_fe.0).abs().to_f64() < 1e-6
                && (abs.1 - anchor_fe.1).abs().to_f64() < 1e-6,
            "center relative+anchor must recover the IntExp anchor"
        );
    });
}

/// Pivot remapping at unchanged home must not scramble partial completions into
/// vertical Dummy bands (live `home` = MoveTo + SetZoom can Replace twice).
#[test]
fn home_double_replace_collector_remap_preserves_completions() {
    run_big(|| {
        let frame = home_frame();
        let n = (frame.1.0 * frame.1.1) as usize;
        let mut results = vec![CompletedPoint::Dummy {}; n];
        // Simulate ~40% fill like an early live frame.
        for i in (0..n).step_by(5) {
            results[i] = CompletedPoint::Repeats {
                period: 1,
                smallness: FloatExp::ONE,
                small_time: 1,
            };
        }
        let pkg = ResultsPackage {
            results: results.clone(),
            screen_res: frame.1,
            location: frame.0.clone(),
        hud: Default::default()
    };
        let remapped = sample_old_values(&pkg, frame.0.clone(), frame.1);
        let before_filled = results
            .iter()
            .filter(|p| !matches!(p, CompletedPoint::Dummy {}))
            .count();
        let after_filled = remapped
            .results
            .iter()
            .filter(|p| !matches!(p, CompletedPoint::Dummy {}))
            .count();
        assert_eq!(
            before_filled, after_filled,
            "identity home remap must preserve filled seat count ({before_filled} vs {after_filled})"
        );
        for i in 0..n {
            let was_filled = !matches!(results[i], CompletedPoint::Dummy {});
            let still_filled = !matches!(remapped.results[i], CompletedPoint::Dummy {});
            if was_filled {
                assert!(
                    still_filled,
                    "identity remap dropped completion at seat index {i}"
                );
            }
        }
    });
}

/// Reference arrival after zero-orbit completions must not leave delivered=true
/// seats stuck while the collector still holds stale Dummy slots.
#[test]
fn home_reference_arrival_reopens_stale_deliveries() {
    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut ctx = from_stencil(frame, None).expect("home");
        // Finish a slice on the zero-orbit floor before reference publishes.
        for _ in 0..2000 {
            if ctx.percent_completed >= 35.0 {
                break;
            }
            perturb_workshift(16_000_000, 2, 4, 150, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(ctx.percent_completed > 10.0, "need partial zero-orbit fill");
        let delivered_before = ctx.points.iter().filter(|p| p.delivered).count();
        let pub_ref = Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        });
        ctx.latest_reference = Some(pub_ref);
        invalidate_stale_deliveries(&mut ctx, 1);
        let delivered_after = ctx.points.iter().filter(|p| p.delivered).count();
        assert!(
            delivered_after < delivered_before,
            "reference gen-1 must reopen stale zero-orbit deliveries"
        );
        for _ in 0..8000 {
            if ctx.points.iter().all(|p| p.delivered) {
                break;
            }
            perturb_workshift(16_000_000, 2, 4, 150, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "must finish after reference arrival"
        );
    });
}

/// Worker-layer sanity: delivered home frame must not show vertical column banding
/// in escape vs interior classification (B-SCH-3 / rectangular black bands).
#[test]
fn home_worker_no_vertical_repeat_columns() {
    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut ctx = from_stencil(frame, None).expect("home");
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        }));
        for _ in 0..10000 {
            if ctx.points.iter().all(|p| p.delivered) {
                break;
            }
            ctx.attention_index = 0;
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            work_update(&mut ctx);
        }
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "home shell must finish for column audit"
        );
        let w = ctx.res.0 as usize;
        let h = ctx.res.1 as usize;
        let mut repeat_heavy_cols = 0usize;
        for seat in 0..w {
            let mut repeats = 0usize;
            let mut escapes = 0usize;
            for row in 0..h {
                let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                let p = &ctx.points[idx];
                if p.repeats {
                    repeats += 1;
                }
                if p.escapes {
                    escapes += 1;
                }
            }
            let total = repeats + escapes;
            if total > 0 && repeats * 100 / total >= 80 {
                repeat_heavy_cols += 1;
            }
        }
        assert!(
            repeat_heavy_cols < 8,
            "worker classified {repeat_heavy_cols} columns as ≥80% interior — vertical band source"
        );
    });
}

/// Before reference publishes, zero-orbit floor must still finish home cleanly.
#[test]
fn home_zero_orbit_floor_pipeline_no_vertical_black_columns() {
    use crate::assemblies::shadergroup::colorer::color::color;
    use crate::assemblies::shadergroup::escaper::{get_value_from_point, ZoomerValuesScreen};
    use crate::assemblies::structs::{Answer, MandelbrotResult, PointStencil, View};
    use crate::assemblies::headgroup::window::sampling::{sample, SamplingContext};
    use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};

    run_big(|| {
        let frame = home_frame();
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        // No published reference — production zero-orbit floor only.
        for _ in 0..5000 {
            if ctx.percent_completed >= 100.0 {
                break;
            }
            perturb_workshift(16_000_000, 2, 4, 150, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(
            ctx.percent_completed >= 100.0,
            "zero-orbit floor must finish home, got {:.1}%",
            ctx.percent_completed
        );
        let w = ctx.res.0 as usize;
        let h = ctx.res.1 as usize;
        let location = frame.0.clone();
        let stencil = PointStencil {
            location: (
                location.pos.0.clone(),
                IntExp::ZERO - location.pos.1.clone(),
                location.zoom_pot,
            ),
            resolution: (w, h),
            serial_number: 0,
        };
        let settings = Settings {
            coloring_script: Some(DEFAULT_COLORING_SCRIPT.to_vec()),
            ..Settings::DEFAULT
        };
        let radius = settings.bailout_radius.clone().determine() as f32;
        let location_f64: (f64, f64) = (
            stencil.location.0.clone().into(),
            stencil.location.1.clone().into(),
        );
        let space_f64: f64 = IntExp::from(1)
            .shift(-stencil.location.2 - crate::constants::PIXELS_PER_UNIT_POT)
            .into();
        let mut results = Vec::new();
        for p in &ctx.points {
            results.push(if p.repeats {
                CompletedPoint::Repeats {
                    period: p.period,
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else {
                CompletedPoint::Escapes {
                    escape_time: p.iterations,
                    escape_location: (p.z.0, p.z.1),
                    escape_derivative: p.dc,
                    start_location: (p.c.0, p.c.1),
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            });
        }
        let answers: Vec<Answer> = results
            .iter()
            .map(|x| match x {
                CompletedPoint::Escapes {
                    escape_time,
                    escape_location,
                    escape_derivative,
                    smallness,
                    small_time,
                    ..
                } => {
                    let ez0: f64 = escape_location.0.into();
                    let ez1: f64 = escape_location.1.into();
                    let ed0: f64 = escape_derivative.0.into();
                    let ed1: f64 = escape_derivative.1.into();
                    Answer {
                        result: MandelbrotResult::Outside {
                            escape_time_r2: *escape_time as u64,
                            escape_z: (ez0 as f32, ez1 as f32),
                            escape_dc: (ed0 as f32, ed1 as f32),
                        },
                        min_magnitude_time: *small_time as u64,
                        min_magnitude: (*smallness).into(),
                    }
                }
                CompletedPoint::Repeats {
                    period,
                    smallness,
                    small_time,
                } => Answer {
                    result: MandelbrotResult::Inside {
                        period: *period as u64,
                    },
                    min_magnitude_time: *small_time as u64,
                    min_magnitude: (*smallness).into(),
                },
                CompletedPoint::Dummy {} => Answer {
                    result: MandelbrotResult::Inside { period: 0 },
                    min_magnitude_time: 0,
                    min_magnitude: 0.0,
                },
            })
            .collect();
        let escaper_results: Vec<CompletedPoint<f64>> = answers
            .into_iter()
            .enumerate()
            .map(|(i, x)| match x.result {
                MandelbrotResult::Inside { period } => CompletedPoint::Repeats {
                    period: period as u32,
                    smallness: x.min_magnitude,
                    small_time: x.min_magnitude_time as u32,
                },
                MandelbrotResult::Outside {
                    escape_time_r2,
                    escape_z,
                    escape_dc,
                } => CompletedPoint::Escapes {
                    escape_time: escape_time_r2 as u32,
                    escape_location: (escape_z.0.into(), escape_z.1.into()),
                    escape_derivative: (escape_dc.0.into(), escape_dc.1.into()),
                    smallness: x.min_magnitude,
                    small_time: x.min_magnitude_time as u32,
                    start_location: (
                        (location_f64.0 + stencil.seat_and_row(i).0 as f64 * space_f64).into(),
                        (location_f64.1 - stencil.seat_and_row(i).1 as f64 * space_f64).into(),
                    ),
                },
            })
            .collect();
        let mut screen_values = Vec::new();
        for i in 0..escaper_results.len() {
            let pos = pos_from_index(i, ctx.res.0);
            screen_values.push(get_value_from_point(
                &escaper_results[i],
                radius,
                pos,
                &escaper_results,
                ctx.res,
                settings.clone(),
            ));
        }
        let zoomer = ZoomerValuesScreen {
            values: screen_values,
            res: ctx.res,
            objective_location: location.clone(),
        hud: Default::default()
    };
        let pixels = color(&zoomer, &mut settings.clone());
        let color_view = View {
            stencil: stencil.clone(),
            data: pixels,
            bitmap: vec![0u8; w * h],
            hud: Default::default()
        };
        let mut sampling = SamplingContext {
            screen: Some(color_view),
            screen_size: ctx.res,
            location: location.clone(),
            updated: false,
            mouse_drag_start: None,
        };
        let mut viewport = Vec::new();
        sample(vec![], &mut viewport, &mut sampling);
        let black_cols = (0..w)
            .filter(|&seat| {
                (0..h)
                    .filter(|&row| {
                        let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                        let c = viewport[idx];
                        c.r() < 30 && c.g() < 30 && c.b() < 30
                    })
                    .count()
                    * 100
                    / h
                    >= 80
            })
            .count();
        assert!(
            black_cols < 8,
            "zero-orbit floor pipeline left {black_cols} ≥80% black columns"
        );
    });
}

/// Production token budget must still finish home without vertical black bands.
#[test]
fn home_production_budget_pipeline_no_vertical_black_columns() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        }));
        const TOKEN_BUDGET: u32 = 16_000_000;
        const ITER_COST: u32 = 2;
        const BOUT_COST: u32 = 4;
        const POINT_COST: u32 = 150;
        for _ in 0..5000 {
            if ctx.percent_completed >= 100.0 {
                break;
            }
            perturb_workshift(TOKEN_BUDGET, ITER_COST, BOUT_COST, POINT_COST, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(
            ctx.percent_completed >= 100.0,
            "production budget must finish home, got {:.1}%",
            ctx.percent_completed
        );
        // Reuse pipeline audit from sibling test (inline minimal check).
        let w = ctx.res.0 as usize;
        let h = ctx.res.1 as usize;
        let undelivered = ctx.points.iter().filter(|p| !p.delivered).count();
        assert_eq!(undelivered, 0, "all seats must deliver under production budget");
        let interior_cols = (0..w)
            .filter(|&seat| {
                let repeats = (0..h)
                    .filter(|&row| {
                        let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                        ctx.points[idx].repeats
                    })
                    .count();
                repeats * 100 / h >= 80
            })
            .count();
        assert!(
            interior_cols < 8,
            "production budget left {interior_cols} repeat-heavy columns"
        );
    });
}

/// Incremental WorkUpdate batches must populate the collector grid (no stuck Dummy).
#[test]
fn home_incremental_collector_matches_worker_delivery() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let orbit = ReferenceOrbit::compute(&req.c, req.precision_bits, 4096);
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit,
            c: req.c,
            generation: 1,
        }));
        let mut collector_results =
            vec![CompletedPoint::Dummy {}; (ctx.res.0 * ctx.res.1) as usize];
        for _ in 0..5000 {
            if ctx.percent_completed >= 100.0 {
                break;
            }
            perturb_workshift(16_000_000, 2, 4, 150, &mut ctx);
            for (point, index) in work_update(&mut ctx) {
                collector_results[index] = point;
            }
        }
        assert!(ctx.percent_completed >= 100.0);
        let worker_delivered = ctx.points.iter().filter(|p| p.delivered).count();
        let collector_dummy = collector_results
            .iter()
            .filter(|p| matches!(p, CompletedPoint::Dummy {}))
            .count();
        assert_eq!(
            collector_dummy, 0,
            "collector must have no Dummy after fill; worker delivered {worker_delivered}"
        );
        let mut mism = 0usize;
        for (i, p) in ctx.points.iter().enumerate() {
            if !p.delivered {
                continue;
            }
            let c = &collector_results[i];
            let ok = if p.repeats {
                matches!(c, CompletedPoint::Repeats { .. })
            } else {
                matches!(c, CompletedPoint::Escapes { .. })
            };
            if !ok {
                mism += 1;
            }
        }
        assert_eq!(mism, 0, "collector results must match worker outcomes");
    });
}

/// Home pipeline with a covering reference must not regress into vertical bands.
#[test]
fn home_pipeline_with_live_series_no_vertical_black_columns() {
    use crate::assemblies::shadergroup::colorer::color::color;
    use crate::assemblies::shadergroup::escaper::{get_value_from_point, ZoomerValuesScreen};
    use crate::assemblies::structs::{Answer, MandelbrotResult, PointStencil, View};
    use crate::assemblies::headgroup::window::sampling::{sample, SamplingContext};
    use crate::assemblies::workgroup::reference_worker::PublishedReference;
    use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};

    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let orbit = ReferenceOrbit::compute(&req.c, req.precision_bits, 4096);
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit,
            c: req.c,
            generation: 1,
        }));
        for _ in 0..5000 {
            if ctx.percent_completed >= 100.0 {
                break;
            }
            perturb_workshift(16_000_000, 2, 4, 150, &mut ctx);
            let _ = work_update(&mut ctx);
        }
        assert!(ctx.percent_completed >= 100.0);
        // Minimal black-column audit after full pipeline (same as sibling test).
        let w = ctx.res.0 as usize;
        let h = ctx.res.1 as usize;
        let location = frame.0.clone();
        let stencil = PointStencil {
            location: (
                location.pos.0.clone(),
                IntExp::ZERO - location.pos.1.clone(),
                location.zoom_pot,
            ),
            resolution: (w, h),
            serial_number: 0,
        };
        let settings = Settings {
            coloring_script: Some(DEFAULT_COLORING_SCRIPT.to_vec()),
            ..Settings::DEFAULT
        };
        let radius = settings.bailout_radius.clone().determine() as f32;
        let location_f64: (f64, f64) = (
            stencil.location.0.clone().into(),
            stencil.location.1.clone().into(),
        );
        let space_f64: f64 = IntExp::from(1)
            .shift(-stencil.location.2 - crate::constants::PIXELS_PER_UNIT_POT)
            .into();
        let answers: Vec<Answer> = ctx
            .points
            .iter()
            .map(|p| {
                if p.repeats {
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: p.period as u64,
                        },
                        min_magnitude_time: p.small_time as u64,
                        min_magnitude: p.smallness_squared.into(),
                    }
                } else {
                    let ez0: f64 = p.z.0.into();
                    let ez1: f64 = p.z.1.into();
                    let ed0: f64 = p.dc.0.into();
                    let ed1: f64 = p.dc.1.into();
                    Answer {
                        result: MandelbrotResult::Outside {
                            escape_time_r2: p.iterations as u64,
                            escape_z: (ez0 as f32, ez1 as f32),
                            escape_dc: (ed0 as f32, ed1 as f32),
                        },
                        min_magnitude_time: p.small_time as u64,
                        min_magnitude: p.smallness_squared.into(),
                    }
                }
            })
            .collect();
        let escaper_results: Vec<CompletedPoint<f64>> = answers
            .into_iter()
            .enumerate()
            .map(|(i, x)| match x.result {
                MandelbrotResult::Inside { period } => CompletedPoint::Repeats {
                    period: period as u32,
                    smallness: x.min_magnitude,
                    small_time: x.min_magnitude_time as u32,
                },
                MandelbrotResult::Outside {
                    escape_time_r2,
                    escape_z,
                    escape_dc,
                } => CompletedPoint::Escapes {
                    escape_time: escape_time_r2 as u32,
                    escape_location: (escape_z.0.into(), escape_z.1.into()),
                    escape_derivative: (escape_dc.0.into(), escape_dc.1.into()),
                    smallness: x.min_magnitude,
                    small_time: x.min_magnitude_time as u32,
                    start_location: (
                        (location_f64.0 + stencil.seat_and_row(i).0 as f64 * space_f64).into(),
                        (location_f64.1 - stencil.seat_and_row(i).1 as f64 * space_f64).into(),
                    ),
                },
            })
            .collect();
        let mut screen_values = Vec::new();
        for i in 0..escaper_results.len() {
            screen_values.push(get_value_from_point(
                &escaper_results[i],
                radius,
                pos_from_index(i, ctx.res.0),
                &escaper_results,
                ctx.res,
                settings.clone(),
            ));
        }
        let zoomer = ZoomerValuesScreen {
            values: screen_values,
            res: ctx.res,
            objective_location: location.clone(),
        hud: Default::default()
    };
        let pixels = color(&zoomer, &mut settings.clone());
        let color_view = View {
            stencil: stencil.clone(),
            data: pixels,
            bitmap: vec![0u8; w * h],
            hud: Default::default()
        };
        let mut sampling = SamplingContext {
            screen: Some(color_view),
            screen_size: ctx.res,
            location,
            updated: false,
            mouse_drag_start: None,
        };
        let mut viewport = Vec::new();
        sample(vec![], &mut viewport, &mut sampling);
        let black_cols = (0..w)
            .filter(|&seat| {
                (0..h)
                    .filter(|&row| {
                        let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                        let c = viewport[idx];
                        c.r() < 30 && c.g() < 30 && c.b() < 30
                    })
                    .count()
                    * 100
                    / h
                    >= 80
            })
            .count();
        assert!(
            black_cols < 8,
            "live-series pipeline left {black_cols} ≥80% black columns"
        );
    });
}

/// Full CPU pipeline (collector → escaper → colorer) must not paint vertical black bands.
#[test]
fn home_pipeline_no_vertical_black_columns() {
    use crate::assemblies::shadergroup::colorer::color::color;
    use crate::assemblies::shadergroup::escaper::{
        get_value_from_point, ZoomerValuesScreen,
    };
    use crate::assemblies::structs::{Answer, MandelbrotResult, PointStencil, View};
    use crate::settings::Settings;
    use crate::settings::DEFAULT_COLORING_SCRIPT;

    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut ctx = from_stencil(frame.clone(), None).expect("home");
        ctx.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        }));
        for _ in 0..10000 {
            if ctx.points.iter().all(|p| p.delivered) {
                break;
            }
            ctx.attention_index = 0;
            perturb_workshift(0, 0, 0, 0, &mut ctx);
            work_update(&mut ctx);
        }
        assert!(ctx.points.iter().all(|p| p.delivered));

        // Mirror work_collector (including f32 narrowing) + escaper reconstruction.
        let mut results = Vec::with_capacity(ctx.points.len());
        for i in 0..ctx.points.len() {
            let p = &ctx.points[i];
            let _ = i; // collector maps via completed_points order; full grid here
            let cp = if p.repeats {
                CompletedPoint::Repeats {
                    period: p.period,
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else if p.escapes {
                CompletedPoint::Escapes {
                    escape_time: p.iterations,
                    escape_location: (p.z.0, p.z.1),
                    escape_derivative: p.dc,
                    start_location: (p.c.0, p.c.1),
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else {
                CompletedPoint::Dummy {}
            };
            results.push(cp);
        }
        let location = frame.0.clone();
        let stencil = PointStencil {
            location: (
                location.pos.0.clone(),
                IntExp::ZERO - location.pos.1.clone(),
                location.zoom_pot,
            ),
            resolution: (ctx.res.0 as usize, ctx.res.1 as usize),
            serial_number: 0,
        };
        let location_f64: (f64, f64) = (
            stencil.location.0.clone().into(),
            stencil.location.1.clone().into(),
        );
        let space_f64: f64 = IntExp::from(1)
            .shift(-stencil.location.2 - crate::constants::PIXELS_PER_UNIT_POT)
            .into();
        // Round-trip through Answer like work_collector → escaper (f32 narrowing).
        let answers: Vec<Answer> = results
            .iter()
            .map(|x| match x {
                CompletedPoint::Escapes {
                    escape_time,
                    escape_location,
                    escape_derivative,
                    smallness,
                    small_time,
                    ..
                } => {
                    let ez0: f64 = escape_location.0.into();
                    let ez1: f64 = escape_location.1.into();
                    let ed0: f64 = escape_derivative.0.into();
                    let ed1: f64 = escape_derivative.1.into();
                    let mag: f64 = (*smallness).into();
                    Answer {
                        result: MandelbrotResult::Outside {
                            escape_time_r2: *escape_time as u64,
                            escape_z: (ez0 as f32, ez1 as f32),
                            escape_dc: (ed0 as f32, ed1 as f32),
                        },
                        min_magnitude_time: *small_time as u64,
                        min_magnitude: mag,
                    }
                }
                CompletedPoint::Repeats {
                    period,
                    smallness,
                    small_time,
                } => {
                    let mag: f64 = (*smallness).into();
                    Answer {
                        result: MandelbrotResult::Inside {
                            period: *period as u64,
                        },
                        min_magnitude_time: *small_time as u64,
                        min_magnitude: mag,
                    }
                }
                CompletedPoint::Dummy {} => Answer {
                    result: MandelbrotResult::Inside { period: 0 },
                    min_magnitude_time: 0,
                    min_magnitude: 0.0,
                },
            })
            .collect();
        let escaper_results: Vec<CompletedPoint<f64>> = answers
            .into_iter()
            .enumerate()
            .map(|(i, x)| match x.result {
                MandelbrotResult::Inside { period } => CompletedPoint::Repeats {
                    period: period as u32,
                    smallness: x.min_magnitude.into(),
                    small_time: x.min_magnitude_time as u32,
                },
                MandelbrotResult::Outside {
                    escape_time_r2,
                    escape_z,
                    escape_dc,
                } => CompletedPoint::Escapes {
                    escape_time: escape_time_r2 as u32,
                    escape_location: (escape_z.0.into(), escape_z.1.into()),
                    escape_derivative: (escape_dc.0.into(), escape_dc.1.into()),
                    smallness: x.min_magnitude.into(),
                    small_time: x.min_magnitude_time as u32,
                    start_location: (
                        (location_f64.0 + stencil.seat_and_row(i).0 as f64 * space_f64).into(),
                        (location_f64.1 - stencil.seat_and_row(i).1 as f64 * space_f64).into(),
                    ),
                },
            })
            .collect();
        let settings = Settings {
            coloring_script: Some(DEFAULT_COLORING_SCRIPT.to_vec()),
            ..Settings::DEFAULT
        };
        let radius = settings.bailout_radius.clone().determine() as f32;
        let mut screen_values = Vec::with_capacity(escaper_results.len());
        for i in 0..escaper_results.len() {
            let pos = pos_from_index(i, ctx.res.0);
            screen_values.push(get_value_from_point(
                &escaper_results[i],
                radius,
                pos,
                &escaper_results,
                ctx.res,
                settings.clone(),
            ));
        }
        let outside_n = screen_values
            .iter()
            .filter(|v| matches!(v, crate::assemblies::shadergroup::escaper::ScreenValue::Outside { .. }))
            .count();
        let inside_n = screen_values.len() - outside_n;
        let w = ctx.res.0 as usize;
        let h = ctx.res.1 as usize;
        let zoomer = ZoomerValuesScreen {
            values: screen_values,
            res: ctx.res,
            objective_location: location.clone(),
        hud: Default::default()
    };
        let mut filament_n = 0usize;
        for y in 0..h {
            for x in 0..w {
                if crate::assemblies::shadergroup::colorer::color::is_in_filament(
                    &zoomer,
                    (x as i32, y as i32),
                ) {
                    filament_n += 1;
                }
            }
        }
        let mut paint_settings = settings.clone();
        let pixels = color(&zoomer, &mut paint_settings);
        let non_black = pixels
            .iter()
            .filter(|c| c.r() >= 30 || c.g() >= 30 || c.b() >= 30)
            .count();
        let mut black_heavy_cols = 0usize;
        for seat in 0..w {
            let mut black = 0usize;
            for row in 0..h {
                let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                let c = pixels[idx];
                if c.r() < 30 && c.g() < 30 && c.b() < 30 {
                    black += 1;
                }
            }
            if black * 100 / h >= 80 {
                black_heavy_cols += 1;
            }
        }
        assert!(
            black_heavy_cols < 8,
            "pipeline painted {black_heavy_cols} ≥80% black columns; outside={outside_n} inside={inside_n} filaments={filament_n} non_black={non_black}"
        );

        // Window samples Color32 through relative remap — bands must not appear here either.
        use crate::assemblies::headgroup::window::sampling::{sample, SamplingContext};
        let color_view = View {
            stencil: stencil.clone(),
            data: pixels,
            bitmap: vec![0u8; w * h],
            hud: Default::default()
        };
        let mut sampling = SamplingContext {
            screen: Some(color_view),
            screen_size: ctx.res,
            location: location.clone(),
            updated: false,
            mouse_drag_start: None,
        };
        let mut viewport = Vec::new();
        sample(vec![], &mut viewport, &mut sampling);
        let mut sampled_black_cols = 0usize;
        for seat in 0..w {
            let mut black = 0usize;
            for row in 0..h {
                let idx = index_from_pos(&(seat as i32, row as i32), ctx.res.0);
                let c = viewport[idx];
                if c.r() < 30 && c.g() < 30 && c.b() < 30 {
                    black += 1;
                }
            }
            if black * 100 / h >= 80 {
                sampled_black_cols += 1;
            }
        }
        assert!(
            sampled_black_cols < 8,
            "window sample() introduced {sampled_black_cols} ≥80% black columns"
        );
    });
}

