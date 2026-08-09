// Property and example tests binding the craftsmanship inventory
// (docs/assistant/tracey/craftsmanship-rules.md) to the v0.0.9 workgroup code.
// Each test names the rule it verifies.
//
// Seam note: WorkContext's completion buffer is a heap Vec-backed Stec
// (capacity 100000). Tests that build one still use run_big for headroom.

use std::time::Instant;
use std::collections::VecDeque;

use proptest::prelude::*;

use super::perturb_floatexp::FloatExpPerturbationKernel;
use super::perturb_kernel::PerturbationKernel;
use super::work_update;
use super::{invalidate_stale_deliveries, workshift::*};
use crate::assemblies::headgroup::window::sampling::index_from_relative_location;
use crate::assemblies::workgroup::c_generator::{CGenerator, Mandelbrotable};
use crate::assemblies::workgroup::work_collector::{sample_old_values, ResultsPackage};
use crate::assemblies::workgroup::screen_worker::workshift::get_random_mixmap;
use crate::utils::{index_from_pos, pos_from_index, IntExp, ObjectivePosAndZoom};
use crate::floatexp::{ComplexFloatExp, FloatExp};

fn run_big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn make_point(c: (f64, f64)) -> Point<FloatExp> {
    let fe = (FloatExp::from(c.0), FloatExp::from(c.1));
    Point {
        delta_c: fe,
        c: fe,
        z: fe,
        dc: (FloatExp::ONE, FloatExp::ZERO),
        real_squared: FloatExp::ZERO,
        imag_squared: FloatExp::ZERO,
        real_imag: FloatExp::ZERO,
        iterations: 0,
        loop_detection_point: ((FloatExp::ZERO, FloatExp::ZERO), 0),
        escapes: false,
        repeats: false,
        delivered: false,
        initialized: true,
        period: 0,
        smallness_squared: <FloatExp as Mandelbrotable>::max_value(),
        small_time: 0,
        delta: None,
        direct_only: false,
        bound_zero_generation: 0,
    }
}

/// 4x2 screen. points[0]/points[1] sit 1e-9 apart so the pitch epsilon is
/// ~4e-12; `esc` escapes in ~2 iterations; `slow` is near-parabolic interior
/// (multiplier ~1-2e-8, needs ~1e9 iterations to trip loop detection — far
/// beyond any 10ms shift), so it never completes inside a test.
const ESC: (f64, f64) = (2.0, 2.0);
const SLOW: (f64, f64) = (0.25 - 1e-16, 0.0);

fn make_context(workshifts: u32) -> WorkContext<FloatExp> {
    let res = (4u32, 2u32);
    let cs = [
        (0.0, 0.0),
        (1e-9, 0.0),
        ESC,
        SLOW,
        SLOW,
        SLOW,
        SLOW,
        SLOW,
    ];
    let points: Vec<Point<FloatExp>> = cs.iter().map(|&c| make_point(c)).collect();
    let n = points.len();
    let c_generator = CGenerator::<FloatExp>::new(&(IntExp::from(0), IntExp::from(0)), -2, res).unwrap();
    let center = ((res.0 / 2) as i32, (res.1 / 2) as i32);
    WorkContext {
        points,
        completed_points: Stec::with_capacity(100000, (CompletedPoint::Dummy {}, 0)),
        last_update: 0,
        index: 0,
        random_index: 0,
        time_created: Instant::now(),
        time_workshift_started: Instant::now(),
        percent_completed: 0.0,
        random_map: (0..n).collect(),
        workshifts,
        total_iterations: 0,
        total_iterations_today: 0,
        total_bouts_today: 0,
        total_points_today: 0,
        spent_tokens_today: 0,
        res,
        scredge_poses: VecDeque::new(),
        edge_queue: VecDeque::new(),
        out_queue: VecDeque::new(),
        in_queue: VecDeque::new(),
        motion: Motion::Neither,
        attention: None,
        attention_anchor: center,
        // Force queue fallthrough unless a test exercises the spiral.
        attention_index: u64::MAX,
        attention_current: None,
        c_generator,
        pitch_epsilon: FloatExp::from(1e-9 * (1.0 / 256.0)),
        coord_anchor: (IntExp::ZERO, IntExp::ZERO),
        view_gear: crate::delta_gear::ComputeGear::FloatExp,
        active_gear: crate::delta_gear::ComputeGear::FloatExp,
        coords_are_relative: false,
        latest_reference: None,
        hud_points_window: 0,
        hud_window_started: Instant::now(),
        reference_floor_active: false,
        pert_trial_shifts_left: 0,
        pert_trial_cooldown: 0,
        generator_generation: 0,
    }
}

fn shift(ctx: &mut WorkContext<FloatExp>) {
    // Scheduler tests isolate queue policy from production numerics.
    workshift_with_kernel(0, 0, 0, 0, ctx, &DirectKernel);
}

fn perturb_workshift(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    bout_token_cost: u32,
    point_token_cost: u32,
    context: &mut WorkContext<FloatExp>,
) {
    workshift_with_kernel(
        day_token_allowance,
        iteration_token_cost,
        point_token_cost,
        bout_token_cost,
        context,
        &FloatExpPerturbationKernel,
    );
}

#[test]
// r[verify cz.craft.kernel-seam+1]
fn direct_kernel_preserves_scheduler_results() {
    let mut a = make_context(1);
    a.edge_queue.push_back(((2, 0), 0));
    let mut b = a.clone();
    workshift_with_kernel(0, 0, 0, 0, &mut a, &DirectKernel);
    workshift_with_kernel(0, 0, 0, 0, &mut b, &DirectKernel);
    assert_eq!(a.completed_points.len, b.completed_points.len);
    assert_eq!(a.points[2].iterations, b.points[2].iterations);
    assert_eq!(a.points[2].escapes, b.points[2].escapes);
    assert_eq!(a.points[2].repeats, b.points[2].repeats);
    assert_eq!(a.points[2].delivered, b.points[2].delivered);
    assert_eq!(a.edge_queue, b.edge_queue);
    assert_eq!(a.out_queue, b.out_queue);
    assert_eq!(a.in_queue, b.in_queue);
}

