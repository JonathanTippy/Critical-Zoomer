/// Test pyramid wall timeouts. The join *must* fire — a hung fill cannot
/// sit on `thread::join()` forever. Inner `check_test_budget` is the same
/// clock; `refresh_test_budget` only skips GPU bring-up, not the join.
const UNIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const INTEGRATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const E2E_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

thread_local! {
    static TEST_BUDGET_START: std::cell::Cell<Option<std::time::Instant>> =
        std::cell::Cell::new(None);
    static TEST_BUDGET_LIMIT: std::cell::Cell<std::time::Duration> =
        std::cell::Cell::new(UNIT_TIMEOUT);
}

fn check_test_budget() {
    TEST_BUDGET_START.with(|c| {
        let Some(start) = c.get() else {
            return;
        };
        let limit = TEST_BUDGET_LIMIT.with(|l| l.get());
        let elapsed = start.elapsed();
        assert!(
            elapsed <= limit,
            "test exceeded {limit:?} wall budget ({elapsed:?})"
        );
    });
}

fn run_with_timeout(limit: std::time::Duration, f: impl FnOnce() + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    let join = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            TEST_BUDGET_LIMIT.with(|c| c.set(limit));
            TEST_BUDGET_START.with(|c| c.set(Some(std::time::Instant::now())));
            f();
            check_test_budget();
        })
        .expect("run_big stack thread");
    std::thread::spawn(move || {
        let _ = tx.send(join.join());
    });
    match rx.recv_timeout(limit) {
        Ok(Ok(())) => {}
        Ok(Err(payload)) => std::panic::resume_unwind(payload),
        Err(_) => panic!(
            "test exceeded {limit:?} wall timeout (hung; join no longer waits forever)"
        ),
    }
}

fn run_big_stack_size(f: impl FnOnce() + Send + 'static) {
    run_with_timeout(UNIT_TIMEOUT, f);
}

fn run_integration(f: impl FnOnce() + Send + 'static) {
    run_with_timeout(INTEGRATION_TIMEOUT, f);
}

fn run_e2e(f: impl FnOnce() + Send + 'static) {
    run_with_timeout(E2E_TIMEOUT, f);
}

/// Restart the 1s wall budget after expensive setup (GPU adapter, long orbits)
/// so the budget measures the fill under test, not adapter bring-up.
fn refresh_test_budget() {
    TEST_BUDGET_START.with(|c| c.set(Some(std::time::Instant::now())));
}

/// Drop the 1s wall budget for long post-fill settle probes (explicit 10s gates).
fn suspend_test_budget() {
    TEST_BUDGET_START.with(|c| c.set(None));
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
        completed_points: Vec::with_capacity(100_000),
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
        c_generator_margin_bits: crate::assemblies::workgroup::c_generator::DEFAULT_C_GENERATOR_MARGIN_BITS,
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
    assert_eq!(a.completed_points.len(), b.completed_points.len());
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

// r[verify cz.craft.epsilon-pixel-pitch+1]
#[test]
fn epsilon_scales_with_pixel_pitch() {
    let a = vec![make_point((0.0, 0.0)), make_point((0.25, 0.0))];
    let b = vec![make_point((0.0, 0.0)), make_point((0.5, 0.0))];
    let ea = pitch_epsilon(&a);
    let eb = pitch_epsilon(&b);
    assert!(ea > FloatExp::ZERO);
    assert_eq!(eb, ea * FloatExp::TWO, "doubling pixel pitch must double epsilon");
}

// r[verify cz.craft.cached-products+1]
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

    // r[verify cz.craft.mixmap-shuffle+1]
    #[test]
    fn mixmap_is_permutation(n in 1usize..2048) {
        let mut m = get_random_mixmap(n);
        m.sort_unstable();
        prop_assert_eq!(m, (0..n).collect::<Vec<_>>());
    }

    // r[verify cz.craft.clamped-remap-smear+1]
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

    // r[verify cz.craft.period-derivative-test+1]
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

    // r[verify cz.craft.period-derivative-test+1]
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

    // r[verify cz.craft.period-derivative-test+1]
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

// r[verify cz.craft.period-derivative-test+1]
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

// r[verify cz.craft.period-derivative-test+1]
#[test]
fn exterior_or_wrong_period_is_not_accepted() {
    assert_eq!(verified_period((1.0, 0.0), 1), None);
    assert_eq!(verified_period((-1.0, 0.0), 1), None);
}

// r[verify cz.craft.lifo-drain+1]
#[test]
fn completion_drain_is_lifo() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        for i in 0..50usize {
            ctx.completed_points.push((CompletedPoint::Dummy {}, i));
        }
        let drained = work_update(&mut ctx);
        let order: Vec<usize> = drained.iter().map(|(_, i)| *i).collect();
        assert_eq!(order, (0..50).rev().collect::<Vec<_>>(), "freshest work must publish first");
        assert_eq!(ctx.completed_points.len(), 0);
    });
}

// r[verify cz.craft.edge-push-front+1]
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

// r[verify cz.craft.cost-metadata+1]
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

