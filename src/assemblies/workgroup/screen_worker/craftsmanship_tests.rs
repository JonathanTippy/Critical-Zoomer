// Property and example tests binding the craftsmanship inventory
// (docs/assistant/tracey/craftsmanship-rules.md) to the v0.0.9 workgroup code.
// Each test names the rule it verifies.
//
// Seam note: WorkContext's completion buffer is a heap Vec-backed Stec
// (capacity 100000). Tests that build one still use run_big for headroom.

use std::time::Instant;
use std::collections::VecDeque;

use proptest::prelude::*;

use super::work_update;
use super::workshift::*;
use crate::assemblies::headgroup::window::sampling::index_from_relative_location;
use crate::assemblies::workgroup::c_generator::CGenerator;
use crate::assemblies::workgroup::work_collector::{sample_old_values, ResultsPackage};
use crate::assemblies::workgroup::screen_worker::workshift::get_random_mixmap;
use crate::utils::{index_from_pos, IntExp, ObjectivePosAndZoom};

fn run_big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn make_point(c: (f64, f64)) -> Point<f64> {
    Point {
        c,
        z: (0.0, 0.0),
        dc: (0.0, 0.0),
        real_squared: 0.0,
        imag_squared: 0.0,
        real_imag: 0.0,
        iterations: 0,
        loop_detection_point: ((0.0, 0.0), 0),
        escapes: false,
        repeats: false,
        delivered: false,
        initialized: true,
        period: 0,
        smallness_squared: f64::MAX,
        small_time: 0,
        delta: None,
        direct_only: false,
    }
}

/// 4x2 screen. points[0]/points[1] sit 1e-9 apart so the pitch epsilon is
/// ~4e-12; `esc` escapes in ~2 iterations; `slow` is near-parabolic interior
/// (multiplier ~1-2e-8, needs ~1e9 iterations to trip loop detection — far
/// beyond any 10ms shift), so it never completes inside a test.
const ESC: (f64, f64) = (2.0, 2.0);
const SLOW: (f64, f64) = (0.25 - 1e-16, 0.0);

fn make_context(workshifts: u32) -> WorkContext<f64> {
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
    let points: Vec<Point<f64>> = cs.iter().map(|&c| make_point(c)).collect();
    let n = points.len();
    let c_generator = CGenerator::new(&(IntExp::from(0), IntExp::from(0)), -2, res).unwrap();
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
        pitch_epsilon: 1e-9 * (1.0 / 256.0),
        latest_reference: None,
    }
}