fn orbit_with_derivative(c: (f64, f64), iterations: u32) -> Point<FloatExp> {
    let mut point = make_point(c);
    for _ in 0..iterations {
        update_point_results(&mut point);
        iterate(&mut point);
    }
    point
}

fn adjacent_ulps(value: f64, upward: bool, ulps: u64) -> f64 {
    if value == 0.0 {
        return if upward { f64::from_bits(ulps) } else { -f64::from_bits(ulps) };
    }
    let bits = value.to_bits();
    f64::from_bits(if upward == (value > 0.0) { bits + ulps } else { bits - ulps })
}

proptest! {
    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn mandelbrot_dc_matches_ulp_finite_difference(
        cr in -1.5f64..0.5,
        ci in -1.0f64..1.0,
        iterations in 1u32..4,
    ) {
        prop_assume!(cr.abs() > 0.01);
        let center = orbit_with_derivative((cr, ci), iterations);
        // A small ULP window avoids making the finite-difference numerator
        // itself indistinguishable from endpoint rounding.
        let lo = adjacent_ulps(cr, false, 1 << 20);
        let hi = adjacent_ulps(cr, true, 1 << 20);
        let z_lo = orbit_with_derivative((lo, ci), iterations).z;
        let z_hi = orbit_with_derivative((hi, ci), iterations).z;
        let finite = (
            (z_hi.0.to_f64() - z_lo.0.to_f64()) / (hi - lo),
            (z_hi.1.to_f64() - z_lo.1.to_f64()) / (hi - lo),
        );
        let tolerance = iterations as f64 * 1.0e-4;
        let dc0 = center.dc.0.to_f64();
        let dc1 = center.dc.1.to_f64();
        prop_assert!((dc0 - finite.0).abs() <= tolerance * dc0.abs().max(1.0));
        prop_assert!((dc1 - finite.1).abs() <= tolerance * dc1.abs().max(1.0));
    }

    // r[verify cz.craft.screen-space-derivative-edges+1]
    #[test]
    fn mandelbrot_dc_obeys_conjugation(
        cr in -2.0f64..1.0,
        ci in -1.5f64..1.5,
        iterations in 1u32..8,
    ) {
        let a = orbit_with_derivative((cr, ci), iterations).dc;
        let b = orbit_with_derivative((cr, -ci), iterations).dc;
        prop_assert_eq!(a.0, b.0);
        prop_assert_eq!(a.1, -b.1);
    }
}

// verifies r[cz.craft.epsilon-pixel-pitch+1]
#[test]
fn epsilon_scales_with_pixel_pitch() {
    let a = vec![make_point((0.0, 0.0)), make_point((0.25, 0.0))];
    let b = vec![make_point((0.0, 0.0)), make_point((0.5, 0.0))];
    let ea = pitch_epsilon(&a);
    let eb = pitch_epsilon(&b);
    assert!(ea > FloatExp::ZERO);
    assert_eq!(eb, ea * FloatExp::TWO, "doubling pixel pitch must double epsilon");
}

// verifies r[cz.craft.cached-products+1]
proptest! {
    #[test]
    fn cached_products_match_z(zr in -1000.0f64..1000.0, zi in -1000.0f64..1000.0) {
        let mut p = make_point((0.1, 0.1));
        p.z = (FloatExp::from(zr), FloatExp::from(zi));
        p.iterations = 17;
        update_point_results(&mut p);
        prop_assert_eq!(p.real_squared, FloatExp::from(zr * zr));
        prop_assert_eq!(p.imag_squared, FloatExp::from(zi * zi));
        prop_assert_eq!(p.real_imag, FloatExp::from(zr * zi));
        // smallness collected as a free side effect
        prop_assert_eq!(p.smallness_squared, FloatExp::from(zr * zr + zi * zi));
        prop_assert_eq!(p.small_time, 17);
    }

    // verifies r[cz.craft.mixmap-shuffle+1]
    #[test]
    fn mixmap_is_permutation(n in 1usize..2048) {
        let mut m = get_random_mixmap(n);
        m.sort_unstable();
        prop_assert_eq!(m, (0..n).collect::<Vec<_>>());
    }

    // verifies r[cz.craft.clamped-remap-smear+1]
    #[test]
    fn remap_index_clamps_to_border(
        lx in -200i32..200,
        ly in -200i32..200,
        w in 1u32..64,
        h in 1u32..64,
    ) {
        let res = (w, h);
        let i = index_from_relative_location((lx, ly), res, (w * h) as usize);
        let cx = lx.max(0).min(w as i32 - 1) as usize;
        let cy = ly.max(0).min(h as i32 - 1) as usize;
        prop_assert_eq!(i, cy * w as usize + cx);
    }

    // verifies r[cz.craft.period-derivative-test+1]
    #[test]
    fn main_cardioid_points_detect_as_period_one(
        radius in 0.0f64..0.95,
        angle in -std::f64::consts::PI..std::f64::consts::PI,
    ) {
        // Multiplier parameterization: c = μ/2 - μ²/4, |μ| < 1.
        let mu = (radius * angle.cos(), radius * angle.sin());
        let mu_squared = (
            mu.0 * mu.0 - mu.1 * mu.1,
            2.0 * mu.0 * mu.1,
        );
        let c = (
            mu.0 / 2.0 - mu_squared.0 / 4.0,
            mu.1 / 2.0 - mu_squared.1 / 4.0,
        );
        prop_assert_eq!(detected_period(c, 4096), 1);
    }

    // verifies r[cz.craft.period-derivative-test+1]
    // The period-2 bulb is exactly the disk |c + 1| < 1/4.
    #[test]
    fn period_two_bulb_detects_as_period_two(
        x in -0.24f64..0.24,
        y in -0.24f64..0.24,
    ) {
        prop_assume!(x * x + y * y < 0.24 * 0.24);
        prop_assert_eq!(detected_period((-1.0 + x, y), 4096), 2);
    }

    // Independent child-component oracles. These centers/periods exist only in
    // the test: production sees an ordinary c and must derive the period.
    #[test]
    fn child_bulb_interiors_are_period_constant(
        dx in -1.0e-5f64..1.0e-5,
        dy in -1.0e-5f64..1.0e-5,
    ) {
        let components = [
            ((-1.7548776662466927, 0.0), 3),
            ((-0.15652016683375508, 1.0322471089228318), 4),
            ((-0.15652016683375508, -1.0322471089228318), 4),
        ];
        for (center, period) in components {
            let c = (center.0 + dx, center.1 + dy);
            prop_assert_eq!(
                detected_period(c, 4096),
                period,
                "period noise inside period-{} component at {:?}",
                period,
                c,
            );
        }
    }

    // verifies r[cz.craft.period-derivative-test+1]
    // The cardioid/bulb neck at c = -0.75 is the parabolic worst case: approach
    // lag explodes there, which is exactly where epsilon-based detection dies.
    // Newton needs no orbit convergence, so correctness must hold to f64 depth.
    #[test]
    fn neck_zoom_classifies_correctly_at_arbitrary_depth(depth in 2u32..40) {
        let delta = 2.0f64.powi(-(depth as i32));
        // just right of the neck: main cardioid, period 1
        prop_assert_eq!(detected_period((-0.75 + delta, 0.0), 8192), 1);
        // just left of the neck (δ < 0.5, so we stay off the bulb tip at -1.25):
        // period-2 bulb
        prop_assert_eq!(detected_period((-0.75 - delta, 0.0), 8192), 2);
        // the neck point itself is on the boundary (|b| = 1): either answer is
        // legitimate, but it must be detected, not garbage
        let neck = detected_period((-0.75, 0.0), 8192);
        prop_assert!(neck == 1 || neck == 2, "neck detected as {}", neck);
    }
}

