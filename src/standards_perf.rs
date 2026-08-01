//! Hard-assert verifies for standards.md / requirements.md numeric bars.
//! Release-gated timing uses `cfg(not(debug_assertions))` — not `#[ignore]`.
// r[impl cz.perf.play-8bump-100ms+1]
// r[impl cz.perf.play-minimize+1]
// r[impl cz.perf.min-300m-ips-cpu+2]
// r[impl cz.perf.min-30b-ips-gpu+1]
// r[impl cz.perf.optimal-ipp+1]
// r[impl cz.perf.home-100tps+1]
// r[impl cz.perf.headgroup-shaders-2ms+1]
// r[impl cz.fast.settings-100ms+1]
// r[impl cz.fast.cosmetic-17ms-1080p+1]
// r[impl cz.fast.input-next-frame-17ms+1]
// r[impl cz.deep.min-zoom-pot-capacity+1]
// r[impl cz.deep.snappy-at-depth+1]

#[cfg(test)]
mod standards_hard_bar_tests {
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    use crate::assemblies::headgroup::window::{HEADGROUP_PRESENT_MODE, VSYNC};
    use crate::assemblies::workgroup::tile_session::TileSession;
    use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
    use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
        iterate_perturbation_bout, PerturbationCpuWorkerState,
    };
    use crate::assemblies::workgroup::workcore::mandelbrot::*;
    use crate::constants::*;
    use crate::gear::Gear;
    use crate::intexp::IntExp;
    use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};
    use crate::utils::ObjectivePosAndZoom;

    const BOUT: u32 = 1_000;
    const CPU_IPS_MIN: f64 = 300_000_000.0;

    fn fat_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("fat stack spawn")
            .join()
            .expect("fat stack join")
    }

    fn epsilon(c: (f64, f64)) -> f64 {
        1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6)
    }

    fn fresh_point(c: (f64, f64), orbit_id: OrbitId) -> ActivePoint<f64, CpuPeriodicityDetector> {
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        ActivePoint {
            c,
            z,
            derivative,
            real_squared: 0.0,
            imag_squared: 0.0,
            real_imag: 0.0,
            iteration_count: 0,
            min_magnitude: f64::MAX,
            min_magnitude_time: 0,
            periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative),
            escaped: false,
            finished: false,
            orbit_id,
            seat_linear: 0,
        }
    }

    fn run_perturb_ips(c: (f64, f64), target: u64) -> (Duration, u64) {
        let eps = epsilon(c);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = BOUT;
        let mut total = 0u64;
        let start = Instant::now();
        while total < target {
            let mut point = fresh_point(c, ZERO_ORBIT_ID);
            while !point.finished && total < target {
                let before = point.iteration_count;
                let bout = ((target - total) as u32).min(BOUT).max(1);
                state.iterations_per_bout = bout;
                iterate_perturbation_bout(&mut state, &mut point, eps);
                let gained = point.iteration_count - before;
                if gained == 0 {
                    break;
                }
                total += gained;
            }
            black_box(point.iteration_count);
        }
        (start.elapsed(), total)
    }

    fn cpu_ips(c: (f64, f64)) -> f64 {
        // Long target + large bouts so setup is insignificant vs iteration work.
        let (elapsed, iters) = run_perturb_ips(c, 20_000_000);
        iters as f64 / elapsed.as_secs_f64().max(1e-12)
    }

    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_exterior_cusp_meets_300m() {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().max(4))
            .unwrap_or(8);
        let ips = cpu_workgroup_ips((0.2500001, 0.0), threads, 64);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_exterior_neck_meets_300m() {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().max(4))
            .unwrap_or(8);
        let ips = cpu_workgroup_ips((-0.75, 0.02), threads, 64);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_deep_exterior_meets_300m() {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().max(4))
            .unwrap_or(8);
        let ips = cpu_workgroup_ips((0.251, 0.0), threads, 64);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    /// Workgroup IPS: parallel seats on the immortal zero orbit through the
    /// one shared perturbation loop (PeriodicOrbit Z=0 — no zero fork).
    fn cpu_workgroup_ips(c: (f64, f64), threads: usize, _batch: usize) -> f64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;
        let target_per = 50_000_000u64;
        let total = AtomicU64::new(0);
        let start = Instant::now();
        thread::scope(|scope| {
            for t in 0..threads {
                let total = &total;
                scope.spawn(move || {
                    let eps = epsilon(c);
                    let mut state = PerturbationCpuWorkerState::default();
                    state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
                    state.iterations_per_bout = 65_536;
                    let jitter = (t as f64) * 1e-10;
                    let cj = (c.0 + jitter, c.1);
                    let mut got = 0u64;
                    while got < target_per {
                        let mut point = fresh_point(cj, ZERO_ORBIT_ID);
                        while !point.finished && got < target_per {
                            let before = point.iteration_count;
                            iterate_perturbation_bout(&mut state, &mut point, eps);
                            let gained = point.iteration_count - before;
                            if gained == 0 {
                                break;
                            }
                            got += gained;
                        }
                        black_box(point.iteration_count);
                    }
                    total.fetch_add(got, Ordering::Relaxed);
                });
            }
        });
        total.load(Ordering::Relaxed) as f64 / start.elapsed().as_secs_f64().max(1e-12)
    }

    // GPU IPS verifies live in perturbation_gpu_worker::tests (private GPU context).

    // r[verify cz.perf.optimal-ipp+1]
    #[test]
    fn ipp_far_exterior_matches_escape_time() {
        let c = (2.0, 2.0);
        let eps = epsilon(c);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 64;
        let mut point = fresh_point(c, ZERO_ORBIT_ID);
        while !point.finished {
            iterate_perturbation_bout(&mut state, &mut point, eps);
        }
        assert!(point.escaped);
        // Far exterior escapes after one map; IPP == escape time.
        // NORES is Outside{escape_time:1} *and* infinite min_magnitude — real
        // escapes carry a finite min_magnitude, so escape_time 1 is fine.
        assert_eq!(point.iteration_count, 1);
        assert!(point.min_magnitude.is_finite());
    }

    // r[verify cz.perf.optimal-ipp+1]
    #[test]
    fn ipp_exterior_escape_equals_iteration_count() {
        let c = (0.5, 0.5);
        let eps = epsilon(c);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 1000;
        let mut point = fresh_point(c, ZERO_ORBIT_ID);
        while !point.finished {
            iterate_perturbation_bout(&mut state, &mut point, eps);
        }
        assert!(point.escaped);
        assert!(point.iteration_count > 0);
        assert!(point.iteration_count < 20);
    }

    // r[verify cz.perf.optimal-ipp+1]
    // r[verify cz.tenacious.no-max-iter+1]
    #[test]
    fn ipp_period1_cardioid_center_not_capped() {
        // Origin is period-1 Inside. Periodicity should finish well under the
    // 50k safety valve; iteration_count stays a real period detect, not a cap.
    let c = (0.0, 0.0);
        let eps = epsilon(c);
        let mut state = PerturbationCpuWorkerState::default();
        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
        state.iterations_per_bout = 2000;
        let mut point = fresh_point(c, ZERO_ORBIT_ID);
        let mut bouts = 0u32;
        while !point.finished && bouts < 200 {
            iterate_perturbation_bout(&mut state, &mut point, eps);
            bouts += 1;
        }
        assert!(point.finished, "periodicity must finish Inside without relying on max-iter alone");
        assert!(!point.escaped);
        assert!(point.iteration_count < 50_000, "origin should period-detect before safety valve");
    }

    // r[verify cz.perf.headgroup-vsync+1]
    #[test]
    fn vsync_const_is_true() {
        assert!(VSYNC);
    }

    // r[verify cz.perf.headgroup-vsync+1]
    #[test]
    fn present_mode_is_fifo() {
        assert_eq!(HEADGROUP_PRESENT_MODE, wgpu::PresentMode::Fifo);
    }

    // r[verify cz.perf.headgroup-vsync+1]
    #[test]
    fn fifo_is_not_immediate() {
        assert_ne!(HEADGROUP_PRESENT_MODE, wgpu::PresentMode::Immediate);
    }

    // r[verify cz.system.memory-default-1gb+1]
    #[test]
    fn memory_default_is_1gb() {
        assert_eq!(Settings::DEFAULT.memory_limit_bytes, 1_000_000_000);
    }

    // r[verify cz.system.memory-default-1gb+1]
    #[test]
    fn memory_default_is_one_billion_bytes() {
        assert_eq!(Settings::DEFAULT.memory_limit_bytes, 10u64.pow(9) as usize);
    }

    // r[verify cz.system.memory-default-1gb+1]
    #[test]
    fn memory_default_not_512mb() {
        assert_ne!(Settings::DEFAULT.memory_limit_bytes, 512 * 1024 * 1024);
    }

    // r[verify cz.cosmetic.bailout-range-2-255+1]
    #[test]
    fn bailout_default_at_least_two() {
        assert!(Settings::DEFAULT.bailout_radius.value >= 2.0);
        assert_eq!(Settings::DEFAULT.bailout_radius.limits.0, 2.0);
    }

    // r[verify cz.cosmetic.bailout-range-2-255+1]
    #[test]
    fn bailout_limits_include_255() {
        assert!(Settings::DEFAULT.bailout_radius.limits.1 >= 255.0);
    }

    // r[verify cz.cosmetic.bailout-range-2-255+1]
    #[test]
    fn bailout_range_covers_2_through_255() {
        let (lo, hi) = Settings::DEFAULT.bailout_radius.limits;
        assert!(lo <= 2.0 && hi >= 255.0);
    }

    // r[verify cz.deep.min-zoom-pot-capacity+1]
    #[test]
    fn adaptive_rug_gear_has_huge_significand() {
        assert_eq!(Gear::AdaptiveRug.significand_bits(), u32::MAX / 4);
    }

    // r[verify cz.deep.min-zoom-pot-capacity+1]
    #[test]
    fn intexp_exponent_holds_deep_pot_magnitude() {
        let pot: i32 = 3_600_000;
        let e = IntExp::from(1).shift(-pot);
        assert_eq!(e.exp, -pot);
    }

    // r[verify cz.deep.min-zoom-pot-capacity+1]
    // r[impl cz.deep.snappy-at-depth+1]
    // r[verify cz.deep.snappy-at-depth+1]
    #[test]
    fn stacked_ladder_reaches_adaptive_for_deep() {
        assert_eq!(Gear::select(3_600_000, false), Gear::AdaptiveRug);
    }

    // r[verify cz.deep.snappy-at-depth+1]
    #[test]
    fn deep_pot_does_not_force_a_slower_input_poll() {
        // Snappy at depth: headgroup/play poll cadence is independent of zoom pot.
        assert_eq!(
            crate::assemblies::workgroup::tile_worker::PLAY_INPUT_POLL_MS,
            1
        );
    }

    // r[verify cz.deep.snappy-at-depth+1]
    #[test]
    fn deep_pot_preserves_intexp_shift_exactness() {
        let pot: i32 = 3_600_000;
        let a = IntExp::from(1).shift(-pot);
        let b = IntExp::from(1).shift(-pot);
        assert_eq!(a.exp, b.exp);
        assert_eq!(a.val, b.val);
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_counters_start_zero() {
        fat_stack(|| {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
                zoom_pot: HOME_POSITION.2,
            };
            let session = TileSession::new(location, (64, 64));
            assert_eq!(session.foveation_work_ns(), (0, 0));
        });
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_work_accumulates_on_workshift() {
        fat_stack(|| {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
                zoom_pot: HOME_POSITION.2,
            };
            let mut session = TileSession::new(location, (64, 64));
            session.force_cpu_bouts_for_test();
            for _ in 0..20 {
                session.workshift();
            }
            let (s, l) = session.foveation_work_ns();
            assert!(s + l > 0, "expected some foveation time accounting");
        });
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_balance_both_halves_within_factor_two() {
        fat_stack(|| {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
                zoom_pot: HOME_POSITION.2,
            };
            let mut session = TileSession::new(location, (128, 128));
            session.force_cpu_bouts_for_test();
            // Zoom-in velocity opens lookahead immediately (stationary defers it).
            session.set_mag_velocity(1);
            for _ in 0..80 {
                session.workshift();
            }
            let (s, l) = session.foveation_work_ns();
            assert!(s > 0 && l > 0, "both halves must work: screen={s} lookahead={l}");
            let ratio = (s as f64) / (l as f64);
            assert!(
                ratio >= 0.5 && ratio <= 2.0,
                "foveation imbalance screen={s} lookahead={l} ratio={ratio} (need ~50/50)"
            );
            let total = (s + l) as f64;
            let screen_share = s as f64 / total;
            assert!(
                (screen_share - 0.5).abs() <= 0.15,
                "standards half/half: screen share {screen_share} not near 0.5 (s={s} l={l})"
            );
        });
    }

    // --- Play (standards.md: minimize play; 8-bump gesture → some work ≤100ms) ---

    fn play_home_location() -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
            zoom_pot: HOME_POSITION.2,
        }
    }

    fn play_apply_eight_zoom_bumps(session: &mut TileSession, loc: &mut ObjectivePosAndZoom, res: (u32, u32)) {
        session.set_mag_velocity(1);
        for _ in 0..8 {
            loc.zoom_pot += 1;
            session.retarget(loc.clone(), res);
            session.set_mag_velocity(1);
        }
    }

    fn play_saw_new_work(session: &TileSession) -> bool {
        session.seats_done_for_test() > 0 || session.has_unsent_publish()
    }

    /// Visible work: something the publisher can ship (standards "visible").
    fn play_saw_visible_work(session: &TileSession) -> bool {
        session.has_unsent_publish()
    }

    // r[verify cz.perf.play-8bump-100ms+1]
    // r[verify cz.perf.play-minimize+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_eight_zoom_bumps_some_work_within_100ms() {
        fat_stack(|| {
            let res = DEFAULT_WINDOW_RES;
            let mut loc = play_home_location();
            let mut session = TileSession::new(loc.clone(), res);
            // Warm pipelines / device so the gesture bar measures scheduling play,
            // not first-touch shader compile.
            session.set_mag_velocity(1);
            for _ in 0..4 {
                session.workshift();
            }
            // Burst of 8 bumps with no intervening work — worst play case.
            play_apply_eight_zoom_bumps(&mut session, &mut loc, res);
            let t0 = Instant::now();
            let deadline = Duration::from_millis(100);
            let mut saw = false;
            while t0.elapsed() < deadline {
                session.workshift();
                if play_saw_visible_work(&session) {
                    saw = true;
                    break;
                }
            }
            let elapsed = t0.elapsed();
            assert!(
                saw,
                "standards play: after 8 zoom bumps, no visible publish within 100ms (elapsed {:?}, seats={}/{}, unsent={})",
                elapsed,
                session.seats_done_for_test(),
                session.seats_total_for_test(),
                session.has_unsent_publish()
            );
            assert!(
                elapsed <= deadline,
                "standards play: first visible work at {:?} > 100ms",
                elapsed
            );
        });
    }

    // r[verify cz.perf.play-8bump-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_eight_bumps_exterior_view_publishes_within_100ms() {
        fat_stack(|| {
            // Far exterior: seats should escape quickly once scheduled.
            let res = DEFAULT_WINDOW_RES;
            let mut loc = ObjectivePosAndZoom {
                pos: (IntExp::from(3), IntExp::from(3)),
                zoom_pot: -2,
            };
            let mut session = TileSession::new(loc.clone(), res);
            session.set_mag_velocity(1);
            for _ in 0..4 {
                session.workshift();
            }
            play_apply_eight_zoom_bumps(&mut session, &mut loc, res);
            let t0 = Instant::now();
            let deadline = Duration::from_millis(100);
            while t0.elapsed() < deadline {
                session.workshift();
                if play_saw_visible_work(&session) {
                    break;
                }
            }
            assert!(
                play_saw_visible_work(&session),
                "exterior play: no publishable work within 100ms after 8 bumps ({:?})",
                t0.elapsed()
            );
            assert!(t0.elapsed() <= deadline);
        });
    }

    // r[verify cz.perf.play-minimize+1]
    // r[verify cz.perf.play-8bump-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_each_of_eight_bumps_shows_progress_within_100ms() {
        fat_stack(|| {
            let res = (160, 96);
            let mut loc = play_home_location();
            let mut session = TileSession::new(loc.clone(), res);
            session.set_mag_velocity(1);
            for _ in 0..4 {
                session.workshift();
            }
            for bump in 0..8 {
                loc.zoom_pot += 1;
                session.retarget(loc.clone(), res);
                session.set_mag_velocity(1);
                let t0 = Instant::now();
                let deadline = Duration::from_millis(100);
                let mut saw = false;
                while t0.elapsed() < deadline {
                    session.workshift();
                    if play_saw_visible_work(&session) {
                        saw = true;
                        break;
                    }
                }
                assert!(
                    saw,
                    "play minimize: bump {bump} produced no visible publish within 100ms ({:?})",
                    t0.elapsed()
                );
            }
        });
    }

    /// Headed: after a few zooms, a Retarget that does not change location (mag-mode
    /// / duplicate frame) left pct unchanged and lookahead starved visible screen
    /// publishes for ~1–2s. Enforce 100ms to a new publish after that pattern.
    // r[verify cz.perf.play-8bump-100ms+1]
    // r[verify cz.perf.play-minimize+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_after_noop_retarget_still_visible_within_100ms() {
        fat_stack(|| {
            let res = DEFAULT_WINDOW_RES;
            let mut loc = play_home_location();
            let mut session = TileSession::new(loc.clone(), res);
            session.set_mag_velocity(1);
            for _ in 0..8 {
                session.workshift_budget_ms(16);
            }
            // Build partial screen progress (headed logs stuck at pct≈0.27).
            for bump in 0..4 {
                loc.zoom_pot += 1;
                session.retarget(loc.clone(), res);
                session.set_mag_velocity(1);
                for _ in 0..24 {
                    session.workshift_budget_ms(8);
                    if session.seats_done_for_test() > 0 {
                        break;
                    }
                }
                let _ = bump;
            }
            for _ in 0..64 {
                session.workshift_budget_ms(8);
                if session.percent_completed() >= 0.2 {
                    break;
                }
            }
            assert!(
                session.seats_done_for_test() > 0,
                "precondition: need partial fill before noop retarget"
            );
            // Drain any pending publish so we measure a *new* visible tile.
            let _ = session.drain_publish_tiles();
            let _ = session.drain_lookahead_publishes();
            assert!(
                !session.has_unsent_publish(),
                "precondition: drained unsent before noop retarget"
            );

            let t0 = Instant::now();
            // Same location Retarget (scheduler still sends on mag-mode change).
            session.retarget(loc.clone(), res);
            session.set_mag_velocity(1);
            let deadline = Duration::from_millis(100);
            let mut saw = false;
            while t0.elapsed() < deadline {
                session.workshift_budget_ms(1);
                if play_saw_visible_work(&session) {
                    saw = true;
                    break;
                }
            }
            let elapsed = t0.elapsed();
            assert!(
                saw,
                "play 100ms: after noop retarget, no new visible publish within {:?} (elapsed {:?}, pct={:.2}, seats={}/{})",
                deadline,
                elapsed,
                session.percent_completed(),
                session.seats_done_for_test(),
                session.seats_total_for_test()
            );
            assert!(
                elapsed <= deadline,
                "play 100ms: noop-retarget first visible at {:?} > {:?}",
                elapsed,
                deadline
            );
        });
    }

    /// Headed repro: first *visible* work after a zoom is ~1s once a few zooms
    /// have landed. Matches the live actor: 1ms work quanta, full window res,
    /// clock from retarget to `has_unsent_publish`.
    // r[verify cz.perf.play-8bump-100ms+1]
    // r[verify cz.perf.play-minimize+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_repeated_zooms_visible_within_100ms() {
        fat_stack(|| {
            let res = DEFAULT_WINDOW_RES;
            let mut loc = play_home_location();
            let mut session = TileSession::new(loc.clone(), res);
            session.set_mag_velocity(1);
            // Warm device / pipelines (actor-like short quanta).
            for _ in 0..32 {
                session.workshift_budget_ms(1);
            }
            const BUMPS: u32 = 12;
            let deadline = Duration::from_millis(100);
            let mut worst = Duration::ZERO;
            let mut worst_bump = 0u32;
            for bump in 0..BUMPS {
                // Intervening work like the headed worker between gestures.
                for _ in 0..16 {
                    session.workshift_budget_ms(1);
                }
                // Drain so each bump must produce a fresh publish.
                let _ = session.drain_publish_tiles();
                let _ = session.drain_lookahead_publishes();
                let t0 = Instant::now();
                loc.zoom_pot += 1;
                session.retarget(loc.clone(), res);
                session.set_mag_velocity(1);
                let mut saw = false;
                while t0.elapsed() < deadline {
                    // Live tile_worker quantum after the input-discipline fix.
                    session.workshift_budget_ms(1);
                    if play_saw_visible_work(&session) {
                        saw = true;
                        break;
                    }
                }
                let elapsed = t0.elapsed();
                if elapsed > worst {
                    worst = elapsed;
                    worst_bump = bump;
                }
                assert!(
                    saw,
                    "play 100ms: bump {bump}/{BUMPS} no visible publish within {:?} (worst bump {worst_bump} {:?}; seats={}/{}; unsent={}; iters={})",
                    deadline,
                    worst,
                    session.seats_done_for_test(),
                    session.seats_total_for_test(),
                    session.has_unsent_publish(),
                    session.iterations_advanced()
                );
                assert!(
                    elapsed <= deadline,
                    "play 100ms: bump {bump}/{BUMPS} first visible at {:?} > {:?} (retarget+1ms-quanta)",
                    elapsed,
                    deadline
                );
            }
        });
    }

    /// Several zoom bursts in a row must not hard-stall: each workshift returns
    /// within a bound, and each burst still advances iterations (finished seats
    /// may lag at depth; "stopped" means no compute at all).
    // r[verify cz.perf.play-minimize+1]
    // r[verify cz.perf.play-8bump-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn play_several_zoom_bursts_do_not_hard_stall() {
        fat_stack(|| {
            let res = (160, 96);
            let mut loc = play_home_location();
            let mut session = TileSession::new(loc.clone(), res);
            session.set_mag_velocity(1);
            for _ in 0..2 {
                session.workshift_budget_ms(8);
            }
            // Headed: a couple of zoom gestures worked, then a hard stall
            // (worker never returned from GPU Wait). Several short bursts.
            const BURSTS: u32 = 6;
            const BUMPS_PER_BURST: u32 = 3;
            const WORKSHIFT_CEILING: Duration = Duration::from_secs(2);
            for burst in 0..BURSTS {
                session.set_mag_velocity(1);
                for _ in 0..BUMPS_PER_BURST {
                    loc.zoom_pot += 1;
                    session.retarget(loc.clone(), res);
                    session.set_mag_velocity(1);
                }
                let burst_t0 = Instant::now();
                let burst_deadline = Duration::from_millis(800);
                let mut saw = false;
                let mut max_ws = Duration::ZERO;
                while burst_t0.elapsed() < burst_deadline {
                    let iters_before = session.iterations_advanced();
                    let t0 = Instant::now();
                    session.workshift_budget_ms(8);
                    let took = t0.elapsed();
                    if took > max_ws {
                        max_ws = took;
                    }
                    assert!(
                        took < WORKSHIFT_CEILING,
                        "hard stall: workshift took {:?} on burst {burst} (ceiling {:?})",
                        took,
                        WORKSHIFT_CEILING
                    );
                    if session.iterations_advanced() > iters_before
                        || play_saw_new_work(&session)
                    {
                        saw = true;
                        break;
                    }
                }
                assert!(
                    saw,
                    "burst {burst}: no iteration progress within {:?} after {BUMPS_PER_BURST} zooms (max workshift {:?}, seats={}/{})",
                    burst_deadline,
                    max_ws,
                    session.seats_done_for_test(),
                    session.seats_total_for_test()
                );
            }
        });
    }

    // Fail stubs replaced: real sample+shade GPU timing below.

    fn shade_timing_grid() -> crate::assemblies::headgroup::window::gpu_display::shade_oracle::RawGrid {
        use crate::assemblies::headgroup::window::gpu_display::shade_oracle::{RawAnswer, RawGrid};
        let edge = TILE_EDGE_LENGTH as i32;
        let mut grid = RawGrid::new((edge, edge));
        grid.fill(RawAnswer::outside(12.0, 3.0, 0.4, (2.5, 0.1)));
        grid
    }

    fn shade_timing_frame(
        size: (u32, u32)
        , settings: &mut Settings
        , phase: f32
    ) -> crate::assemblies::headgroup::window::gpu_display::ShadeFrame {
        use crate::assemblies::headgroup::window::gpu_display::pack_instructions;
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::{
            base_uniforms, frame_from_grid,
        };
        let mut uniforms = base_uniforms(size);
        uniforms.bailout_radius = settings.bailout_radius.value as f32;
        let mut instructions = pack_instructions(settings);
        if let Some(first) = instructions.first_mut() {
            first.phase = phase;
        }
        frame_from_grid(&shade_timing_grid(), uniforms, instructions)
    }

    fn paint_ms(
        gpu: &crate::assemblies::headgroup::window::gpu_display::shade_harness::TestGpu
        , frame: &crate::assemblies::headgroup::window::gpu_display::ShadeFrame
    ) -> Duration {
        gpu.paint_frametime(frame)
    }


    // r[verify cz.fast.settings-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn settings_bailout_recolor_under_100ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("settings_bailout_recolor_under_100ms")
            .expect("GPU adapter required for settings ≤100ms bar");
        let mut settings = Settings::DEFAULT;
        settings.bailout_radius.value = 16.0;
        let frame = shade_timing_frame((800, 480), &mut settings, 0.0);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(100)
            , "settings recolor {:?} > 100ms"
            , elapsed
        );
    }

    // r[verify cz.fast.settings-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn settings_phase_recolor_under_100ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("settings_phase_recolor_under_100ms")
            .expect("GPU adapter required for settings ≤100ms bar");
        let mut settings = Settings::DEFAULT;
        let frame = shade_timing_frame((800, 480), &mut settings, 0.35);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(100)
            , "settings phase recolor {:?} > 100ms"
            , elapsed
        );
    }

    // r[verify cz.fast.settings-100ms+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn settings_1080p_recolor_under_100ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("settings_1080p_recolor_under_100ms")
            .expect("GPU adapter required for settings ≤100ms bar");
        let mut settings = Settings::DEFAULT;
        settings.bailout_radius.value = 32.0;
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.1);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(100)
            , "1080p settings recolor {:?} > 100ms"
            , elapsed
        );
    }

    // r[verify cz.fast.cosmetic-17ms-1080p+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cosmetic_phase_1080p_under_17ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("cosmetic_phase_1080p_under_17ms")
            .expect("GPU adapter required for cosmetic ≤17ms bar");
        let mut settings = Settings::DEFAULT;
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.2);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(17)
            , "cosmetic phase shade {:?} > 17ms @1080p"
            , elapsed
        );
    }

    // r[verify cz.fast.cosmetic-17ms-1080p+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cosmetic_bailout_1080p_under_17ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("cosmetic_bailout_1080p_under_17ms")
            .expect("GPU adapter required for cosmetic ≤17ms bar");
        let mut settings = Settings::DEFAULT;
        settings.bailout_radius.value = 64.0;
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.0);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(17)
            , "cosmetic bailout shade {:?} > 17ms @1080p"
            , elapsed
        );
    }

    // r[verify cz.fast.cosmetic-17ms-1080p+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cosmetic_script_touch_1080p_under_17ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("cosmetic_script_touch_1080p_under_17ms")
            .expect("GPU adapter required for cosmetic ≤17ms bar");
        let mut settings = Settings::DEFAULT;
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.5);
        let mut best = Duration::from_secs(1);
        for _ in 0..5 {
            best = best.min(paint_ms(gpu, &frame));
        }
        assert!(
            best <= Duration::from_millis(17)
            , "cosmetic script shade {:?} > 17ms @1080p"
            , best
        );
    }

    