// r[verify cz.craft.scredge-first-shift0+1]
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
        assert!(ctx.completed_points.len() > 0);
        assert_eq!(ctx.completed_points[0].1, index_from_pos(&(3, 1), ctx.res.0),
            "shift 0 fallthrough must prove the motion edge first");
    });
    run_big_stack_size(|| {
        // shift 1: edge outranks scredge.
        let mut ctx = make_context(1);
        ctx.scredge_poses.push_back((3, 1));
        ctx.edge_queue.push_back(((2, 0), 0));
        shift(&mut ctx);
        assert!(ctx.completed_points.len() > 0);
        assert_eq!(ctx.completed_points[0].1, index_from_pos(&(2, 0), ctx.res.0),
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

// r[verify cz.craft.out-rotates-in-stays+1]
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

// r[verify cz.craft.provisional-not-delivered+1]
#[test]
fn provisional_answer_never_marks_delivered() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        ctx.scredge_poses.push_back((3, 1));
        shift(&mut ctx);
        assert!(ctx.completed_points.len() > 0, "scredge publishes provisional answers");
        let index = index_from_pos(&(3, 1), ctx.res.0);
        assert!(!ctx.points[index].delivered,
            "a guess must never block the truth");
        let periods = ctx.completed_points
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

// r[verify cz.craft.wait-on-channel-full+1] — unsent batch restages; delivered stays
#[test]
fn channel_full_restages_without_clearing_delivered() {
    run_big_stack_size(|| {
        let mut ctx = make_context(1);
        let idx = index_from_pos(&(2, 0), ctx.res.0);
        ctx.points[idx].delivered = true;
        ctx.points[idx].escapes = true;
        let batch = vec![(
            CompletedPoint::Escapes {
                escape_time: 1,
                escape_location: (FloatExp::ZERO, FloatExp::ZERO),
                escape_derivative: (FloatExp::ONE, FloatExp::ZERO),
                start_location: (FloatExp::ZERO, FloatExp::ZERO),
                smallness: FloatExp::ZERO,
                small_time: 0,
            },
            idx,
        )];
        super::restage_unsent_batch(&mut ctx, batch);
        assert!(
            ctx.points[idx].delivered,
            "wait-on-full must not clear delivered (Dummy holes)"
        );
        assert_eq!(
            ctx.completed_points.len(),
            1,
            "unsent answers must return to staging for the next flush"
        );
        assert_eq!(ctx.completed_points[0].1, idx);
    });
}

// r[verify cz.craft.shared-remap-transform+1]
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

// r[verify cz.craft.wall-clock-law+1]
// Structural: a workshift always terminates (the loop law is elapsed wall-clock,
// not queue state). The 10ms constant itself stays code-reviewed.
#[test]
fn workshift_always_terminates() {
    run_big_stack_size(|| {
        // no queues at all: immediate exit
        let mut ctx = make_context(0);
        let t = Instant::now();
        shift(&mut ctx);
        assert!(t.elapsed() <= UNIT_TIMEOUT);

        // queues full of slow work: bounded by the clock
        let mut ctx = make_context(2);
        ctx.out_queue.push_back(((3, 0), 0));
        let t = Instant::now();
        shift(&mut ctx);
        assert!(
            t.elapsed() <= UNIT_TIMEOUT,
            "the wall clock, not the workload, bounds a shift"
        );
        assert_eq!(ctx.workshifts, 3);
    });
}

#[test]
fn lifetime_iteration_counters_saturate_instead_of_overflow() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        ctx.total_iterations = u32::MAX;
        ctx.total_iterations_today = u32::MAX;
        ctx.total_points_today = u32::MAX;
        ctx.total_bouts_today = u32::MAX;
        ctx.workshifts = u32::MAX;
        ctx.points[2].iterations = u32::MAX;
        ctx.total_iterations = ctx
            .total_iterations
            .saturating_add(ctx.points[2].iterations);
        ctx.total_iterations_today = ctx
            .total_iterations_today
            .saturating_add(ctx.points[2].iterations);
        ctx.total_points_today = ctx.total_points_today.saturating_add(1);
        ctx.total_bouts_today = ctx.total_bouts_today.saturating_add(1);
        ctx.workshifts = ctx.workshifts.saturating_add(1);
        ctx.spent_tokens_today = ctx
            .total_bouts_today
            .saturating_mul(4)
            .saturating_add(ctx.total_points_today.saturating_mul(150))
            .saturating_add(ctx.total_iterations_today.saturating_mul(150));
        assert_eq!(ctx.total_iterations, u32::MAX);
        assert_eq!(ctx.total_iterations_today, u32::MAX);
        assert_eq!(ctx.total_points_today, u32::MAX);
        assert_eq!(ctx.total_bouts_today, u32::MAX);
        assert_eq!(ctx.workshifts, u32::MAX);
        assert_eq!(ctx.spent_tokens_today, u32::MAX);
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

// r[verify cz.craft.completion-cap-fits-screen+1]
#[test]
fn enlarge_replace_completion_vec_accepts_full_screen() {
    run_big_stack_size(|| {
        let small = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            (320u32, 240u32),
        );
        let large = (small.0.clone(), (640u32, 480u32));
        let large_n = (large.1 .0 * large.1 .1) as usize;

        let prior = from_stencil::<f64>(small.clone(), None).expect("small");
        let next = from_stencil::<f64>(large, Some((prior, small.0))).expect("enlarged");
        assert!(
            next.completed_points.capacity() >= large_n,
            "enlarged Replace must reserve room for the new screen"
        );

        let mut ctx = next;
        for i in 0..large_n {
            let _ = ctx.push_delivery(Delivery::Final(CompletedPoint::Dummy {}), i);
        }
        assert_eq!(
            ctx.completed_points.len(),
            large_n,
            "growable completion Vec must accept one Final per seat"
        );
        assert!(ctx.points.iter().all(|p| p.delivered));
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
        assert!(ctx.completed_points
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

/// Filter-friendly thought-kill for `BoutCap` clamp (`>` not `>=` / constant).
#[test]
fn mutant_kill_bout_cap_clamp() {
    bout_cap_clamps_above_max();
    // Exactly MAX_BOUT must pass through (kill `>=` → clamp MAX_BOUT+? wrong).
    assert_eq!(BoutCap::new(MAX_BOUT).get(), 1000);
    assert_ne!(BoutCap::new(999).get(), 1000);
    assert_eq!(BoutCap::new(999).get(), 999);
    assert_ne!(BoutCap::new(u32::MAX).get(), u32::MAX);
    assert_ne!(BoutCap::new(0).get(), 1);
    assert_eq!(MAX_BOUT, 1000);
}

/// Thought-killed pin: Provisional must not set `delivered`; Final does.
#[test]
fn mutant_kill_push_delivery_provisional_not_final() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        let idx = 0usize;
        ctx.points[idx].delivered = false;
        let completed = CompletedPoint::Escapes {
            escape_time: 3,
            escape_location: (FloatExp::ZERO, FloatExp::ZERO),
            escape_derivative: (FloatExp::ONE, FloatExp::ZERO),
            start_location: (FloatExp::ZERO, FloatExp::ZERO),
            smallness: FloatExp::ZERO,
            small_time: 0,
        };
        let out = ctx.push_delivery(Delivery::Provisional(completed.clone()), idx);
        assert_eq!(out, PushOutcome::Published);
        assert!(
            !ctx.points[idx].delivered,
            "provisional must not mark delivered"
        );
        let out2 = ctx.push_delivery(Delivery::Final(completed), idx);
        assert_eq!(out2, PushOutcome::Published);
        assert!(ctx.points[idx].delivered, "final must mark delivered");

        // Shutdown-interrupt path: restage keeps delivered + answers for retry.
        let batch = work_update(&mut ctx);
        assert_eq!(batch.len(), 2, "provisional + final were staged");
        super::restage_unsent_batch(&mut ctx, batch);
        assert!(
            ctx.points[idx].delivered,
            "restage must keep Final delivered (no Dummy hole)"
        );
        assert_eq!(ctx.completed_points.len(), 2);
    });
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

/// Thought-killed pins for `classify_motion` (zoom≻pan, || on pos axes, Neither).
#[test]
fn mutant_kill_classify_motion() {
    let base = ObjectivePosAndZoom {
        pos: (IntExp::from(-2), IntExp::from(2)),
        zoom_pot: -2,
    };
    assert_eq!(classify_motion(None, &base), Motion::Neither);
    assert_eq!(classify_motion(Some(&base), &base), Motion::Neither);

    let zoomed = ObjectivePosAndZoom {
        pos: base.pos.clone(),
        zoom_pot: 0,
    };
    assert_eq!(classify_motion(Some(&base), &zoomed), Motion::Zoomed);

    let panned = ObjectivePosAndZoom {
        pos: (IntExp::from(-1), IntExp::from(2)),
        zoom_pot: -2,
    };
    assert_eq!(classify_motion(Some(&base), &panned), Motion::Panned);

    let panned_im = ObjectivePosAndZoom {
        pos: (IntExp::from(-2), IntExp::from(3)),
        zoom_pot: -2,
    };
    assert_eq!(classify_motion(Some(&base), &panned_im), Motion::Panned);

    // Both changed → Zoomed (not Panned): zoom check is first.
    let both = ObjectivePosAndZoom {
        pos: (IntExp::from(0), IntExp::from(0)),
        zoom_pot: 3,
    };
    assert_eq!(classify_motion(Some(&base), &both), Motion::Zoomed);
    assert_ne!(classify_motion(Some(&base), &both), Motion::Panned);
}

fn bare_point_f64(z: (f64, f64), c: (f64, f64)) -> Point<f64> {
    Point {
        delta_c: c,
        c,
        z,
        dc: (1.0, 0.0),
        real_squared: z.0 * z.0,
        imag_squared: z.1 * z.1,
        real_imag: z.0 * z.1,
        iterations: 0,
        loop_detection_point: ((0.0, 0.0), 0),
        escapes: false,
        repeats: false,
        delivered: false,
        initialized: true,
        period: 0,
        smallness_squared: 100.0,
        small_time: 0,
        delta: None,
        direct_only: false,
        bound_zero_generation: 0,
    }
}

/// Thought-killed pins for naive step / bailout `>` / cached products / period partials.
#[test]
fn mutant_kill_bailout_iterate_period_partials() {
    // Bailout is |z|² > r² (not >=).
    let on = bare_point_f64((2.0, 0.0), (0.0, 0.0));
    assert!(!bailout_point(&on, 4.0));
    let out = bare_point_f64((2.1, 0.0), (0.0, 0.0));
    assert!(bailout_point(&out, 4.0));
    assert_ne!(bailout_point(&on, 4.0), true);

    // iterate_with_c: z' = z²+c and dc' = 2 z dc + 1.
    let mut p = bare_point_f64((0.5, 0.0), (0.1, 0.0));
    update_point_results(&mut p);
    let c = p.c;
    iterate_with_c(&mut p, c);
    // z' = 0.25 + 0.1 = 0.35; dc' = 2*0.5*1 + 1 = 2
    assert!((p.z.0 - 0.35).abs() < 1e-12, "z.re={}", p.z.0);
    assert!((p.dc.0 - 2.0).abs() < 1e-12, "dc.re={}", p.dc.0);
    assert_eq!(p.iterations, 1);
    assert_ne!(p.z.0, 0.5 + 0.5 + 0.1); // *→+ on z²
    assert_ne!(p.dc.0, 0.5 * 1.0 + 1.0); // missing 2·

    // update_point_results: products * not +; smallness records min rad.
    let mut q = bare_point_f64((3.0, 4.0), (0.0, 0.0));
    q.smallness_squared = 100.0;
    q.iterations = 7;
    update_point_results(&mut q);
    assert!((q.real_squared - 9.0).abs() < 1e-12);
    assert!((q.imag_squared - 16.0).abs() < 1e-12);
    assert!((q.real_imag - 12.0).abs() < 1e-12);
    assert!((q.smallness_squared - 25.0).abs() < 1e-12);
    assert_eq!(q.small_time, 7);
    assert_ne!(q.real_squared, 3.0 + 3.0);

    // period_partials: ascending records; c=0 records n=1 then stays.
    let (partials, _) = period_partials((0.0, 0.0), 8);
    assert_eq!(partials.first().copied(), Some(1));
    assert!(partials.windows(2).all(|w| w[0] < w[1]));
    // Exterior c=2: still gets records before escape growth.
    let (p2, _) = period_partials((2.0, 0.0), 4);
    assert!(!p2.is_empty());
    assert_eq!(p2[0], 1);
}

/// Thought-killed pins for neighbor queueing bounds / delivered skip / period 0.
#[test]
fn mutant_kill_queue_neighbors_and_verified_period_zero() {
    assert_eq!(verified_period((0.0, 0.0), 0), None);
    assert_eq!(verified_period_from((0.0, 0.0), 0, (0.0, 0.0)), None);
    assert_eq!(verified_period((0.0, 0.0), 1), Some(1));
    assert_eq!(verified_period((-1.0, 0.0), 2), Some(2));
    assert_ne!(verified_period((-1.0, 0.0), 1), Some(1));
    // Candidate multiple of true period reduces to minimal (4 → 2 at c=-1).
    assert_eq!(verified_period((-1.0, 0.0), 4), Some(2));
    // Exterior / non-attracting: no verified period.
    assert_eq!(verified_period((2.0, 0.0), 1), None);
    assert_eq!(verified_period((2.0, 0.0), 3), None);
    // period_partials for period-2 bulb includes 1 then 2.
    let (p_m1, _) = period_partials((-1.0, 0.0), 8);
    assert!(p_m1.contains(&1));
    assert!(p_m1.contains(&2));

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        let res = ctx.res;
        // Mark center delivered so it is skipped when queuing from a neighbor.
        let center = (1i32, 1i32);
        let center_i = index_from_pos(&center, res.0);
        ctx.points[center_i].delivered = true;
        ctx.points[center_i].iterations = 42;

        let mut q = std::collections::VecDeque::new();
        queue_incomplete_neighbors(&(1, 1), res, &ctx.points, &mut q);
        // From an interior seat with all neighbors incomplete except itself —
        // wait, we queue FROM (1,1)'s neighbors looking at those seats' delivered.
        // Call from (0,0): should enqueue in-bounds undelivered neighbors.
        q.clear();
        queue_incomplete_neighbors(&(0, 0), res, &ctx.points, &mut q);
        assert!(!q.is_empty());
        // Out-of-bounds seats never appear.
        assert!(q.iter().all(|(p, _)| {
            p.0 >= 0 && p.1 >= 0 && p.0 < res.0 as i32 && p.1 < res.1 as i32
        }));
        // Difficulty is source iterations.
        let src_iters = ctx.points[index_from_pos(&(0, 0), res.0)].iterations;
        assert!(q.iter().all(|(_, d)| *d == src_iters));

        // Delivered neighbor is omitted.
        q.clear();
        let right_of_center = (2i32, 1i32);
        if right_of_center.0 < res.0 as i32 {
            queue_incomplete_neighbors(&right_of_center, res, &ctx.points, &mut q);
            assert!(
                q.iter().all(|(p, _)| *p != center),
                "delivered center must not be re-queued"
            );
        }

        // ||→&& on bounds would accept negatives if only one axis checked — force corner.
        q.clear();
        queue_incomplete_neighbors(&(0, 0), res, &ctx.points, &mut q);
        assert!(!q.iter().any(|(p, _)| p.0 < 0 || p.1 < 0));
    });
}

/// Thought-killed pins: edge detection, in/edge queues, spiral geometry, admission.
#[test]
fn mutant_kill_edge_spiral_and_generator_admission() {
    // square_ring_offset: ring 0 origin; ring 1 has exactly 8 seats; Chebyshev = r.
    assert_eq!(square_ring_offset(0), (0, 0));
    let mut ring1 = Vec::new();
    for k in 1..=8u64 {
        let (dx, dy) = square_ring_offset(k);
        assert_eq!(dx.abs().max(dy.abs()), 1, "k={k}");
        ring1.push((dx, dy));
    }
    ring1.sort();
    ring1.dedup();
    assert_eq!(ring1.len(), 8, "ring 1 must visit 8 distinct offsets");
    // k=9 starts ring 2.
    let (dx, dy) = square_ring_offset(9);
    assert_eq!(dx.abs().max(dy.abs()), 2);

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        let res = ctx.res;
        let a = (1i32, 1i32);
        let b = (2i32, 1i32);
        let ia = index_from_pos(&a, res.0);
        let ib = index_from_pos(&b, res.0);

        // Escape vs repeat neighbor → edge.
        ctx.points[ia].escapes = true;
        ctx.points[ia].repeats = false;
        ctx.points[ib].escapes = false;
        ctx.points[ib].repeats = true;
        ctx.points[ib].period = 2;
        assert_eq!(point_is_edge(&a, res, &ctx.points), Some((a, b)));

        // Same membership, period 0 unknown → no filament edge.
        ctx.points[ia].escapes = false;
        ctx.points[ia].repeats = true;
        ctx.points[ia].period = 0;
        ctx.points[ib].period = 3;
        assert_eq!(point_is_edge(&a, res, &ctx.points), None);
        // Distinct nonzero periods → edge.
        ctx.points[ia].period = 2;
        assert_eq!(point_is_edge(&a, res, &ctx.points), Some((a, b)));

        // in-queue carries period (not iterations).
        ctx.points[ia].iterations = 99;
        ctx.points[ia].period = 5;
        let mut qin = VecDeque::new();
        queue_incomplete_neighbors_in(&a, res, &ctx.points, &mut qin);
        assert!(!qin.is_empty());
        assert!(qin.iter().all(|(_, d)| *d == 5));
        assert!(qin.iter().all(|(_, d)| *d != 99));

        // of_edge: difficulty from pos1 iterations; push_front ahead of existing.
        ctx.points[ia].iterations = 33;
        let mut qe = VecDeque::new();
        qe.push_back(((0, 0), 777));
        queue_incomplete_neighbors_of_edge(&a, &b, res, &ctx.points, &mut qe);
        assert_eq!(qe.back().unwrap().1, 777);
        assert!(qe.iter().any(|(_, d)| *d == 33));
        // Bounds: no negatives from horizontal edge at left.
        let left = (0i32, 1i32);
        let right = (1i32, 1i32);
        let mut ql = VecDeque::new();
        queue_incomplete_neighbors_of_edge(&left, &right, res, &ctx.points, &mut ql);
        assert!(ql.iter().all(|(p, _)| p.0 >= 0 && p.1 >= 0));

        // view_center: +half width on re, −half height on im (not flipped).
        let loc = (IntExp::from(0), IntExp::from(0));
        let vc = view_center_compute(&loc, -2, res);
        let pitch = IntExp::from(1).shift(((-2i32).saturating_add(crate::constants::PIXELS_PER_UNIT_POT)).saturating_neg());
        let expect_re = IntExp::from(0) + pitch.clone() * IntExp::from((res.0 / 2) as i32);
        let expect_im = IntExp::from(0) - pitch * IntExp::from((res.1 / 2) as i32);
        assert_eq!(vc.0, expect_re);
        assert_eq!(vc.1, expect_im);

        // Absolute vs Relative admission plumbing + undelivered invalidate.
        use crate::assemblies::workgroup::c_generator::GeneratorAdmission;
        let gen = CGenerator::<FloatExp>::new(&loc, -2, res).unwrap();
        let (_, space) = gen.origin_and_space();
        ctx.generator_generation = 1;
        ctx.points[ia].delivered = false;
        ctx.points[ia].initialized = true;
        ctx.points[ia].delta = Some(DeltaState {
            delta_z: ComplexFloatExp::new(FloatExp::from(0.25), FloatExp::ZERO),
            checkpoint: ComplexFloatExp::ZERO,
            checkpoint_n: 0,
            delta_c: ComplexFloatExp::ZERO,
            c: ComplexFloatExp::ZERO,
            dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
            generation: 1,
            gear: crate::delta_gear::ComputeGear::FloatExp,
            scale: FloatExp::ONE,
        });
        let delivered = index_from_pos(&(0, 0), res.0);
        ctx.points[delivered].delivered = true;
        ctx.points[delivered].initialized = true;
        apply_generator_admission(
            &mut ctx,
            GeneratorAdmission::Absolute(gen.clone()),
            vc.clone(),
            2,
        );
        assert!(!ctx.coords_are_relative);
        assert_eq!(ctx.coord_anchor, vc);
        assert_eq!(ctx.generator_generation, 2);
        assert!(!ctx.points[ia].initialized);
        assert!(ctx.points[ia].delta.is_none());
        assert!(ctx.points[delivered].initialized, "delivered seats keep init");
        let expect_eps = space.abs() * FloatExp::from(1.0 / 256.0);
        assert_eq!(ctx.pitch_epsilon, expect_eps);

        let anchor = (IntExp::from(3), IntExp::from(-4));
        apply_generator_admission(
            &mut ctx,
            GeneratorAdmission::Relative {
                generator: gen,
                anchor: anchor.clone(),
            },
            vc,
            2, // same generation → no wipe
        );
        assert!(ctx.coords_are_relative);
        assert_eq!(ctx.coord_anchor, anchor);
    });
}