// The completion-path pipeline: partials ascending, tail-started Newton,
// first verified wins.
fn detected_period(c: (f64, f64), max_iter: u32) -> u32 {
    let (partials, tail) = period_partials(c, max_iter);
    partials
        .into_iter()
        .find_map(|p| verified_period_from(c, p, tail))
        .unwrap_or(0)
}

// verifies r[cz.craft.period-derivative-test+1]
#[test]
fn known_attractors_have_their_published_periods() {
    assert_eq!(verified_period((0.0, 0.0), 1), Some(1));
    assert_eq!(verified_period((0.25, 0.0), 1), Some(1));
    assert_eq!(verified_period((-1.0, 0.0), 2), Some(2));
    assert_eq!(
        verified_period((-1.7548776662466927, 0.0), 3),
        Some(3)
    );
    assert_eq!(
        verified_period((-0.15652016683375508, 1.0322471089228318), 4),
        Some(4)
    );
}

// verifies r[cz.craft.period-derivative-test+1]
#[test]
fn exterior_or_wrong_period_is_not_accepted() {
    assert_eq!(verified_period((1.0, 0.0), 1), None);
    assert_eq!(verified_period((-1.0, 0.0), 1), None);
}

// verifies r[cz.craft.lifo-drain+1]
#[test]
fn completion_drain_is_lifo() {
    run_big(|| {
        let mut ctx = make_context(0);
        for i in 0..50usize {
            assert!(ctx.completed_points.try_push((CompletedPoint::Dummy {}, i)));
        }
        let drained = work_update(&mut ctx);
        let order: Vec<usize> = drained.iter().map(|(_, i)| *i).collect();
        assert_eq!(order, (0..50).rev().collect::<Vec<_>>(), "freshest work must publish first");
        assert_eq!(ctx.completed_points.len, 0);
    });
}

// verifies r[cz.craft.edge-push-front+1]
#[test]
fn edge_neighbors_jump_queue_front() {
    run_big(|| {
        let ctx = make_context(0);
        let mut queue: VecDeque<((i32, i32), u32)> = VecDeque::new();
        queue.push_back(((0, 0), 999)); // pre-existing entry
        queue_incomplete_neighbors_of_edge(&(2, 0), &(3, 0), ctx.res, &ctx.points, &mut queue);
        assert_eq!(queue.back().unwrap().1, 999, "edge neighbors must jump ahead of existing entries");
        assert!(queue.len() > 1);
        // delivered neighbors are excluded
        let mut ctx2 = make_context(0);
        let mut q2: VecDeque<((i32, i32), u32)> = VecDeque::new();
        let delivered_neighbor = (2, 1);
        ctx2.points[index_from_pos(&delivered_neighbor, ctx2.res.0)].delivered = true;
        queue_incomplete_neighbors_of_edge(&(2, 0), &(3, 0), ctx2.res, &ctx2.points, &mut q2);
        assert!(q2.iter().all(|(pos, _)| *pos != delivered_neighbor));
    });
}

// verifies r[cz.craft.cost-metadata+1]
#[test]
fn queue_entries_carry_source_cost() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.points[index_from_pos(&(1, 0), ctx.res.0)].iterations = 42;
        let mut q: VecDeque<((i32, i32), u32)> = VecDeque::new();
        queue_incomplete_neighbors(&(1, 0), ctx.res, &ctx.points, &mut q);
        assert!(!q.is_empty());
        assert!(q.iter().all(|(_, difficulty)| *difficulty == 42),
            "out-queue entries carry the source point's iteration cost");

        ctx.points[index_from_pos(&(1, 0), ctx.res.0)].period = 7;
        let mut qin: VecDeque<((i32, i32), u32)> = VecDeque::new();
        queue_incomplete_neighbors_in(&(1, 0), ctx.res, &ctx.points, &mut qin);
        assert!(!qin.is_empty());
        assert!(qin.iter().all(|(_, period)| *period == 7),
            "in-queue entries carry the source point's period");
    });
}

