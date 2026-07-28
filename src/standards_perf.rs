//! Hard-assert verifies for standards.md / requirements.md numeric bars.
//! Release-gated timing uses `cfg(not(debug_assertions))` — not `#[ignore]`.

#[cfg(test)]
mod standards_hard_bar_tests {
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    use crate::assemblies::headgroup::window::{HEADGROUP_PRESENT_MODE, VSYNC};
    use crate::assemblies::workgroup_new::tile_session::TileSession;
    use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
    use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
        iterate_perturbation_bout, PerturbationCpuWorkerState,
    };
    use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
    use crate::constants::*;
    use crate::gear::Gear;
    use crate::intexp::IntExp;
    use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};
    use crate::utils::ObjectivePosAndZoom;

    const BOUT: u32 = 1_000;
    const CPU_IPS_MIN: f64 = 300_000_000.0;

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

    // r[verify cz.perf.min-300m-ips-cpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_exterior_cusp_meets_300m() {
        let ips = cpu_workgroup_ips((0.2500001, 0.0), 4);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_exterior_neck_meets_300m() {
        let ips = cpu_workgroup_ips((-0.75, 0.02), 4);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    // r[verify cz.perf.min-300m-ips-cpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn cpu_ips_deep_exterior_meets_300m() {
        let ips = cpu_workgroup_ips((0.251, 0.0), 4);
        assert!(ips >= CPU_IPS_MIN, "CPU IPS {ips} < {CPU_IPS_MIN}");
    }

    /// Workgroup IPS: parallel independent zero-orbit bouts (standards: workgroup performance).
    fn cpu_workgroup_ips(c: (f64, f64), threads: usize) -> f64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;
        let target_per = 20_000_000u64;
        let total = AtomicU64::new(0);
        let start = Instant::now();
        thread::scope(|scope| {
            for t in 0..threads {
                let total = &total;
                scope.spawn(move || {
                    let eps = epsilon(c);
                    let mut state = PerturbationCpuWorkerState::default();
                    state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
                    state.iterations_per_bout = 50_000;
                    let jitter = (t as f64) * 1e-10;
                    let cj = (c.0 + jitter, c.1);
                    let mut got = 0u64;
                    while got < target_per {
                        let mut point = fresh_point(cj, ZERO_ORBIT_ID);
                        while !point.finished && got < target_per {
                            let before = point.iteration_count;
                            iterate_perturbation_bout(&mut state, &mut point, eps);
                            got += point.iteration_count - before;
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
        // Far exterior escapes after one map; encoded escape time is 2 because
        // escape_time_r2==1 is reserved for NORES_ANSWER.
        assert_eq!(point.iteration_count, 2);
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
    #[test]
    fn stacked_ladder_reaches_adaptive_for_deep() {
        assert_eq!(Gear::select(3_600_000, false), Gear::AdaptiveRug);
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_counters_start_zero() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
            zoom_pot: HOME_POSITION.2,
        };
        let session = TileSession::new(location, (64, 64));
        assert_eq!(session.foveation_work_ns(), (0, 0));
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_work_accumulates_on_workshift() {
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
    }

    // r[verify cz.perf.foveation-half-time+1]
    #[test]
    fn foveation_balance_both_halves_within_factor_two() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
            zoom_pot: HOME_POSITION.2,
        };
        let mut session = TileSession::new(location, (128, 128));
        session.force_cpu_bouts_for_test();
        // Zoom-in velocity opens lookahead immediately (stationary defers it).
        session.set_mag_velocity(1);
        for _ in 0..400 {
            session.workshift();
        }
        let (s, l) = session.foveation_work_ns();
        assert!(s > 0 && l > 0, "both halves must work: screen={s} lookahead={l}");
        let ratio = (s as f64) / (l as f64);
        assert!(
            ratio >= 0.5 && ratio <= 2.0,
            "foveation imbalance screen={s} lookahead={l} ratio={ratio}"
        );
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

    // r[verify cz.perf.headgroup-shaders-2ms+1]
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
    #[cfg(not(debug_assertions))]
    #[test]
    fn shader_sample_shade_800x480_under_2ms() {
        use crate::assemblies::headgroup::window::gpu_display::shade_harness::gpu_or_skip;
        let gpu = gpu_or_skip("shader_sample_shade_800x480_under_2ms")
            .expect("GPU adapter required for headgroup shader 2ms bar");
        let mut settings = Settings::DEFAULT;
        let frame = shade_timing_frame((800, 480), &mut settings, 0.0);
        let elapsed = paint_ms(gpu, &frame);
        assert!(
            elapsed <= Duration::from_millis(2)
            , "sample+shade {:?} > 2ms @800x480"
            , elapsed
        );
    }

    // r[verify cz.perf.headgroup-shaders-2ms+1]
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
}