/// Thought-killed pins: gear HUD ranks, absolute/relative C, completion branches, attention reset.
#[test]
fn mutant_kill_gear_coords_and_completion() {
    use crate::delta_gear::ComputeGear;

    // absolute_c: anchor + relative (not − / axis swap).
    let anchor = (IntExp::from(2), IntExp::from(-3));
    let rel = (FloatExp::from(0.5), FloatExp::from(0.25));
    let abs = absolute_c(rel, &anchor);
    assert!((abs.0.to_f64() - 2.5).abs() < 1e-12);
    assert!((abs.1.to_f64() - (-2.75)).abs() < 1e-12);
    assert_ne!(abs.0.to_f64(), 2.0 - 0.5);
    assert_ne!(abs.1.to_f64(), -3.0 - 0.25);

    // f64 host path + legacy alias.
    let c64 = c_from_delta_c_f64((0.5, -0.25), &anchor);
    assert!((c64.0 - 2.5).abs() < 1e-9);
    assert!((c64.1 - (-3.25)).abs() < 1e-9);
    assert_eq!(abs_c_f64((0.5, -0.25), &anchor), c64);
    let cf = c_floatexp_from_delta_c((0.5, -0.25), &anchor);
    assert!((cf.re.to_f64() - 2.5).abs() < 1e-9);
    assert!((cf.im.to_f64() - (-3.25)).abs() < 1e-9);

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        ctx.view_gear = ComputeGear::ScaledF64;
        refresh_active_gear(&mut ctx);
        assert_eq!(ctx.active_gear, ComputeGear::ScaledF64);
        // Never demote below view_gear.
        note_seat_gear(&mut ctx, ComputeGear::F64);
        assert_eq!(ctx.active_gear, ComputeGear::ScaledF64);
        // Mixed seat notes are ignored.
        note_seat_gear(&mut ctx, ComputeGear::Mixed);
        assert_eq!(ctx.active_gear, ComputeGear::ScaledF64);
        // Matching view gear is a no-op (keeps active).
        note_seat_gear(&mut ctx, ComputeGear::ScaledF64);
        assert_eq!(ctx.active_gear, ComputeGear::ScaledF64);
        // Promote then conflict → Mixed.
        ctx.view_gear = ComputeGear::F64;
        refresh_active_gear(&mut ctx);
        note_seat_gear(&mut ctx, ComputeGear::ScaledF64);
        assert_eq!(ctx.active_gear, ComputeGear::ScaledF64);
        note_seat_gear(&mut ctx, ComputeGear::FloatExp);
        assert_eq!(ctx.active_gear, ComputeGear::Mixed);

        // set_attention resets spiral only when the anchor changes.
        ctx.attention_anchor = (0, 0);
        ctx.attention_index = 99;
        ctx.attention_current = Some((0, 0));
        set_attention(&mut ctx, Some((1, 1)));
        assert_eq!(ctx.attention, Some((1, 1)));
        assert_eq!(ctx.attention_anchor, (1, 1));
        assert_eq!(ctx.attention_index, 0);
        assert!(ctx.attention_current.is_none());
        ctx.attention_index = 5;
        set_attention(&mut ctx, Some((1, 1)));
        assert_eq!(ctx.attention_index, 5, "same anchor must not reset spiral");
        set_attention(&mut ctx, None);
        assert_eq!(ctx.attention, None);
        assert_eq!(
            ctx.attention_anchor,
            ((ctx.res.0 / 2) as i32, (ctx.res.1 / 2) as i32)
        );

        // Escapes completion uses iterations + start c (not z-as-start).
        let mut esc = make_point(ESC);
        esc.escapes = true;
        esc.repeats = false;
        esc.iterations = 7;
        esc.z = (FloatExp::from(10.0), FloatExp::from(0.0));
        esc.dc = (FloatExp::ONE, FloatExp::ZERO);
        match direct_completion(&mut esc) {
            CompletedPoint::Escapes {
                escape_time,
                escape_location,
                start_location,
                ..
            } => {
                assert_eq!(escape_time, 7);
                assert!((escape_location.0.to_f64() - 10.0).abs() < 1e-12);
                assert_eq!(start_location.0, esc.c.0);
                assert_eq!(start_location.1, esc.c.1);
            }
            other => panic!("expected Escapes, got {other:?}"),
        }
        // Override c for with_c path.
        let alt = (FloatExp::from(9.0), FloatExp::from(8.0));
        match direct_completion_with_c(&mut esc, alt) {
            CompletedPoint::Escapes { start_location, .. } => {
                assert_eq!(start_location.0, alt.0);
                assert_eq!(start_location.1, alt.1);
            }
            other => panic!("expected Escapes with override c, got {other:?}"),
        }

        // Repeats → verified period (c=0 → period 1).
        let mut rep = make_point((0.0, 0.0));
        rep.repeats = true;
        rep.escapes = false;
        rep.iterations = 32;
        match direct_completion(&mut rep) {
            CompletedPoint::Repeats { period, .. } => {
                assert_eq!(period, 1);
                assert_eq!(rep.period, 1);
            }
            other => panic!("expected Repeats, got {other:?}"),
        }

        // Relative vs absolute seat C.
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(2)),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
        );
        let mut f64_ctx = from_stencil::<f64>(frame, None).expect("stencil");
        f64_ctx.coords_are_relative = false;
        assert_eq!(c_for_seat_f64(&f64_ctx, (1.25, -0.5)), (1.25, -0.5));
        f64_ctx.coords_are_relative = true;
        f64_ctx.coord_anchor = (IntExp::from(10), IntExp::from(20));
        let seat = c_for_seat_f64(&f64_ctx, (0.5, -0.25));
        let expect = c_from_delta_c_f64((0.5, -0.25), &f64_ctx.coord_anchor);
        assert!((seat.0 - expect.0).abs() < 1e-9);
        assert!((seat.1 - expect.1).abs() < 1e-9);
        assert_ne!(seat, (0.5, -0.25));
    });
}

