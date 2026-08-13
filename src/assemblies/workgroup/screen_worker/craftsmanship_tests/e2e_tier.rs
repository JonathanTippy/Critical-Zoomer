// 60s e2e-tier lib test. Cargo filter: `e2e_tier`.
// Cadence lives in tests/pipeline_cadence.rs (run separately).
/// After home fill completes, seats must stay delivered for a long idle window.
/// Short tests missed post-settle reopen / continuous workshift thrash.
// r[verify cz.craft.load-proportional-ignorance+1]
#[test]
fn steady_state_home_stays_parked_for_10s_after_fill() {
    let _gpu_guard = super::super::naive_gpu::lock_gpu_tests();
    run_e2e(|| {
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut shifts = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            let _ = work_update(&mut ctx);
            shifts += 1;
            assert!(shifts < 50_000, "home fill never completed");
        }
        assert!(ctx.points.iter().all(|p| p.delivered));
        // Fill is done; the 10s settle is the assertion, not a hung-fill guard.
        suspend_test_budget();

        let idle_start = Instant::now();
        let mut idle_shifts = 0u32;
        let mut idle_iters = 0u64;
        while idle_start.elapsed() < Duration::from_secs(10) {
            // Do not refresh_test_budget here — wall clock is the settle gate.
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            idle_iters += shift_iterations_delta(&ctx);
            let _ = work_update(&mut ctx);
            idle_shifts += 1;
            assert!(
                ctx.points.iter().all(|p| p.delivered),
                "seat reopened during post-fill settle; shift={idle_shifts} after {elapsed:?}",
                elapsed = idle_start.elapsed()
            );
            // Sample across the full 10s without spinning the core at full tilt.
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(
            idle_iters, 0,
            "parked home must not burn iterations over 10s idle; shifts={idle_shifts} iters={idle_iters}"
        );
        assert!(
            idle_shifts > 0,
            "settle loop must run at least one post-fill shift"
        );
    });
}