fn shift(ctx: &mut WorkContext<f64>) {
    // Scheduler tests isolate queue policy from production numerics.
    workshift_with_kernel(0, 0, 0, 0, ctx, &DirectKernel);
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

fn orbit_with_derivative(c: (f64, f64), iterations: u32) -> Point<f64> {
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
        let finite = ((z_hi.0 - z_lo.0) / (hi - lo), (z_hi.1 - z_lo.1) / (hi - lo));
        let tolerance = iterations as f64 * 1.0e-4;
        prop_assert!((center.dc.0 - finite.0).abs() <= tolerance * center.dc.0.abs().max(1.0));
        prop_assert!((center.dc.1 - finite.1).abs() <= tolerance * center.dc.1.abs().max(1.0));
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
    let ea: f64 = pitch_epsilon(&a);
    let eb: f64 = pitch_epsilon(&b);
    assert!(ea > 0.0);
    assert_eq!(eb, ea * 2.0, "doubling pixel pitch must double epsilon");
}

// verifies r[cz.craft.cached-products+1]
proptest! {
    #[test]
    fn cached_products_match_z(zr in -1000.0f64..1000.0, zi in -1000.0f64..1000.0) {
        let mut p = make_point((0.1, 0.1));
        p.z = (zr, zi);
        p.iterations = 17;
        update_point_results(&mut p);
        prop_assert_eq!(p.real_squared, zr * zr);
        prop_assert_eq!(p.imag_squared, zi * zi);
        prop_assert_eq!(p.real_imag, zr * zi);
        // smallness collected as a free side effect
        prop_assert_eq!(p.smallness_squared, zr * zr + zi * zi);
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
        let results: Vec<CompletedPoint<f64>> = (0..(res.0 * res.1))
            .map(|i| CompletedPoint::Escapes {
                escape_time: i,
                escape_location: (0.0, 0.0),
                escape_derivative: (1.0, 0.0),
                start_location: (0.0, 0.0),
                smallness: 0.0,
                small_time: 0,
            })
            .collect();
        let location = ObjectivePosAndZoom { pos: (IntExp::ZERO, IntExp::ZERO), zoom_pot: -2 };
        let package = ResultsPackage { results, screen_res: res, location: location.clone() };
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
        let ctx = from_stencil::<f64>(frame_info, None).unwrap();
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
        let mut ctx = from_stencil::<f64>(frame_info, None).unwrap();
        for row in 0..res.1 {
            for seat in 0..res.0 {
                let pos = (seat as i32, row as i32);
                ensure_started(&mut ctx, pos);
                let index = index_from_pos(&pos, res.0);
                assert!(ctx.points[index].initialized);
                assert_eq!(
                    ctx.points[index].c,
                    ctx.c_generator.get_c((seat, row)),
                    "seat ({seat},{row})"
                );
                assert_eq!(ctx.points[index].z, ctx.points[index].c);
                assert_eq!(ctx.points[index].dc, (1.0, 0.0));
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
        let mut ctx = from_stencil::<f64>(frame_a.clone(), None).unwrap();
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
        let ctx = from_stencil::<f64>(frame, None).unwrap();
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
        let fresh = from_stencil::<f64>((base.clone(), res), None).unwrap();
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
        let panned_base = from_stencil::<f64>((base.clone(), res), None).unwrap();
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
use crate::assemblies::workgroup::screen_worker::perturb_kernel::PerturbationKernel;
use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use crate::reference::ReferenceOrbit;
use std::sync::Arc;

fn home_frame() -> (ObjectivePosAndZoom, (u32, u32)) {
    (
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::ZERO - IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        },
        DEFAULT_WINDOW_RES,
    )
}

fn outcome_key(p: &Point<f64>) -> (bool, bool, u32, u32) {
    (p.escapes, p.repeats, p.iterations, p.period)
}

#[test]
fn home_reference_request_matches_c_generator() {
    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<f64>(None, &frame);
        let ctx = from_stencil(frame, None).expect("home view");
        let center = (ctx.res.0 / 2, ctx.res.1 / 2);
        let gc = ctx.c_generator.get_c((center.0, center.1));
        let req_f = (f64::from(req.c.0.clone()), f64::from(req.c.1.clone()));
        assert_eq!(req_f, gc, "reference request must use the same c grid as seats");
    });
}

/// Shallow f64-valid data-flow check only: DirectKernel is not a deep-zoom
/// oracle. Ground truth at depth is the rug precision-doubling oracle.
#[test]
fn home_workshift_with_reference_matches_direct() {
    run_big(|| {
        let frame = home_frame();
        let req = select_reference_request::<f64>(None, &frame);
        let mut direct = from_stencil(frame.clone(), None).expect("home");
        let mut perturb = from_stencil(frame, None).expect("home");
        perturb.latest_reference = Some(Arc::new(PublishedReference {
            orbit: ReferenceOrbit::compute(&req.c, req.precision_bits, req.max_iterations),
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
            workshift(0, 0, 0, 0, &mut perturb);
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
        PerturbationKernel.start_seat(&mut ctx, (0, 0));
        PerturbationKernel.iterate_bout(
            &mut ctx.points[0], None, 4.0, 1e-15, BoutCap::new(max_n),
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
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    PerturbationKernel.iterate_bout(
        &mut ctx.points[0], None, 4.0, ctx.pitch_epsilon, BoutCap::new(2),
    );
    assert!(ctx.points[0].repeats);
    assert_eq!(ctx.points[0].period, 1);
}

#[test]
// r[verify cz.ref.zero-orbit-same-path+1 cz.depth.delta-kernel+1]
fn zero_orbit_floor_matches_direct_kernel_escape_times() {
    // Shallow f64-valid comparator only; deep truth is the rug doubling oracle.
    // Start both seats at production z₀=c (not classical z=0).
    for c in [(2.0, 2.0), (-1.0, 0.2), (0.4, 0.4), (-0.75, 0.1)] {
        let mut direct_ctx = make_context(0);
        direct_ctx.points[0] = make_point(c);
        direct_ctx.points[0].z = c;
        direct_ctx.points[0].dc = (1.0, 0.0);
        let mut perturb_ctx = direct_ctx.clone();
        DirectKernel.start_seat(&mut direct_ctx, (0, 0));
        PerturbationKernel.start_seat(&mut perturb_ctx, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct_ctx.points[0], None, 4.0, 1e-15, BoutCap::new(512),
        );
        PerturbationKernel.iterate_bout(
            &mut perturb_ctx.points[0], None, 4.0, 1e-15, BoutCap::new(512),
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
        direct_ctx.points[0].z = c;
        direct_ctx.points[0].dc = (1.0, 0.0);
        let mut perturb_ctx = direct_ctx.clone();
        perturb_ctx.latest_reference = Some(published.clone());
        DirectKernel.start_seat(&mut direct_ctx, (0, 0));
        PerturbationKernel.start_seat(&mut perturb_ctx, (0, 0));
        DirectKernel.iterate_bout(
            &mut direct_ctx.points[0], None, 4.0, 1e-15, BoutCap::new(512),
        );
        PerturbationKernel.iterate_bout(
            &mut perturb_ctx.points[0],
            Some(&published.orbit),
            4.0,
            1e-15,
            BoutCap::new(512),
        );
        assert_eq!(
            outcome_key(&direct_ctx.points[0]),
            outcome_key(&perturb_ctx.points[0]),
            "published reference must match direct on shallow c={c:?}"
        );
    }
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
    PerturbationKernel.start_seat(&mut ctx, (2, 0));
    let initial_dz = ctx.points[2].delta.as_ref().unwrap().dz;
    PerturbationKernel.iterate_bout(
        &mut ctx.points[2],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        4.0,
        ctx.pitch_epsilon,
        BoutCap::new(5),
    );
    assert!(ctx.points[2].iterations > 0);
    ctx.latest_reference = Some(Arc::new(PublishedReference {
        orbit: ReferenceOrbit::compute(&(IntExp::ZERO, IntExp::ZERO), 64, 32),
        c: (IntExp::ZERO, IntExp::ZERO),
        generation: 2,
    }));
    PerturbationKernel.start_seat(&mut ctx, (2, 0));
    let delta = ctx.points[2].delta.as_ref().unwrap();
    assert_eq!(delta.generation, 2);
    assert_eq!(delta.dz, initial_dz);
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
        dz: ComplexFloatExp::new(FloatExp::from(0.25), FloatExp::ZERO),
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc: ComplexFloatExp::ZERO,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation: 7,
    });
    PerturbationKernel.iterate_bout(
        &mut ctx.points[0],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        4.0,
        1e-15,
        BoutCap::new(1),
    );
    assert!(ctx.points[0].direct_only);
    assert!(ctx.points[0].delta.is_none());
    assert!(!ctx.points[0].initialized);
    assert!(!ctx.points[0].escapes && !ctx.points[0].repeats);
    assert!(!ctx.points[0].delivered);
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    assert_eq!(ctx.points[0].delta.as_ref().unwrap().generation, 0);
    PerturbationKernel.iterate_bout(
        &mut ctx.points[0], None, 4.0, ctx.pitch_epsilon, BoutCap::new(2),
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
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    PerturbationKernel.iterate_bout(
        &mut ctx.points[0],
        ctx.latest_reference.as_ref().map(|r| &r.orbit),
        4.0,
        1e-15,
        BoutCap::new(20),
    );
    assert!(ctx.points[0].iterations > 0 && ctx.points[0].iterations < 20);
    assert!(!ctx.points[0].escapes && !ctx.points[0].repeats);
    assert!(!ctx.points[0].delivered);
    assert!(ctx.points[0].delta.is_some());
}

#[test]
fn perturbation_bout_obeys_cap_and_split_bouts_match() {
    let mut ctx = make_context(0);
    ctx.points[0] = make_point((-0.1, 0.65));
    PerturbationKernel.start_seat(&mut ctx, (0, 0));
    let mut whole = ctx.points[0].clone();
    let mut split = whole.clone();
    PerturbationKernel.iterate_bout(
        &mut whole, None, 4.0, 1e-15, BoutCap::new(17),
    );
    PerturbationKernel.iterate_bout(
        &mut split, None, 4.0, 1e-15, BoutCap::new(5),
    );
    assert!(split.iterations <= 5);
    PerturbationKernel.iterate_bout(
        &mut split, None, 4.0, 1e-15, BoutCap::new(12),
    );
    assert_eq!(whole.iterations, split.iterations);
    assert_eq!(whole.escapes, split.escapes);
    assert_eq!(whole.repeats, split.repeats);
    assert_eq!(whole.z, split.z);
    assert_eq!(whole.dc, split.dc);
    assert_eq!(whole.delta.as_ref().unwrap().dz, split.delta.as_ref().unwrap().dz);
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
        PerturbationKernel.start_seat(&mut ctx, (0, 0));
        PerturbationKernel.iterate_bout(
            &mut ctx.points[0], None, 4.0, 1e-15, BoutCap::new(5),
        );
        let expected = rug_orbit_derivative((0.1, ci), 6);
        let actual = ctx.points[0].dc;
        assert!((actual.0 - expected.0).abs() < 1e-12);
        assert!((actual.1 - expected.1).abs() < 1e-12);
        derivatives.push(actual);
    }
    assert!((derivatives[0].0 - derivatives[1].0).abs() < 1e-12);
    assert!((derivatives[0].1 + derivatives[1].1).abs() < 1e-12);
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
    ] {
        assert!(
            src.contains(&format!("fn {name}")),
            "missing phase-two inventory test `{name}`"
        );
    }
    for needle in [
        "delta: None",
        "direct_only: false",
        "latest_reference: None",
        "workshift_with_kernel(0, 0, 0, 0, ctx, &DirectKernel)",
    ] {
        assert!(
            src.contains(needle),
            "missing phase-two fixture state `{needle}`"
        );
    }
}