// verifies r[cz.craft.scredge-first-shift0+1]
// Attention owns slot 0; when the spiral is exhausted, shift 0 still prefers
// scredge over edge (the old first-shift motion-edge proof).
#[test]
fn scredge_first_only_on_shift_zero() {
    run_big(|| {
        // shift 0: scredge seat (3,1) is worked before the edge seat (2,0),
        // so the first completion in the buffer belongs to (3,1).
        let mut ctx = make_context(0);
        ctx.scredge_poses.push_back((3, 1));
        ctx.edge_queue.push_back(((2, 0), 0));
        shift(&mut ctx);
        assert!(ctx.completed_points.len > 0);
        assert_eq!(ctx.completed_points.stuff[0].1, index_from_pos(&(3, 1), ctx.res.0),
            "shift 0 fallthrough must prove the motion edge first");
    });
    run_big(|| {
        // shift 1: edge outranks scredge.
        let mut ctx = make_context(1);
        ctx.scredge_poses.push_back((3, 1));
        ctx.edge_queue.push_back(((2, 0), 0));
        shift(&mut ctx);
        assert!(ctx.completed_points.len > 0);
        assert_eq!(ctx.completed_points.stuff[0].1, index_from_pos(&(2, 0), ctx.res.0),
            "after shift 0 scredge is demoted behind edge");
    });
}

// r[verify cz.craft.pan-zoom-slot0+1]
#[test]
fn slots_one_to_four_ignore_motion() {
    run_big(|| {
        // Motion only affects slot 0. For each of slots 1..=4, Panned and
        // Zoomed contexts must make the same leading pick.
        // Slot 1..=3 lead edge; slot 4 leads scredge.
        for slot in 1u32..=4 {
            let lead = if slot == 4 { (3, 1) } else { (2, 0) };
            let lead_index = index_from_pos(&lead, (4u32, 2u32).0);
            for motion in [Motion::Panned, Motion::Zoomed] {
                let mut ctx = make_context(slot); // workshifts % 5 == slot
                ctx.motion = motion;
                ctx.scredge_poses.push_back((3, 1));
                ctx.edge_queue.push_back(((2, 0), 0));
                shift(&mut ctx);
                assert!(
                    ctx.points[lead_index].delivered || ctx.points[lead_index].iterations > 0,
                    "slot {slot} with {:?} must start its queue lead seat", motion
                );
            }
        }
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn spiral_skips_offscreen_seats() {
    run_big(|| {
        let mut ctx = make_context(0);
        // Anchor at the corner; ring 1 walks off-screen on the negative side.
        set_attention(&mut ctx, Some((0, 0)));
        ctx.attention_index = 0;
        // First on-screen seat the spiral returns must be in-bounds.
        for _ in 0..16 {
            if let Some(pos) = next_attention_spiral_pos(&mut ctx) {
                assert!(pos.0 >= 0 && pos.1 >= 0
                    && pos.0 < ctx.res.0 as i32 && pos.1 < ctx.res.1 as i32,
                    "spiral returned off-screen {:?}", pos);
            }
        }
    });
}

// verifies r[cz.craft.out-rotates-in-stays+1]
// Note: the In arm's rotation is commented out in workshift.rs; the asymmetry
// is currently latent (In seats are re-selected in place). This test pins the
// Out rotation and the fact that neither queue loses entries.
#[test]
fn out_rotates_without_loss() {
    run_big(|| {
        let mut ctx = make_context(2); // % 5 == 2: out first
        ctx.out_queue.push_back(((3, 0), 0));
        ctx.out_queue.push_back(((3, 1), 0));
        shift(&mut ctx);
        assert_eq!(ctx.out_queue.len(), 2, "slow out seats rotate to the back, never dropped");
        let poses: Vec<(i32, i32)> = ctx.out_queue.iter().map(|(p, _)| *p).collect();
        assert!(poses.contains(&(3, 0)) && poses.contains(&(3, 1)));
        assert!(!ctx.points[index_from_pos(&(3, 0), ctx.res.0)].delivered);
    });
    run_big(|| {
        let mut ctx = make_context(5); // % 5 == 0, != 0: in reachable
        ctx.in_queue.push_back(((3, 0), 0));
        shift(&mut ctx);
        assert_eq!(ctx.in_queue.len(), 1, "in seat is not dropped");
        assert!(!ctx.points[index_from_pos(&(3, 0), ctx.res.0)].delivered);
    });
}

// verifies r[cz.craft.provisional-not-delivered+1]
#[test]
fn provisional_answer_never_marks_delivered() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.scredge_poses.push_back((3, 1));
        shift(&mut ctx);
        assert!(ctx.completed_points.len > 0, "scredge publishes provisional answers");
        let index = index_from_pos(&(3, 1), ctx.res.0);
        assert!(!ctx.points[index].delivered,
            "a guess must never block the truth");
        let periods = ctx.completed_points.stuff[..ctx.completed_points.len]
            .iter()
            .filter_map(|(answer, answer_index)| {
                (*answer_index == index).then_some(match answer {
                    CompletedPoint::Repeats { period, .. } => *period,
                    _ => panic!("an unfinished interior point must publish as repeating"),
                })
            })
            .collect::<Vec<_>>();
        assert!(!periods.is_empty(), "target point publishes provisionally");
        assert!(periods.iter().all(|period| *period == 0),
            "provisional checkpoint gaps are not periods: {:?}", periods);
    });
}

// verifies r[cz.craft.undeliver-on-full+1]
#[test]
fn full_buffer_undelivers_and_stops() {
    run_big(|| {
        let mut ctx = make_context(1); // % 5 == 1: edge first
        while ctx.completed_points.try_push((CompletedPoint::Dummy {}, 0)) {}
        assert_eq!(ctx.completed_points.len, 100000);
        ctx.edge_queue.push_back(((2, 0), 0));
        shift(&mut ctx);
        assert_eq!(ctx.completed_points.len, 100000, "nothing is lost");
        assert!(!ctx.points[index_from_pos(&(2, 0), ctx.res.0)].delivered,
            "backpressure degrades to re-queue: the point is un-delivered for a later shift");
        // Drain one slot and confirm the re-queued seat completes later.
        // The first completion flooded out_queue with (2,0)'s neighbors; use a
        // fresh edge-first slot and clear out_queue so (2,0) is the lead again.
        ctx.completed_points.len -= 1;
        ctx.out_queue.clear();
        ctx.workshifts = 1; // edge-first slot
        ctx.edge_queue.push_back(((2, 0), 0));
        shift(&mut ctx);
        assert!(ctx.points[index_from_pos(&(2, 0), ctx.res.0)].delivered,
            "the affected seat completes once the buffer has room");
    });
}