/// Thought-killed pins: pitch epsilon, bout loop/escape, spiral walk, smallness.
#[test]
fn mutant_kill_pitch_loop_spiral_and_bout() {
    // pitch = |Δδc| / 256 (abs + scale; not missing abs / wrong factor).
    let pts = vec![make_point((0.0, 0.0)), make_point((0.25, 0.0))];
    let e = pitch_epsilon(&pts);
    assert_eq!(e, FloatExp::from(0.25) * FloatExp::from(1.0 / 256.0));
    let flipped = vec![make_point((0.25, 0.0)), make_point((0.0, 0.0))];
    assert_eq!(pitch_epsilon(&flipped), e, "abs must ignore seat order");
    assert_ne!(e, FloatExp::from(0.25));
    assert_ne!(e, FloatExp::from(0.25) * FloatExp::from(256.0));
    assert_ne!(e, FloatExp::ZERO);

    // Interior c=0: loop_check fires; period = iterations − checkpoint n.
    let mut interior = make_point((0.0, 0.0));
    iterate_max_n_times(
        &mut interior,
        FloatExp::from(4.0),
        FloatExp::from(1e-20),
        BoutCap::new(16),
    );
    assert!(interior.repeats, "0+0i must repeat");
    assert!(!interior.escapes);
    assert!(
        interior.period >= 1,
        "loop_check must record a positive period, got {}",
        interior.period
    );
    // period is set as iterations−checkpoint *before* update_loop_check may advance
    // the checkpoint to the current iterate — do not re-derive against the final pair.

    // Far exterior already |z|²>r² before any step: escapes without iterating.
    let mut exterior = make_point(ESC);
    iterate_max_n_times(
        &mut exterior,
        FloatExp::from(4.0),
        FloatExp::from(1e-20),
        BoutCap::new(8),
    );
    assert!(exterior.escapes);
    assert!(!exterior.repeats);
    assert_eq!(exterior.iterations, 0, "pre-escaped z must not count fake steps");

    // Escape after at least one iterate: start inside bailout, c pushes out.
    let mut escapes_after = make_point((0.0, 0.0));
    escapes_after.c = (FloatExp::from(2.0), FloatExp::from(2.0));
    escapes_after.z = (FloatExp::from(0.0), FloatExp::from(0.0));
    escapes_after.delta_c = escapes_after.c;
    iterate_max_n_times(
        &mut escapes_after,
        FloatExp::from(4.0),
        FloatExp::from(1e-20),
        BoutCap::new(8),
    );
    assert!(escapes_after.escapes);
    assert!(escapes_after.iterations >= 1);

    // BoutCap hard-stops unfinished orbits (exact step count when no conclusion).
    let mut capped = make_point((0.1, 0.2));
    let start_iters = capped.iterations;
    iterate_max_n_times(
        &mut capped,
        FloatExp::from(4.0),
        FloatExp::from(1e-30),
        BoutCap::new(5),
    );
    assert!(
        !capped.escapes && !capped.repeats,
        "shallow exterior bout of 5 should stay unfinished"
    );
    assert_eq!(capped.iterations, start_iters + 5);

    // update_point_results: smallness only improves (strict <).
    let mut p = make_point((0.0, 0.0));
    p.z = (FloatExp::from(3.0), FloatExp::from(4.0));
    p.iterations = 3;
    p.smallness_squared = FloatExp::from(100.0);
    update_point_results(&mut p);
    assert_eq!(p.real_squared, FloatExp::from(9.0));
    assert_eq!(p.imag_squared, FloatExp::from(16.0));
    assert_eq!(p.real_imag, FloatExp::from(12.0));
    assert_eq!(p.smallness_squared, FloatExp::from(25.0));
    assert_eq!(p.small_time, 3);
    p.z = (FloatExp::from(10.0), FloatExp::ZERO);
    p.iterations = 9;
    update_point_results(&mut p);
    assert_eq!(
        p.smallness_squared,
        FloatExp::from(25.0),
        "larger rad must not overwrite smallness"
    );
    assert_eq!(p.small_time, 3);

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        set_attention(&mut ctx, Some((2, 2)));
        ctx.attention_index = 0;
        let first = next_attention_spiral_pos(&mut ctx).expect("anchor");
        assert_eq!(first, (2, 2));
        assert_eq!(ctx.attention_index, 1);
        let second = next_attention_spiral_pos(&mut ctx).expect("ring1");
        let (dx, dy) = (second.0 - 2, second.1 - 2);
        assert_eq!(dx.abs().max(dy.abs()), 1, "first ring seat after origin");
        // Delivered seats are skipped.
        let idx = index_from_pos(&second, ctx.res.0);
        ctx.points[idx].delivered = true;
        ctx.attention_index = 1;
        let again = next_attention_spiral_pos(&mut ctx).expect("skip delivered");
        assert_ne!(again, second);
    });
}