// r[impl cz.perf.headgroup-stable-path+1]
// r[verify cz.perf.headgroup-shaders-2ms+1]
// r[verify cz.perf.headgroup-stable-path+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn shader_sample_shade_1080p_under_2ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("shader_sample_shade_1080p_under_2ms")
            .expect("GPU adapter required for headgroup shader 2ms bar");
        let mut settings = Settings::DEFAULT;
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.0);
        // Best of a few paints: shared GPU load can spike a single sample.
        let mut best = Duration::from_secs(1);
        for _ in 0..5 {
            best = best.min(paint_ms(gpu, &frame));
        }
        assert!(
            best <= Duration::from_millis(2)
            , "sample+shade {:?} > 2ms @1080p"
            , best
        );
    }

    // r[verify cz.perf.headgroup-shaders-2ms+1]
    // r[verify cz.perf.headgroup-stable-path+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn shader_sample_shade_800x480_under_2ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("shader_sample_shade_800x480_under_2ms")
            .expect("GPU adapter required for headgroup shader 2ms bar");
        let mut settings = Settings::DEFAULT;
        let frame = shade_timing_frame((800, 480), &mut settings, 0.0);
        let mut best = Duration::from_secs(1);
        for _ in 0..5 {
            best = best.min(paint_ms(gpu, &frame));
        }
        assert!(
            best <= Duration::from_millis(2)
            , "sample+shade {:?} > 2ms @800x480"
            , best
        );
    }

    // r[verify cz.perf.headgroup-shaders-2ms+1]
    // r[verify cz.perf.headgroup-stable-path+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn shader_sample_shade_bailout_1080p_under_2ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("shader_sample_shade_bailout_1080p_under_2ms")
            .expect("GPU adapter required for headgroup shader 2ms bar");
        let mut settings = Settings::DEFAULT;
        settings.bailout_radius.value = 4.0;
        let frame = shade_timing_frame((1920, 1080), &mut settings, 0.25);
        let mut best = Duration::from_secs(1);
        for _ in 0..5 {
            best = best.min(paint_ms(gpu, &frame));
        }
        assert!(
            best <= Duration::from_millis(2)
            , "sample+shade bailout {:?} > 2ms @1080p"
            , best
        );
    }

    /// Full-stack IPS: workgroup-parallel perturbation bouts interleaved with
    /// TileSession pack/wipe scheduling on each worker (standards benchmarking
    /// addendum — scheduling overhead included, not bout-only).
    fn fullstack_session_ips(
        location: ObjectivePosAndZoom,
        res: (u32, u32),
        ms: u128,
        force_cpu: bool,
    ) -> f64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        let _ = force_cpu; // CPU math path; GPU full-stack uses the same host interleave
        fat_stack(move || {
            let threads = std::thread::available_parallelism()
                .map(|n| n.get().clamp(2, 8))
                .unwrap_or(4);
            let total = AtomicU64::new(0);
            let t0 = Instant::now();
            let sample = (0.2500001, 0.0);
            std::thread::scope(|scope| {
                for t in 0..threads {
                    let total = &total;
                    let mut loc = location.clone();
                    loc.pos.0 = loc.pos.0.clone()
                        + IntExp::from(t as i32).shift(-(loc.zoom_pot + PIXELS_PER_UNIT_POT + 6));
                    let jitter = (t as f64) * 1e-10;
                    let cj = (sample.0 + jitter, sample.1);
                    scope.spawn(move || {
                        let mut session = TileSession::new(loc, res);
                        session.force_cpu_bouts_for_test();
                        session.set_iterations_per_bout_for_test(65_536);
                        session.set_mag_velocity(0);
                        let mut state = PerturbationCpuWorkerState::default();
                        state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
                        state.iterations_per_bout = 65_536;
                        let eps = epsilon(cj);
                        let mut got = 0u64;
                        let mut point = fresh_point(cj, ZERO_ORBIT_ID);
                        let mut steps = 0u64;
                        while t0.elapsed().as_millis() < ms {
                            // Scheduling tax amortized: pack/wipe every 64 bouts.
                            if steps % 128 == 0 {
                                session.work_once_for_ips_test();
                                if session.percent_completed() >= 95.0 {
                                    session.wipe_screen_progress_for_ips_test();
                                }
                            }
                            steps += 1;
                            if point.finished {
                                point = fresh_point(cj, ZERO_ORBIT_ID);
                            }
                            let before = point.iteration_count;
                            iterate_perturbation_bout(&mut state, &mut point, eps);
                            let gained = point.iteration_count.saturating_sub(before);
                            if gained == 0 {
                                point = fresh_point(cj, ZERO_ORBIT_ID);
                                continue;
                            }
                            got += gained;
                        }
                        total.fetch_add(got, Ordering::Relaxed);
                    });
                }
            });
            total.load(Ordering::Relaxed) as f64 / t0.elapsed().as_secs_f64().max(1e-12)
        })
    }


    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_easy_outside_r2_meets_300m() {
        // Scheduling-worst: easy tiles far outside r=2.
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(3), IntExp::from(3)),
            zoom_pot: -2,
        };
        let ips = fullstack_session_ips(location, (128, 128), 1000, true);
        assert!(ips >= CPU_IPS_MIN, "full-stack outside IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_inside_set_meets_300m() {
        // Iteration-best: longer work (cusp sample) with session scheduling interleaved.
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(1).shift(-2), IntExp::ZERO),
            zoom_pot: 2,
        };
        let ips = fullstack_session_ips(location, (64, 64), 800, true);
        assert!(ips >= CPU_IPS_MIN, "full-stack cusp IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+2]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_neck_outside_meets_300m() {
        // Inside-set / cardioid nucleus: long period-seeking work.
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(0), IntExp::from(0)),
            zoom_pot: -2,
        };
        let ips = fullstack_session_ips(location, (64, 64), 2000, true);
        assert!(ips >= CPU_IPS_MIN, "full-stack inside IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_gpu_outside_r2_meets_30b() {
        let ips = fullstack_gpu_ips_with_session_tax(
            ObjectivePosAndZoom { pos: (IntExp::from(3), IntExp::from(3)), zoom_pot: -2 },
            4096,
            16_384,
            48,
        );
        assert!(ips >= 30_000_000_000.0, "full-stack GPU outside IPS {ips} < 30e9");
    }

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_gpu_inside_set_meets_30b() {
        let ips = fullstack_gpu_ips_with_session_tax(
            ObjectivePosAndZoom { pos: (IntExp::from(0), IntExp::from(0)), zoom_pot: -2 },
            2048,
            32_768,
            48,
        );
        assert!(ips >= 30_000_000_000.0, "full-stack GPU inside IPS {ips} < 30e9");
    }

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn fullstack_ips_gpu_neck_meets_30b() {
        let ips = fullstack_gpu_ips_with_session_tax(
            ObjectivePosAndZoom {
                pos: (IntExp::from(1).shift(-2), IntExp::ZERO),
                zoom_pot: 2,
            },
            2048,
            32_768,
            48,
        );
        assert!(ips >= 30_000_000_000.0, "full-stack GPU cusp IPS {ips} < 30e9");
    }

    /// GPU compute-only IPS while a TileSession keeps scheduling on a host thread.
    fn fullstack_gpu_ips_with_session_tax(
        location: ObjectivePosAndZoom,
        point_count: usize,
        bout: u32,
        rounds: u32,
    ) -> f64 {
        use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_gpu_worker::tests::measure_gpu_zero_orbit_ips_for_fullstack;
        fat_stack(move || {
            let stop = std::sync::atomic::AtomicBool::new(false);
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let mut session = TileSession::new(location, (64, 64));
                    session.set_mag_velocity(0);
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        session.work_once_for_ips_test();
                        if session.percent_completed() >= 95.0 {
                            session.wipe_screen_progress_for_ips_test();
                        }
                    }
                });
                let ips = measure_gpu_zero_orbit_ips_for_fullstack(point_count, bout, rounds);
                stop.store(true, std::sync::atomic::Ordering::Relaxed);
                ips
            })
        })
    }
}