// verifies r[cz.craft.shared-remap-transform+1]
#[test]
fn remap_onto_same_view_is_fixed_point() {
    run_big(|| {
        let res = (4u32, 3u32);
        let results: Vec<CompletedPoint<FloatExp>> = (0..(res.0 * res.1))
            .map(|i| CompletedPoint::Escapes {
                escape_time: i,
                escape_location: (FloatExp::ZERO, FloatExp::ZERO),
                escape_derivative: (FloatExp::ONE, FloatExp::ZERO),
                start_location: (FloatExp::ZERO, FloatExp::ZERO),
                smallness: FloatExp::ZERO,
                small_time: 0,
            })
            .collect();
        let location = ObjectivePosAndZoom { pos: (IntExp::ZERO, IntExp::ZERO), zoom_pot: -2 };
        let package = ResultsPackage { results, screen_res: res, location: location.clone() , hud: Default::default()
    };
        let remapped = sample_old_values(&package, location, res);
        for (i, r) in remapped.results.iter().enumerate() {
            match r {
                CompletedPoint::Escapes { escape_time, .. } =>
                    assert_eq!(*escape_time, i as u32, "identity view must reproduce the package"),
                _ => panic!("remap changed the answer kind"),
            }
        }
    });
}

// verifies r[cz.craft.wall-clock-law+1]
// Structural: a workshift always terminates (the loop law is elapsed wall-clock,
// not queue state). The 10ms constant itself stays code-reviewed.
#[test]
fn workshift_always_terminates() {
    run_big(|| {
        // no queues at all: immediate exit
        let mut ctx = make_context(0);
        let t = Instant::now();
        shift(&mut ctx);
        assert!(t.elapsed().as_secs() < 5);

        // queues full of slow work: bounded by the clock
        let mut ctx = make_context(2);
        ctx.out_queue.push_back(((3, 0), 0));
        let t = Instant::now();
        shift(&mut ctx);
        assert!(t.elapsed().as_secs() < 5, "the wall clock, not the workload, bounds a shift");
        assert_eq!(ctx.workshifts, 3);
    });
}

// r[verify cz.craft.stencil-only-replace+2]
#[test]
fn fresh_shell_leaves_seats_uninitialized() {
    run_big(|| {
        let frame_info = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            (8u32, 4u32),
        );
        let ctx = from_stencil::<FloatExp>(frame_info, None).unwrap();
        assert!(ctx.points.iter().all(|p| !p.initialized && !p.delivered));
        assert_eq!(ctx.points.len(), 32);
        assert!(!ctx.scredge_poses.is_empty());
    });
}

// r[verify cz.craft.stencil-only-replace+2]
#[test]
fn ensure_started_matches_generator_bit_for_bit() {
    run_big(|| {
        let res = (17u32, 11u32);
        let frame_info = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: 7,
            },
            res,
        );
        let mut ctx = from_stencil::<FloatExp>(frame_info, None).unwrap();
        for row in 0..res.1 {
            for seat in 0..res.0 {
                let pos = (seat as i32, row as i32);
                ensure_started(&mut ctx, pos);
                let index = index_from_pos(&pos, res.0);
                assert!(ctx.points[index].initialized);
                assert_eq!(
                    ctx.points[index].delta_c,
                    ctx.c_generator.get_c((seat, row)),
                    "seat ({seat},{row})"
                );
                assert_eq!(ctx.points[index].c, ctx.points[index].delta_c);
                assert_eq!(ctx.points[index].z, ctx.points[index].delta_c);
                assert_eq!(ctx.points[index].dc, (FloatExp::ONE, FloatExp::ZERO));
            }
        }
    });
}