/// Thought-killed pins: dispatch priority, PPS probe lock, stale delivery invalidate.
#[test]
fn mutant_kill_dispatch_pps_and_stale_invalidate() {
    use crate::assemblies::structs::KernelMode;

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        // Default absolute → Naive.
        assert_eq!(ctx.dispatch_kernel(true), KernelMode::Naive);
        // Probe queue head wins over Naive.
        ctx.pps_probe_queue = vec![KernelMode::NaiveGpu, KernelMode::Pert];
        assert_eq!(ctx.dispatch_kernel(true), KernelMode::NaiveGpu);
        // Locked overrides probe queue.
        ctx.pps_locked_kernel = Some(KernelMode::Pert);
        assert_eq!(ctx.dispatch_kernel(true), KernelMode::Pert);
        // Soft trial floor → Pert even with locked cleared.
        ctx.pps_locked_kernel = None;
        ctx.pps_probe_queue.clear();
        ctx.reference_floor_active = true;
        assert_eq!(ctx.dispatch_kernel(false), KernelMode::Pert);
        // Relative shell always Pert (beats NaiveGpu probe).
        ctx.reference_floor_active = false;
        ctx.coords_are_relative = true;
        ctx.pps_probe_queue = vec![KernelMode::NaiveGpu];
        assert_eq!(ctx.dispatch_kernel(true), KernelMode::Pert);
        // Manual beats everything.
        ctx.manual_gear = Some(KernelMode::Naive);
        assert_eq!(ctx.dispatch_kernel(true), KernelMode::Naive);
        ctx.manual_gear = None;

        // Relative ensure_pps_probe forces Pert lock and clears race queue.
        ctx.coords_are_relative = true;
        ctx.pps_locked_kernel = None;
        ctx.pps_probe_queue = vec![KernelMode::Naive];
        ctx.ensure_pps_probe(true);
        assert_eq!(ctx.pps_locked_kernel, Some(KernelMode::Pert));
        assert!(ctx.pps_probe_queue.is_empty());
        // Manual skips probe setup.
        ctx.coords_are_relative = false;
        ctx.manual_gear = Some(KernelMode::Naive);
        ctx.pps_locked_kernel = None;
        ctx.pps_probe_queue.clear();
        ctx.ensure_pps_probe(true);
        assert!(ctx.pps_locked_kernel.is_none());
        assert!(ctx.pps_probe_queue.is_empty());
        ctx.manual_gear = None;
        // Absolute cold start fills legal queue.
        ctx.ensure_pps_probe(true);
        assert!(!ctx.pps_probe_queue.is_empty());
        assert_eq!(
            ctx.pps_probe_shifts_left,
            crate::gearbox::PPS_PROBE_SHIFTS_PER_CANDIDATE
        );

        // tick_pert_trial: inactive → None; countdown expire ends trial.
        assert_eq!(ctx.tick_pert_trial(), None);
        ctx.reference_floor_active = true;
        ctx.pert_trial_shifts_left = 1;
        assert_eq!(ctx.tick_pert_trial(), Some("trial_expired"));
        assert!(!ctx.reference_floor_active);
        assert_eq!(ctx.pert_trial_shifts_left, 0);

        // invalidate_stale_deliveries: matching generation kept; mismatch / None wiped.
        let i_keep = index_from_pos(&(0, 0), ctx.res.0);
        let i_stale = index_from_pos(&(1, 0), ctx.res.0);
        let i_none = index_from_pos(&(2, 0), ctx.res.0);
        for i in [i_keep, i_stale, i_none] {
            ctx.points[i].delivered = true;
            ctx.points[i].initialized = true;
        }
        ctx.points[i_keep].delta = Some(DeltaState {
            delta_z: ComplexFloatExp::ZERO,
            checkpoint: ComplexFloatExp::ZERO,
            checkpoint_n: 0,
            delta_c: ComplexFloatExp::ZERO,
            c: ComplexFloatExp::ZERO,
            dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
            generation: 7,
            gear: crate::delta_gear::ComputeGear::FloatExp,
            scale: FloatExp::ONE,
        });
        ctx.points[i_stale].delta = Some(DeltaState {
            delta_z: ComplexFloatExp::ZERO,
            checkpoint: ComplexFloatExp::ZERO,
            checkpoint_n: 0,
            delta_c: ComplexFloatExp::ZERO,
            c: ComplexFloatExp::ZERO,
            dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
            generation: 3,
            gear: crate::delta_gear::ComputeGear::FloatExp,
            scale: FloatExp::ONE,
        });
        ctx.points[i_none].delta = None;
        invalidate_stale_deliveries(&mut ctx, 7);
        assert!(ctx.points[i_keep].delivered);
        assert!(ctx.points[i_keep].initialized);
        assert!(ctx.points[i_keep].delta.is_some());
        assert!(!ctx.points[i_stale].delivered);
        assert!(!ctx.points[i_stale].initialized);
        assert!(ctx.points[i_stale].delta.is_none());
        assert!(!ctx.points[i_none].delivered);
        assert!(ctx.points[i_none].delta.is_none());
        // Undelivered seats are not touched.
        let i_open = index_from_pos(&(3, 0), ctx.res.0);
        ctx.points[i_open].delivered = false;
        ctx.points[i_open].initialized = true;
        invalidate_stale_deliveries(&mut ctx, 99);
        assert!(ctx.points[i_open].initialized);
    });
}

