/// Hard wall budget for every craftsmanship test. Shift caps are banned; if the
/// code under test is wrong a fill may never finish — this is the only halt.
const TEST_WALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(1);

thread_local! {
    static TEST_BUDGET_START: std::cell::Cell<Option<std::time::Instant>> =
        std::cell::Cell::new(None);
}

fn check_test_budget() {
    TEST_BUDGET_START.with(|c| {
        let Some(start) = c.get() else {
            return;
        };
        let elapsed = start.elapsed();
        assert!(
            elapsed <= TEST_WALL_BUDGET,
            "test exceeded 1s wall budget ({elapsed:?})"
        );
    });
}

fn run_big_stack_size(f: impl FnOnce() + Send + 'static) {
    let join = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            TEST_BUDGET_START.with(|c| c.set(Some(std::time::Instant::now())));
            f();
            check_test_budget();
        })
        .expect("run_big stack thread");
    match join.join() {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Restart the 1s wall budget after expensive setup (GPU adapter, long orbits)
/// so the budget measures the fill under test, not adapter bring-up.
fn refresh_test_budget() {
    TEST_BUDGET_START.with(|c| c.set(Some(std::time::Instant::now())));
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

/// `TEST_SCREEN_RES` grid. points[0]/points[1] sit 1e-9 apart so the pitch
/// epsilon is ~4e-12; `esc` escapes in ~2 iterations; `slow` is near-parabolic
/// interior (multiplier ~1-2e-8, needs ~1e9 iterations to trip loop detection
/// — far beyond any 10ms shift), so it never completes inside a test.
const ESC: (f64, f64) = (2.0, 2.0);
const SLOW: (f64, f64) = (0.25 - 1e-16, 0.0);

fn make_context(workshifts: u32) -> WorkContext<FloatExp> {
    let res = TEST_SCREEN_RES;
    let n = (res.0 as usize) * (res.1 as usize);
    let mut points: Vec<Point<FloatExp>> = (0..n).map(|_| make_point(SLOW)).collect();
    points[0] = make_point((0.0, 0.0));
    points[1] = make_point((1e-9, 0.0));
    points[2] = make_point(ESC);
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
        last_used_naive_gpu: false,
        manual_gear: None,
        pps_locked_kernel: None,
        pps_lock_started: Instant::now(),
        pps_probe_queue: Vec::new(),
        pps_probe_shifts_left: 0,
        pps_probe_points: 0,
        pps_probe_started: Instant::now(),
        pps_probe_samples: Vec::new(),
        reference_library: Vec::new(),
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
        // Motion only affects slot 0. For each of slots 1..=4, Panned and
        // Zoomed contexts must make the same leading pick.
        // Slot 1..=3 lead edge; slot 4 leads scredge.
        for slot in 1u32..=4 {
            let lead = if slot == 4 { (3, 1) } else { (2, 0) };
            let lead_index = index_from_pos(&lead, TEST_SCREEN_RES.0);
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
        let mut ctx = make_context(2); // % 5 == 2: out first
        ctx.out_queue.push_back(((3, 0), 0));
        ctx.out_queue.push_back(((3, 1), 0));
        shift(&mut ctx);
        assert_eq!(ctx.out_queue.len(), 2, "slow out seats rotate to the back, never dropped");
        let poses: Vec<(i32, i32)> = ctx.out_queue.iter().map(|(p, _)| *p).collect();
        assert!(poses.contains(&(3, 0)) && poses.contains(&(3, 1)));
        assert!(!ctx.points[index_from_pos(&(3, 0), ctx.res.0)].delivered);
    });
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
        let res = TEST_SCREEN_RES;
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
    run_big_stack_size(|| {
        // no queues at all: immediate exit
        let mut ctx = make_context(0);
        let t = Instant::now();
        shift(&mut ctx);
        assert!(t.elapsed() <= TEST_WALL_BUDGET);

        // queues full of slow work: bounded by the clock
        let mut ctx = make_context(2);
        ctx.out_queue.push_back(((3, 0), 0));
        let t = Instant::now();
        shift(&mut ctx);
        assert!(
            t.elapsed() <= TEST_WALL_BUDGET,
            "the wall clock, not the workload, bounds a shift"
        );
        assert_eq!(ctx.workshifts, 3);
    });
}

// r[verify cz.craft.stencil-only-replace+2]
#[test]
fn fresh_shell_leaves_seats_uninitialized() {
    run_big_stack_size(|| {
        let frame_info = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
        );
        let ctx = from_stencil::<FloatExp>(frame_info, None).unwrap();
        assert!(ctx.points.iter().all(|p| !p.initialized && !p.delivered));
        assert_eq!(
            ctx.points.len(),
            TEST_SCREEN_RES.0 as usize * TEST_SCREEN_RES.1 as usize
        );
        assert!(!ctx.scredge_poses.is_empty());
    });
}

// r[verify cz.craft.stencil-only-replace+2]
#[test]
fn ensure_started_matches_generator_bit_for_bit() {
    run_big_stack_size(|| {
        let res = TEST_SCREEN_RES;
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
    run_big_stack_size(|| {
        let frame_a = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
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
            TEST_SCREEN_RES,
        );
        let ctx2 = from_stencil(frame_b, Some((ctx, frame_a.0.clone()))).unwrap();
        assert!(ctx2.points.iter().all(|p| !p.initialized));
        assert!(ctx2.points.capacity() >= cap);
        assert_eq!(
            ctx2.points.len(),
            TEST_SCREEN_RES.0 as usize * TEST_SCREEN_RES.1 as usize
        );
        assert_eq!(ctx2.motion, Motion::Zoomed);
    });
}

// r[verify cz.craft.attention-spiral+1]
#[test]
fn from_stencil_defaults_attention_anchor_to_center() {
    run_big_stack_size(|| {
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
        );
        let ctx = from_stencil::<FloatExp>(frame, None).unwrap();
        assert_eq!(ctx.attention, None);
        assert_eq!(
            ctx.attention_anchor,
            ((TEST_SCREEN_RES.0 / 2) as i32, (TEST_SCREEN_RES.1 / 2) as i32)
        );
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((3, 1)));
        assert_eq!(ctx.attention_anchor, (3, 1));
        assert_eq!(ctx.attention_index, 0);
        ctx.attention_index = 17;
        ctx.attention_current = Some((3, 1));
        set_attention(&mut ctx, None);
        assert_eq!(ctx.attention, None);
        assert_eq!(
            ctx.attention_anchor,
            ((TEST_SCREEN_RES.0 / 2) as i32, (TEST_SCREEN_RES.1 / 2) as i32),
        );
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
        let base = ObjectivePosAndZoom {
            pos: (IntExp::from(-2), IntExp::from(2)),
            zoom_pot: -2,
        };
        let res = TEST_SCREEN_RES;
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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
    run_big_stack_size(|| {
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