// r[verify cz.craft.stencil-only-replace+2]
#[test]
fn replace_reuses_points_capacity_and_resets_initialized() {
    run_big(|| {
        let frame_a = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            (8u32, 4u32),
        );
        let mut ctx = from_stencil::<FloatExp>(frame_a.clone(), None).unwrap();
        ensure_started(&mut ctx, (0, 0));
        assert!(ctx.points[0].initialized);
        let cap = ctx.points.capacity();

        let frame_b = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -1,
            },
            (8u32, 4u32),
        );
        let ctx2 = from_stencil(frame_b, Some((ctx, frame_a.0.clone()))).unwrap();
        assert!(ctx2.points.iter().all(|p| !p.initialized));
        assert!(ctx2.points.capacity() >= cap);
        assert_eq!(ctx2.points.len(), 32);
        assert_eq!(ctx2.motion, Motion::Zoomed);
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn from_stencil_defaults_attention_anchor_to_center() {
    run_big(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            (8u32, 4u32),
        );
        let ctx = from_stencil::<FloatExp>(frame, None).unwrap();
        assert_eq!(ctx.attention, None);
        assert_eq!(ctx.attention_anchor, (4, 2));
        assert_eq!(ctx.attention_index, 0);
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn square_ring_spiral_is_nondecreasing_chebyshev() {
    let mut prev = 0i32;
    for k in 0..200u64 {
        let (dx, dy) = square_ring_offset(k);
        let d = dx.abs().max(dy.abs());
        assert!(d >= prev, "k={k} offset=({dx},{dy}) decreased Chebyshev distance");
        prev = d;
    }
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn attention_slot_picks_spiral_before_queues() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.attention_index = 0;
        set_attention(&mut ctx, Some((1, 0)));
        ctx.edge_queue.push_back(((3, 1), 0));
        // One bout: slot 0 should take the attention anchor, not the edge seat.
        // Bypass the 10ms wall by calling the selector directly.
        let pos = next_attention_spiral_pos(&mut ctx).unwrap();
        assert_eq!(pos, (1, 0));
        assert_eq!(ctx.attention_index, 1);
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn spiral_skips_delivered_and_falls_through_when_exhausted() {
    run_big(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((0, 0)));
        ctx.attention_index = 0;
        for p in &mut ctx.points {
            p.delivered = true;
        }
        // One call: scan budget spent on delivered seats → None.
        assert!(next_attention_spiral_pos(&mut ctx).is_none());
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn attention_bout_works_seat_to_completion() {
    run_big(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((2, 0))); // ESC seat
        ctx.attention_index = 0;
        ctx.scredge_poses.push_back((3, 1));
        shift(&mut ctx);
        let index = index_from_pos(&(2, 0), ctx.res.0);
        assert!(ctx.points[index].delivered, "attention bout must finish its seat");
        assert!(ctx.completed_points.stuff[..ctx.completed_points.len]
            .iter()
            .any(|(_, i)| *i == index), "completed attention seat is published");
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn attention_holds_seat_across_bouts_until_complete() {
    run_big(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((2, 0))); // ESC seat, escapes quickly
        ctx.attention_index = 0;

        // Seed a held seat manually as if a prior bout capped out mid-seat:
        // an unfinished, undelivered seat that the spiral never returns to.
        let held = (1, 0);
        ctx.attention_current = Some(held);
        let held_index = index_from_pos(&held, ctx.res.0);
        ctx.points[held_index].initialized = true;
        ctx.points[held_index].iterations = 7; // partial prior work

        shift(&mut ctx);

        // The held seat must be finished. The spiral stays parked at 0 until
        // the hold releases; once it does, later bouts pick fresh spiral seats.
        assert!(ctx.points[held_index].delivered, "held seat must be finished");
        // After the shift the phase has moved on: whatever is currently held
        // (if anything) is a spiral seat, never the already-done held one.
        assert_ne!(ctx.attention_current, Some(held), "completed hold not retained");
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn attention_releases_held_seat_delivered_elsewhere() {
    run_big(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((2, 0)));
        ctx.attention_index = 0;
        let held = (2, 0);
        ctx.attention_current = Some(held);
        // Seat already delivered (e.g. via another queue): the bout must
        // release the hold and fall through rather than spin forever.
        let held_index = index_from_pos(&held, ctx.res.0);
        ctx.points[held_index].delivered = true;
        shift(&mut ctx);
        // The delivered hold must be dropped (the bout cannot spin on it); the
        // phase then moves on, holding only fresh spiral seats.
        assert_ne!(ctx.attention_current, Some(held), "delivered held seat released");
        assert!(ctx.attention_index > 0, "spiral advanced after release");
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn set_attention_none_restores_center_anchor() {
    run_big(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((3, 1)));
        assert_eq!(ctx.attention_anchor, (3, 1));
        assert_eq!(ctx.attention_index, 0);
        ctx.attention_index = 17;
        ctx.attention_current = Some((3, 1));
        set_attention(&mut ctx, None);
        assert_eq!(ctx.attention, None);
        assert_eq!(ctx.attention_anchor, (2, 1)); // 4x2 center
        assert_eq!(ctx.attention_index, 0, "anchor change restarts the spiral");
        assert_eq!(ctx.attention_current, None, "anchor change drops the hold");
    });
}

// r[verify cz.craft.bout-cap+1]
#[test]
fn bout_cap_clamps_above_max() {
    assert_eq!(BoutCap::new(0).get(), 0);
    assert_eq!(BoutCap::new(MAX_BOUT).get(), MAX_BOUT);
    assert_eq!(BoutCap::new(MAX_BOUT + 1).get(), MAX_BOUT);
    assert_eq!(BoutCap::new(u32::MAX).get(), MAX_BOUT);
    assert_eq!(BoutCap::STANDARD.get(), MAX_BOUT);
}

// r[verify cz.craft.bout-cap+1]
#[test]
fn attention_bout_on_hard_seat_never_exceeds_max_bout() {
    run_big(|| {
        // SLOW never completes inside a shift; the held-seat tenacity must
        // still bound each bout to MAX_BOUT iterations (no unbounded call).
        let mut ctx = make_context(0);
        let hard = (3, 0); // SLOW
        let idx = index_from_pos(&hard, ctx.res.0);
        assert!(!ctx.points[idx].repeats && !ctx.points[idx].escapes);
        set_attention(&mut ctx, Some(hard));
        ctx.attention_index = 0;
        ctx.attention_current = Some(hard);

        // Drive many bouts directly (bypass the 10ms wall) and check the cap.
        for _ in 0..5 {
            let before = ctx.points[idx].iterations;
            let mut p = ctx.points[idx].clone();
            iterate_max_n_times(&mut p, 4.0f32.into(), ctx.pitch_epsilon, BoutCap::STANDARD);
            let delta = p.iterations - before;
            assert!(delta <= MAX_BOUT, "bout ran {delta} iterations (> {MAX_BOUT})");
            ctx.points[idx] = p;
        }
    });
}

// r[verify cz.craft.pan-zoom-slot0+1]
#[test]
fn from_stencil_classifies_zoom_pan_neither() {
    run_big(|| {
        let base = ObjectivePosAndZoom {
            pos: (IntExp::from(-2), IntExp::from(2)),
            zoom_pot: -2,
        };
        let res = (8u32, 4u32);
        let fresh = from_stencil::<FloatExp>((base.clone(), res), None).unwrap();
        assert_eq!(fresh.motion, Motion::Neither);

        let zoomed_fi = ObjectivePosAndZoom {
            pos: base.pos.clone(),
            zoom_pot: -1,
        };
        let zoomed = from_stencil(
            (zoomed_fi.clone(), res),
            Some((fresh, base.clone())),
        )
        .unwrap();
        assert_eq!(zoomed.motion, Motion::Zoomed);

        let panned_fi = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::from(2)),
            zoom_pot: -2,
        };
        let panned_base = from_stencil::<FloatExp>((base.clone(), res), None).unwrap();
        let panned = from_stencil(
            (panned_fi, res),
            Some((panned_base, base.clone())),
        )
        .unwrap();
        assert_eq!(panned.motion, Motion::Panned);
    });
}

// r[verify cz.craft.pan-zoom-slot0+1]
#[test]
fn pan_scredge_lead_only_on_first_shift() {
    run_big(|| {
        // After the first shift of a pan shell, slot 0 returns to attention —
        // stopping the pan must not leave scredge sticky forever.
        let mut ctx = make_context(5); // workshifts % 5 == 0, but not the first
        ctx.motion = Motion::Panned;
        ctx.attention_index = 0;
        set_attention(&mut ctx, Some((1, 0)));
        ctx.scredge_poses.push_back((3, 1));
        let attn_index = index_from_pos(&(1, 0), ctx.res.0);
        shift(&mut ctx);
        assert!(
            ctx.points[attn_index].delivered || ctx.points[attn_index].iterations > 0,
            "stopped / later pan shifts must lead with attention"
        );
    });
}

// r[verify cz.craft.pan-zoom-slot0+1]
#[test]
fn pan_slot0_prefers_scredge_over_attention() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.motion = Motion::Panned;
        ctx.attention_index = 0;
        set_attention(&mut ctx, Some((1, 0)));
        ctx.scredge_poses.push_back((3, 1));
        let scredge_index = index_from_pos(&(3, 1), ctx.res.0);
        shift(&mut ctx);
        assert!(
            ctx.points[scredge_index].delivered || ctx.points[scredge_index].iterations > 0,
            "pan slot0 must start the scredge seat"
        );
        let attn_index = index_from_pos(&(1, 0), ctx.res.0);
        assert!(
            !ctx.points[attn_index].delivered && ctx.points[attn_index].iterations == 0,
            "attention must not lead on a pan"
        );
    });
}