/// Thought-killed pins: f64 stencil admit, placeholder defaults, HUD PPS window.
#[test]
fn mutant_kill_stencil_admit_and_hud_pps() {
    let homeish = (IntExp::from(-2), IntExp::from(1));
    assert!(f64_stencil_admits(&homeish, -2, TEST_SCREEN_RES));
    // Deep pots still admit via relative-to-center fallback (not absolute-only).
    assert!(f64_stencil_admits(&homeish, 80, TEST_SCREEN_RES));
    // Degenerate 0-width would not make a grid — use tiny but valid res.
    assert!(f64_stencil_admits(&homeish, -2, (2, 2)));

    let ph = placeholder_point::<f64>();
    assert!(!ph.delivered);
    assert!(!ph.initialized);
    assert!(!ph.escapes);
    assert!(!ph.repeats);
    assert_eq!(ph.iterations, 0);
    assert_eq!(ph.period, 0);
    assert!(ph.delta.is_none());
    assert_eq!(ph.dc, (1.0, 0.0));

    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        ctx.hud_points_window = 0;
        ctx.hud_window_started = Instant::now();
        ctx.record_hud_completion_batch(10);
        assert_eq!(ctx.hud_points_window, 10);
        ctx.record_hud_completion_batch(5);
        assert_eq!(ctx.hud_points_window, 15);
        let pps = ctx.hud_pps_estimate();
        assert!(pps > 0.0);
        // screen_point_count is width*height (not +).
        assert_eq!(
            ctx.screen_point_count(),
            (TEST_SCREEN_RES.0 as u64) * (TEST_SCREEN_RES.1 as u64)
        );
        assert_ne!(
            ctx.screen_point_count(),
            (TEST_SCREEN_RES.0 as u64) + (TEST_SCREEN_RES.1 as u64)
        );
        // floor_policy without ref → no_ref.
        ctx.latest_reference = None;
        ctx.reference_floor_active = false;
        ctx.pert_trial_cooldown = 0;
        assert_eq!(ctx.floor_policy_label(), "no_ref");
        ctx.pert_trial_cooldown = 3;
        assert_eq!(ctx.floor_policy_label(), "cooldown");
        ctx.pert_trial_cooldown = 0;
        ctx.reference_floor_active = true;
        assert_eq!(ctx.floor_policy_label(), "trial_active");
        assert!(ctx.perturbation_kernel_required());
    });
}

/// Thought-killed pins: queue fallback priority (scredge vs edge/out/in).
#[test]
fn mutant_kill_queue_fallback_priority() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        // Empty → None.
        assert!(queue_fallback_pos_pub(&ctx, true).is_none());
        assert!(queue_fallback_pos_pub(&ctx, false).is_none());

        ctx.in_queue.push_back(((0, 0), 1));
        ctx.out_queue.push_back(((1, 0), 2));
        ctx.edge_queue.push_back(((2, 0), 3));
        ctx.scredge_poses.push_back((3, 1));

        // prefer_scredge: Scredge beats Edge.
        assert_eq!(
            queue_fallback_pos_pub(&ctx, true),
            Some(((3, 1), Step::Scredge))
        );
        // !prefer_scredge: Edge before Out before Scredge before In.
        assert_eq!(
            queue_fallback_pos_pub(&ctx, false),
            Some(((2, 0), Step::Edge))
        );
        ctx.edge_queue.clear();
        assert_eq!(
            queue_fallback_pos_pub(&ctx, false),
            Some(((1, 0), Step::Out))
        );
        ctx.out_queue.clear();
        assert_eq!(
            queue_fallback_pos_pub(&ctx, false),
            Some(((3, 1), Step::Scredge))
        );
        ctx.scredge_poses.clear();
        assert_eq!(
            queue_fallback_pos_pub(&ctx, false),
            Some(((0, 0), Step::In))
        );
        // prefer_scredge with empty scredge falls through to Edge/Out/In.
        ctx.scredge_poses.clear();
        ctx.edge_queue.push_back(((2, 0), 3));
        assert_eq!(
            queue_fallback_pos_pub(&ctx, true),
            Some(((2, 0), Step::Edge))
        );
    });
}