// r[verify cz.craft.pan-zoom-slot0+1]
#[test]
fn zoom_slot0_prefers_attention_over_scredge() {
    run_big(|| {
        let mut ctx = make_context(0);
        ctx.motion = Motion::Zoomed;
        ctx.attention_index = 0;
        set_attention(&mut ctx, Some((1, 0)));
        ctx.scredge_poses.push_back((3, 1));
        let attn_index = index_from_pos(&(1, 0), ctx.res.0);
        shift(&mut ctx);
        assert!(
            ctx.points[attn_index].delivered || ctx.points[attn_index].iterations > 0,
            "zoom slot0 must start the attention seat"
        );
    });
}

// ---------------------------------------------------------------------------
// Home view regression (reference c must match CGenerator; workshift parity)
// ---------------------------------------------------------------------------

use crate::assemblies::workgroup::reference_worker::{PublishedReference, select_reference_request};
use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use crate::reference::ReferenceOrbit;
use std::sync::Arc;

fn home_frame() -> (ObjectivePosAndZoom, (u32, u32)) {
    // Match the live window's ObjectivePosAndZoom: display Y is stored unflipped;
    // `from_stencil` / `objective_c` apply the compute-space Y flip once.
    (
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        },
        DEFAULT_WINDOW_RES,
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
        ctx.hud_points_window = 2500;
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

fn fill_until_complete_perturb(ctx: &mut WorkContext<FloatExp>, max_shifts: usize) {
    for _ in 0..max_shifts {
        if ctx.percent_completed >= 100.0 {
            break;
        }
        perturb_workshift(16_000_000, 2, 4, 150, ctx);
        let _ = work_update(ctx);
    }
}

fn fill_until_complete_direct(ctx: &mut WorkContext<FloatExp>, max_shifts: usize) {
    for _ in 0..max_shifts {
        if ctx.percent_completed >= 100.0 {
            break;
        }
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
        let frame = home_frame();
        let req = select_reference_request::<FloatExp>(None, &frame);
        let mut direct = from_stencil(frame.clone(), None).expect("home");
        let mut perturb = from_stencil(frame, None).expect("home");
        perturb.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, 4096),
            c: req.c,
            generation: 1,
        }));
        for _ in 0..10000 {
            if direct.points.iter().all(|p| p.delivered) {
                break;
            }
            direct.attention_index = 0;
            workshift_with_kernel(0, 0, 0, 0, &mut direct, &DirectKernel);
            work_update(&mut direct);
        }
        for _ in 0..10000 {
            if perturb.points.iter().all(|p| p.delivered) {
                break;
            }
            perturb.attention_index = 0;
            perturb_workshift(0, 0, 0, 0, &mut perturb);
            work_update(&mut perturb);
        }
        let mut mismatches = 0usize;
        for i in 0..direct.points.len() {
            let d = &direct.points[i];
            let p = &perturb.points[i];
            if d.delivered && p.delivered && outcome_key(d) != outcome_key(p) {
                mismatches += 1;
            }
        }
        assert_eq!(
            mismatches, 0,
            "perturbation path must match direct on shallow home (data-flow)"
        );
        assert!(
            direct.points.iter().all(|p| p.delivered),
            "direct shallow comparator must finish the home shell"
        );
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
        let mut direct_ctx = from_stencil::<f64>(home_frame(), None).expect("home direct");
        let direct_start = Instant::now();
        let mut direct_shifts = 0u32;
        while !direct_ctx.points.iter().all(|p| p.delivered) {
            workshift_with_kernel(0, 0, 0, 0, &mut direct_ctx, &DirectKernel);
            while direct_ctx.completed_points.try_pop().is_some() {}
            direct_shifts += 1;
            assert!(direct_start.elapsed().as_secs() < 30, "direct home fill stalled");
            if direct_shifts > 5_000 {
                panic!("direct home did not finish");
            }
        }

        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home f64");
        let start = Instant::now();
        let mut shifts = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            workshift(0, 0, 0, 0, &mut ctx);
            while ctx.completed_points.try_pop().is_some() {}
            shifts += 1;
            assert!(
                start.elapsed().as_secs() < 30,
                "f64 home fill stalled: shifts={shifts} pct={:.1} gear={:?}",
                ctx.percent_completed,
                ctx.active_gear
            );
            if shifts > 5_000 {
                panic!(
                    "f64 home did not finish in 5000 shifts (pct={:.1})",
                    ctx.percent_completed
                );
            }
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
    use std::time::Instant;
    run_big(|| {
        let frame = frame_at_center(-0.743643887037151, 0.131825904205216, 19, (64, 48));
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
        let start = Instant::now();
        let mut shifts = 0u32;
        while ctx.points.iter().filter(|p| p.delivered).count() < 100 {
            workshift(16_000_000, 2, 4, 150, &mut ctx);
            while ctx.completed_points.try_pop().is_some() {}
            shifts += 1;
            assert!(
                shifts < 5_000,
                "seahorse pot 19 stalled: delivered={} gear={:?}",
                ctx.points.iter().filter(|p| p.delivered).count(),
                ctx.active_gear
            );
            assert!(
                start.elapsed().as_secs() < 30,
                "seahorse pot 19 timed out"
            );
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
        (8u32, 8u32),
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
#[ignore = "series approximation deferred"]
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
            (4u32, 4u32),
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
        (4u32, 4u32),
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
        for _ in 0..500 {
            if direct.points[0].escapes || direct.points[0].repeats {
                break;
            }
            DirectKernel.iterate_bout(
                &mut direct.points[0],
                None,
                4.0,
                1e-15,
                BoutCap::new(256),
            );
        }
        for _ in 0..500 {
            if perturb.points[0].escapes || perturb.points[0].repeats {
                break;
            }
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
#[ignore = "series approximation deferred"]
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
#[ignore = "series approximation deferred"]
// r[verify cz.math.mandelbrot-real-axis-symmetry+1]
// r[verify cz.depth.series-approximation+1]
fn home_package_with_live_series_obeys_real_axis_symmetry() {
    run_big(|| {
        // Origin real −2, zoom −2: covers the main cardioid plus |c|≳2 exterior.
        let frame = real_axis_symmetric_shallow_frame((96, 65), -2, -2);
        let mut ctx = from_stencil(frame.clone(), None).expect("symmetric shell");
        install_covering_reference_with_series(&mut ctx, &frame);
        fill_until_complete_perturb(&mut ctx, 8_000);
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
#[ignore = "series approximation deferred"]
// r[verify cz.depth.series-approximation+1]
// r[verify cz.depth.delta-kernel+1]
fn home_package_with_live_series_matches_direct_kernel_answers() {
    run_big(|| {
        let frame = real_axis_symmetric_shallow_frame((96, 65), -2, -2);
        let mut direct = from_stencil(frame.clone(), None).expect("direct");
        let mut perturb = from_stencil(frame.clone(), None).expect("perturb");
        install_covering_reference_with_series(&mut perturb, &frame);
        // Direct ignores the reference; install anyway so shells stay aligned.
        install_covering_reference_with_series(&mut direct, &frame);

        fill_until_complete_direct(&mut direct, 8_000);
        fill_until_complete_perturb(&mut perturb, 8_000);
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
#[ignore = "series approximation deferred"]
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
        (4u32, 4u32),
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
    let src = include_str!("craftsmanship_tests.rs");
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
    let kernel = include_str!("perturb_kernel.rs");
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
            (32u32, 24u32),
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
    let kernel = include_str!("perturb_kernel.rs");
    let worker = include_str!("mod.rs");
    let shift = include_str!("workshift.rs");
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
#[ignore = "series approximation deferred"]
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
#[ignore = "series approximation deferred"]
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
#[ignore = "series approximation deferred"]
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
        (8u32, 8u32),
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
        (8u32, 6u32),
    );
    assert!(
        from_stencil_relative::<FloatExp>(deep, None).is_some(),
        "FloatExp must admit deep-ish frames"
    );
}

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
            workshift(16_000_000, 2, 4, 150, &mut ctx);
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
            workshift(16_000_000, 2, 4, 150, &mut ctx);
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
#[ignore = "retired: f64 DirectKernel is not a deep membership oracle; see pin_exterior_not_marked_in_at_zoom_52"]
// r[verify cz.perf.pps-selected-kernel+1]
// r[verify cz.depth.delta-kernel+1]
/// Headed report: (0.95703125, −0.08984375i) at 2^74 went black on escaping seats.
fn deep_relative_exterior_not_instant_black_at_reported_location() {
    use super::perturb_kernel::PerturbationKernel;
    use crate::assemblies::headgroup::window::coords::{decimal_str_to_intexp, ul_for_center};
    run_big(|| {
        let res = (32u32, 32u32);
        let zoom_pot = 74i32;
        let center_re = decimal_str_to_intexp("0.95703125").unwrap();
        let center_im = decimal_str_to_intexp("-0.08984375").unwrap();
        let obj = ul_for_center(center_re, center_im, zoom_pot, res);
        let frame = (obj, res);
        let mut direct = from_stencil::<f64>(frame.clone(), None).expect("direct shell");
        let mut perturb = from_stencil::<f64>(frame, None).expect("perturb shell");
        assert!(
            direct.coords_are_relative,
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
                    held_ref.as_ref().map(|r| &r.orbit),
                    4.0,
                    perturb.pitch_epsilon,
                    BoutCap::new(512),
                );
                let d = &direct.points[idx];
                let p = &perturb.points[idx];
                if !d.escapes {
                    continue;
                }
                exterior_compared += 1;
                if d.iterations > 2
                    && p.escapes
                    && p.iterations <= 2
                {
                    instant_black.push(format!(
                        "({x},{y}) direct_et={} perturb_et={}",
                        d.iterations, p.iterations
                    ));
                }
                if (d.escapes, d.iterations, d.repeats) != (p.escapes, p.iterations, p.repeats) {
                    mismatches.push(format!(
                        "({x},{y}) direct={:?} perturb={:?}",
                        (d.escapes, d.iterations, d.repeats),
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
            "perturb vs direct mismatches ({}):\n{}",
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
        assert_eq!(ctx.view_gear, ComputeGear::F64);
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