/// Thought-killed pin: idle metric is delivered fraction × 100, not raw fraction.
#[test]
fn mutant_kill_percent_completed_is_percent_scale() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        let total = ctx.points.len();
        for p in &mut ctx.points {
            p.delivered = false;
        }
        let half = total / 2;
        for i in 0..half {
            ctx.points[i].delivered = true;
        }
        // Empty queues + disabled spiral → shift should not start new seats.
        assert!(ctx.scredge_poses.is_empty());
        assert!(ctx.edge_queue.is_empty());
        assert!(ctx.out_queue.is_empty());
        assert!(ctx.in_queue.is_empty());
        assert_eq!(ctx.attention_index, u64::MAX);
        shift(&mut ctx);
        let expected = half as f64 / total as f64 * 100.0;
        assert!(
            (ctx.percent_completed - expected).abs() < 1e-9,
            "got {} want {}",
            ctx.percent_completed,
            expected
        );
        // *→/ or omit ×100 leaves a tiny fraction; /→* explodes.
        assert!(ctx.percent_completed > 10.0);
        assert!(ctx.percent_completed < 100.0);
        assert_ne!(ctx.percent_completed, half as f64 / total as f64);
    });
}

/// Park predicate: once every seat is delivered, the actor must not keep
/// chaining workshifts (load-proportional-ignorance).
#[test]
fn mutant_kill_complete_frame_has_no_undelivered_seats() {
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        for p in &mut ctx.points {
            p.delivered = true;
        }
        let needs_work = ctx.points.iter().any(|p| !p.delivered);
        assert!(!needs_work);
        shift(&mut ctx);
        assert!(
            ctx.percent_completed >= 100.0,
            "all-delivered must report complete, got {}",
            ctx.percent_completed
        );
        assert!(!ctx.points.iter().any(|p| !p.delivered));
        // One undelivered seat must keep the worker busy.
        ctx.points[0].delivered = false;
        assert!(ctx.points.iter().any(|p| !p.delivered));
    });
}

/// Thought-killed pins: LIFO completion drain and struggling_to_clear 2s gate.
#[test]
fn mutant_kill_completion_lifo_and_struggling_to_clear() {
    // Per-shift Vec drained LIFO (newest first) — same order Stec pop had.
    run_big_stack_size(|| {
        let mut ctx = make_context(0);
        ctx.completed_points.push((CompletedPoint::Dummy {}, 1));
        ctx.completed_points.push((CompletedPoint::Dummy {}, 2));
        ctx.completed_points.push((CompletedPoint::Dummy {}, 3));
        let drained = work_update(&mut ctx);
        assert_eq!(drained.iter().map(|(_, i)| *i).collect::<Vec<_>>(), vec![3, 2, 1]);
        assert!(ctx.completed_points.is_empty());
    });

    run_big_stack_size(|| {
        let ctx = make_context(0);
        assert!(!ctx.struggling_to_clear_pub(0, 100.0));
        assert!(!ctx.struggling_to_clear_pub(100, 0.99));
        assert!(!ctx.struggling_to_clear_pub(100, 0.0));
        assert!(!ctx.struggling_to_clear_pub(200, 100.0)); // 2.0s
        assert!(ctx.struggling_to_clear_pub(201, 100.0)); // >2.0s
        assert!(ctx.struggling_to_clear_pub(100, 1.0)); // 100s
        assert_ne!(ctx.struggling_to_clear_pub(200, 100.0), true);
        assert_ne!(ctx.struggling_to_clear_pub(0, 1e9), true);
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

#[test]
fn unit_timeout_actually_fires() {
    let t0 = std::time::Instant::now();
    let hit = std::panic::catch_unwind(|| {
        run_big_stack_size(|| {
            std::thread::sleep(std::time::Duration::from_secs(3));
        });
    });
    assert!(hit.is_err(), "1s unit timeout must panic a hung body");
    let elapsed = t0.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "timeout must fire near 1s, not hang ({elapsed:?})"
    );
}

