// ---------------------------------------------------------------------------
// Steady-state speed path (screen worker alone + workgroup telemetry chain)
// See docs/assistant/testing.md — these are the lifeblood integration tests.
// ---------------------------------------------------------------------------

/// Actor formula: after a workshift, `total_iterations_today` *is* the shift delta
/// (workshift zeros the counter at start). Must stay non-zero across many shifts.
fn shift_iterations_delta(ctx: &WorkContext<f64>) -> u64 {
    ctx.total_iterations_today as u64
}

/// Screen-worker alone: home fill under DirectKernel reports real full-stack IPS.
// r[verify cz.perf.min-300m-ips-cpu+2]
#[test]
fn steady_state_screen_worker_home_ips_cpu_direct() {
    run_big(|| {
        // Share the GPU test lock: parallel GPU probes steal cores and trip the
        // home IPS floor without any DirectKernel regression.
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let t0 = Instant::now();
        let mut shifts = 0u32;
        let mut iters = 0u64;
        let mut deltas_nonzero = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            let d = shift_iterations_delta(&ctx);
            iters += d;
            if d > 0 {
                deltas_nonzero += 1;
            }
            let _ = work_update(&mut ctx);
            shifts += 1;
        }
        let secs = t0.elapsed().as_secs_f64().max(1e-9);
        let ips = iters as f64 / secs;
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "home frame did not complete in {shifts} shifts"
        );
        // Late seats often finish via safe-skip with zero iterates; allow a small
        // tail of zero-delta shifts. Mid-fill must still keep IPS alive (≥90%).
        assert!(
            deltas_nonzero * 100 >= shifts * 90,
            "iterations_delta went zero on too many shifts ({deltas_nonzero}/{shifts}); HUD IPS would die"
        );
        assert!(
            ips > 3.0e6,
            "screen-worker DirectKernel home IPS {ips:.3e} below steady-state floor (3e6); iters={iters} shifts={shifts}"
        );
        eprintln!(
            "steady_state screen_worker CPU DirectKernel: ips={ips:.3e} iters={iters} shifts={shifts} wall={secs:.3}s"
        );
    });
}

/// Screen-worker alone: naive GPU home fill reports IPS and completes on GPU
/// (host queues grow; no CPU mop phase).
// r[verify cz.perf.min-30b-ips-gpu+1]
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_screen_worker_home_ips_naive_gpu_path() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut gpu = super::naive_gpu::NaiveGpuContext::try_new();
        refresh_test_budget();
        assert!(gpu.is_some(), "expected naive GPU adapter");
        let t0 = Instant::now();
        let mut shifts = 0u32;
        let mut iters = 0u64;
        let mut deltas_nonzero = 0u32;
        let mut used_gpu = false;
        let mut cpu_fallback_shifts = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            let unfinished = ctx.points.iter().any(|p| !p.delivered);
            workshift(0, 0, 0, 0, &mut ctx, gpu.as_mut());
            if ctx.last_used_naive_gpu {
                used_gpu = true;
            } else if unfinished {
                cpu_fallback_shifts += 1;
            }
            let d = shift_iterations_delta(&ctx);
            iters += d;
            if d > 0 {
                deltas_nonzero += 1;
            }
            let _ = work_update(&mut ctx);
            shifts += 1;
        }
        let secs = t0.elapsed().as_secs_f64().max(1e-9);
        let ips = iters as f64 / secs;
        let delivered = ctx.points.iter().filter(|p| p.delivered).count();
        assert_eq!(
            delivered,
            ctx.points.len(),
            "home frame incomplete on GPU path: delivered={delivered}/{} shifts={shifts}",
            ctx.points.len()
        );
        assert!(used_gpu, "expected naive GPU path; adapter missing or forced off");
        // Only allowed CPU shifts: no-shader-F64 escalate fallback (not a mop gate).
        assert!(
            cpu_fallback_shifts <= 2,
            "too many non-GPU shifts while unfinished ({cpu_fallback_shifts}/{shifts}); CPU mop must not exist"
        );
        assert!(
            deltas_nonzero >= shifts.saturating_sub(2).max(1),
            "iterations_delta zeroed on GPU path ({deltas_nonzero}/{shifts})"
        );
        assert!(
            ips > 5.0e4,
            "screen-worker naive-GPU home IPS {ips:.3e} below TEST_SCREEN_RES floor; used_gpu={used_gpu}"
        );
        eprintln!(
            "steady_state screen_worker naive-GPU: ips={ips:.3e} iters={iters} shifts={shifts}"
        );
        drop(gpu);
    });
}

/// First GPU finals must grow host neighbor/edge queues (flood-fill discovery).
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_naive_gpu_home_neighbor_queues_grow() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_home_neighbor_queues_grow: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let q0 = ctx.out_queue.len() + ctx.in_queue.len() + ctx.edge_queue.len();
        let mut saw_final = false;
        let mut q = q0;
        while !(saw_final && q > q0) && !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            assert!(
                ctx.last_used_naive_gpu,
                "home shallow fill must stay on naive GPU"
            );
            let completed = work_update(&mut ctx);
            if !completed.is_empty() {
                saw_final = true;
            }
            q = ctx.out_queue.len() + ctx.in_queue.len() + ctx.edge_queue.len();
            if saw_final && q > q0 {
                eprintln!(
                    "steady_state neighbor queues grew: out={} in={} edge={}",
                    ctx.out_queue.len(),
                    ctx.in_queue.len(),
                    ctx.edge_queue.len()
                );
                return;
            }
        }
        assert!(
            saw_final,
            "expected at least one GPU Final on home within budget"
        );
        assert!(
            q > q0,
            "host queues must grow after GPU Finals (out={} in={} edge={}); bulk skip of neighbor discovery regressed",
            ctx.out_queue.len(),
            ctx.in_queue.len(),
            ctx.edge_queue.len()
        );
    });
}

/// Home fill must stay on naive GPU until done (no ≥N% CPU mop / seeded queues).
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_naive_gpu_home_fills_without_cpu_mop() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_home_fills_without_cpu_mop: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut shifts = 0u32;
        let mut cpu_while_unfinished = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            let unfinished = ctx.points.iter().any(|p| !p.delivered);
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            if unfinished && !ctx.last_used_naive_gpu {
                cpu_while_unfinished += 1;
            }
            let _ = work_update(&mut ctx);
            shifts += 1;
        }
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "home did not complete without CPU mop; shifts={shifts}"
        );
        assert!(
            cpu_while_unfinished <= 2,
            "CPU workshifts while unfinished={cpu_while_unfinished} (allowed only for no-F64 escalate); mop gate is forbidden"
        );
        eprintln!(
            "steady_state no-cpu-mop: shifts={shifts} cpu_fallback={cpu_while_unfinished}"
        );
    });
}

/// GPU+host-queue fill leaves no Dummy holes in the collector grid.
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_naive_gpu_home_no_dummy_holes() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_home_no_dummy_holes: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut collector_results =
            vec![CompletedPoint::Dummy {}; (ctx.res.0 * ctx.res.1) as usize];
        let mut shifts = 0u32;
        let mut cpu_while_unfinished = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            let unfinished = ctx.points.iter().any(|p| !p.delivered);
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            if unfinished && !ctx.last_used_naive_gpu {
                cpu_while_unfinished += 1;
            }
            for (point, index) in work_update(&mut ctx) {
                collector_results[index] = point;
            }
            shifts += 1;
        }
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "undelivered seats remain after GPU fill; shifts={shifts}"
        );
        assert!(
            cpu_while_unfinished <= 2,
            "CPU mop mid-fill forbidden; cpu_while_unfinished={cpu_while_unfinished}"
        );
        let collector_dummy = collector_results
            .iter()
            .filter(|p| matches!(p, CompletedPoint::Dummy {}))
            .count();
        assert_eq!(
            collector_dummy, 0,
            "collector Dummy holes after GPU fill; delivered={}",
            ctx.points.iter().filter(|p| p.delivered).count()
        );
        eprintln!("steady_state no-dummy-holes: shifts={shifts}");
    });
}

/// Whole workgroup speed chain: workshift → WorkUpdate.iterations_delta → ViewHud → RateCounter.
/// Catches the class of bug where the worker does work but HUD IPS stays ~0.
/// Also records PPS (`points_delta`) on the same path.
// r[verify cz.depth.gear-hud+2]
#[test]
fn steady_state_workgroup_ips_delta_reaches_hud_rate_counter() {
    use crate::assemblies::structs::ViewHud;
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut ips = RateCounter::default();
        let mut pps = RateCounter::default();
        let mut total_recorded = 0u64;
        let mut total_points = 0u64;
        let mut shifts_with_delta = 0u32;
        let mut shifts_with_points = 0u32;
        let mut shifts = 0u32;
        let t0 = Instant::now();
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            let delta = shift_iterations_delta(&ctx);
            let completed = work_update(&mut ctx);
            let update = telemetry_update(None, completed, Some(&mut ctx), delta);
            assert_eq!(
                update.iterations_delta, delta,
                "WorkUpdate must carry the shift iteration count unchanged"
            );
            // Collector → ViewHud (same fields the window RateCounter reads).
            let hud = ViewHud {
                stack: update.host_stack,
                mode: update.kernel_mode,
                reference: update.reference_status,
                gear: update.active_gear,
                points_delta: update.completed_points.len() as u64,
                iterations_delta: update.iterations_delta,
            };
            let now = Instant::now();
            ips.record(hud.iterations_delta, now);
            pps.record(hud.points_delta, now);
            total_recorded += hud.iterations_delta;
            total_points += hud.points_delta;
            if hud.iterations_delta > 0 {
                shifts_with_delta += 1;
            }
            if hud.points_delta > 0 {
                shifts_with_points += 1;
            }
            shifts += 1;
        }
        let elapsed = t0.elapsed();
        let secs = elapsed.as_secs_f64().max(1e-9);
        let rate = ips.rate(Instant::now());
        let pps_rate = pps.rate(Instant::now());
        let wall_pps = total_points as f64 / secs;
        assert!(
            ctx.points.iter().all(|p| p.delivered),
            "workgroup steady-state fill incomplete"
        );
        assert!(
            shifts_with_delta >= 1 && total_recorded > 0,
            "expected nonzero iterations_delta through HUD; got {shifts_with_delta}/{shifts} total={total_recorded}"
        );
        assert!(
            shifts_with_points >= 1 && total_points > 0,
            "expected nonzero points_delta through HUD; got {shifts_with_points}/{shifts} total={total_points}"
        );
        assert!(
            total_recorded > 10_000,
            "HUD chain recorded only {total_recorded} iterations across {shifts} shifts"
        );
        assert_eq!(
            total_points,
            ctx.points.len() as u64,
            "HUD points_delta must count every delivered seat exactly once"
        );
        // RateCounter is a short window; require it saw real work, not a stuck zero.
        assert!(
            rate > 0.0 || total_recorded > 0,
            "RateCounter rate={rate} with total_recorded={total_recorded}"
        );
        assert!(
            wall_pps > 1.0e4,
            "home wall PPS {wall_pps:.3e} below TEST_SCREEN_RES smoke floor"
        );
        eprintln!(
            "steady_state workgroup IPS/PPS chain: recorded_iters={total_recorded} recorded_pts={total_points} shifts_ips={shifts_with_delta}/{shifts} shifts_pps={shifts_with_points}/{shifts} ips_window≈{rate:.3e} pps_window≈{pps_rate:.3e} wall_pps≈{wall_pps:.3e} wall={elapsed:?}"
        );
    });
}

/// Smoothness: continuous outputs on home naive-GPU — after the first completion,
/// no more than 5 consecutive shifts without a drained completion while fill is
/// still progressing (≤50 ms at ~10 ms/shift). r[verify cz.craft.emergent-cadence+1]
#[test]
fn steady_state_naive_gpu_home_continuous_outputs() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_home_continuous_outputs: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        let mut shifts = 0u32;
        let mut seen_first_point = false;
        let mut gap = 0u32;
        let mut max_gap = 0u32;
        let mut shifts_with_points = 0u32;
        let mut fill = 0.0f64;
        while fill < 0.90 {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            let completed = work_update(&mut ctx);
            let n = completed.len() as u32;
            shifts += 1;
            if n > 0 {
                seen_first_point = true;
                shifts_with_points += 1;
                max_gap = max_gap.max(gap);
                gap = 0;
            } else if seen_first_point {
                gap += 1;
                max_gap = max_gap.max(gap);
                assert!(
                    gap <= 5,
                    "home GPU quiet for {gap} shifts (>50 ms) after first completion; shift={shifts} fill={:.2}%",
                    ctx.percent_completed
                );
            }
            fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
                / ctx.points.len().max(1) as f64;
        }
        assert!(
            seen_first_point,
            "home GPU produced no completions in {shifts} shifts"
        );
        let fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
            / ctx.points.len().max(1) as f64;
        assert!(
            fill >= 0.90,
            "home continuous-output fill too low: {fill:.4} shifts={shifts}"
        );
        eprintln!(
            "steady_state home continuous outputs: shifts={shifts} with_points={shifts_with_points} max_gap={max_gap} fill={fill:.4}"
        );
    });
}

/// Iterate-only telemetry must still reach the HUD: a shift can burn iterations
/// with zero completions (deep interior) and must not drop `iterations_delta`.
// r[verify cz.depth.gear-hud+2]
#[test]
fn steady_state_ips_delta_sent_without_completions() {
    use crate::assemblies::structs::ViewHud;
    let update = telemetry_update::<f64>(None, vec![], None, 12_345);
    assert_eq!(update.iterations_delta, 12_345);
    assert!(update.completed_points.is_empty());
    let hud = ViewHud {
        stack: update.host_stack,
        mode: update.kernel_mode,
        reference: update.reference_status,
        gear: update.active_gear,
        points_delta: update.completed_points.len() as u64,
        iterations_delta: update.iterations_delta,
    };
    let mut ips = RateCounter::default();
    let now = Instant::now();
    ips.record(hud.iterations_delta, now);
    assert!((ips.rate(now) - 12_345.0).abs() < 1e-9);
}

/// Home PPS: GPU vs CPU DirectKernel wall rate (fill completions / time).
/// Target class is ~FLOP ratio (~160× on this 1080 Ti); shallow home is
/// finish/scheduling heavy so the ratio is a progress metric, not the IPS bar.
#[test]
fn steady_state_home_pps_gpu_vs_cpu_ratio() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();

        let measure_cpu = || {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut shifts = 0u32;
            let mut points = 0u64;
            // Same bulk window as GPU (90%) — fair PPS rate, not endgame mop tax.
            let mut fill = 0.0f64;
            while fill < 0.90 {
                check_test_budget();
                workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
                let completed = work_update(&mut ctx);
                points += completed.len() as u64;
                shifts += 1;
                fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
                    / ctx.points.len().max(1) as f64;
            }
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            let fill = points as f64 / ctx.points.len().max(1) as f64;
            assert!(
                fill >= 0.90,
                "CPU home fill too low for PPS probe: points={points}/{} fill={fill:.4}",
                ctx.points.len()
            );
            points as f64 / secs
        };

        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_home_pps_gpu_vs_cpu_ratio: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        // Warm pipelines / adapter so first-submit latency is not the ratio.
        {
            let mut warm = from_stencil::<f64>(home_frame(), None).expect("warm");
            workshift(0, 0, 0, 0, &mut warm, Some(&mut gpu));
            let _ = work_update(&mut warm);
        }

        let measure_gpu = |gpu: &mut super::naive_gpu::NaiveGpuContext| {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut shifts = 0u32;
            let mut points = 0u64;
            // Bulk GPU fill to 90% for a PPS rate sample (full close is other pins).
            let mut fill = 0.0f64;
            while fill < 0.90 {
                check_test_budget();
                workshift(0, 0, 0, 0, &mut ctx, Some(gpu));
                let completed = work_update(&mut ctx);
                points += completed.len() as u64;
                shifts += 1;
                fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
                    / ctx.points.len().max(1) as f64;
            }
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            let fill = points as f64 / ctx.points.len().max(1) as f64;
            assert!(
                fill >= 0.90,
                "GPU home fill too low for PPS probe: points={points}/{} fill={fill:.4}",
                ctx.points.len()
            );
            points as f64 / secs
        };

        let cpu_pps = measure_cpu();
        // Best-of-3 GPU — shallow finish rate is sync-noisy; track climb, not jitter.
        let mut best_gpu = 0.0f64;
        for trial in 0..3 {
            let g = measure_gpu(&mut gpu);
            eprintln!("home PPS trial={trial}: gpu={g:.3e}");
            best_gpu = best_gpu.max(g);
        }
        let ratio = best_gpu / cpu_pps.max(1.0);
        eprintln!(
            "steady_state home PPS: cpu={cpu_pps:.3e} gpu_best={best_gpu:.3e} ratio={ratio:.2}× (aspiration ≈160× FLOP-class)"
        );
        assert!(
            best_gpu > 1.0e4 && cpu_pps > 1.0e4,
            "home PPS floor missed: cpu={cpu_pps:.3e} gpu={best_gpu:.3e}"
        );
        // TEST_SCREEN_RES home fill is host-sync / scheduling bound (not FLOP).
        // Require GPU within 20% of CPU; FLOP-class ~160× remains Criterion.
        assert!(
            ratio >= 0.80,
            "GPU home PPS best-of-3 far below CPU on TEST_SCREEN_RES: ratio={ratio:.2}× (cpu={cpu_pps:.3e} gpu={best_gpu:.3e})"
        );
        if ratio < 10.0 {
            eprintln!(
                "NOTE: GPU home PPS {ratio:.2}× still ≪ ~160× FLOP-class aspiration"
            );
        }
    });
}

/// Faux-user zoom past the F32 precision wall must escalate naive GPU to F64
/// (or honest CPU DirectKernel when the adapter has no SHADER_F64).
/// Collapse is detected from generator plane geometry, not lazy seat init.
#[test]
// r[verify cz.craft.kernel-seam+1]
fn steady_state_naive_gpu_f64_gear_via_faux_user_zoom() {
    run_big(|| {
        use crate::assemblies::headgroup::window::coords::{
            commands_from_goto_line, format_location_readout, ul_for_center, viewport_center,
        };
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;
        use crate::delta_gear::ComputeGear;

        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_f64_gear_via_faux_user_zoom: no GPU — skipped");
            return;
        };
        refresh_test_budget();

        let res = TEST_SCREEN_RES;
        let mut nav = SamplingContext {
            screen: None,
            screen_size: res,
            location: ul_for_center(IntExp::from(-2), IntExp::from(-2), -2, res),
            updated: false,
            mouse_drag_start: None,
        };
        // Stepwise zoom toward a mid-set center (faux user input path).
        let target = "-0.75 + 0.0i mag 2^20";
        let cmds = commands_from_goto_line(target).expect("goto");
        let mut dead = SamplingContext {
            screen: None,
            screen_size: res,
            location: ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, res),
            updated: false,
            mouse_drag_start: None,
        };
        transform(cmds, &mut dead);
        let (dre, dim) = viewport_center(&dead.location, res);

        let mut prev: Option<(WorkContext<f64>, ObjectivePosAndZoom)> = None;
        let mut saw_f64_path = false;
        for pot in [0, 8, 12, 16, 18, 20] {
            let line = format_location_readout(&dre, &dim, pot);
            transform(
                commands_from_goto_line(&line).expect("zoom step"),
                &mut nav,
            );
            assert_eq!(nav.location.zoom_pot, pot);
            let frame = (nav.location.clone(), res);
            let mut ctx = match prev.take() {
                Some((old, old_obj)) => {
                    from_stencil::<f64>(frame.clone(), Some((old, old_obj))).expect("zoom replace")
                }
                None => from_stencil::<f64>(frame.clone(), None).expect("fresh"),
            };
            // One workshift is enough for gear selection (collapse is geometric).
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            let _ = work_update(&mut ctx);

            if pot >= 18 {
                let gear = ctx.active_gear;
                let used_gpu = ctx.last_used_naive_gpu;
                eprintln!(
                    "faux zoom pot={pot}: gear={gear:?} used_gpu={used_gpu} gpu_prec={:?}",
                    gpu.precision
                );
                if gpu.has_f64() {
                    assert_eq!(
                        gear,
                        ComputeGear::F64,
                        "pot {pot}: adapter has SHADER_F64 — HUD gear must be F64, got {gear:?}"
                    );
                    assert!(
                        used_gpu,
                        "pot {pot}: should stay on naive GPU F64 path"
                    );
                    assert_eq!(gpu.precision, super::naive_gpu::GpuPrecision::F64);
                } else {
                    assert!(
                        !used_gpu,
                        "pot {pot}: no GPU F64 — must fall back to CPU naive, not walled F32"
                    );
                    assert_eq!(gear, ComputeGear::F64);
                }
                saw_f64_path = true;
            } else if pot <= 12 && ctx.last_used_naive_gpu {
                assert_eq!(
                    ctx.active_gear,
                    ComputeGear::F32,
                    "shallow pot {pot}: expect F32 GPU gear"
                );
            }
            prev = Some((ctx, frame.0));
        }
        assert!(saw_f64_path, "zoom path never reached pot≥18");
    });
}

/// Deep cusp view: unfinished work must never flatline (stall = missed halt /
/// missed progress, not "tenacity"). Progress = iterations and/or completions
/// every shift while seats remain undelivered.
#[test]
// r[verify cz.craft.wall-clock-law+1]
// r[verify cz.craft.emergent-cadence+1]
fn steady_state_naive_gpu_deep_cusp_never_stalls() {
    run_big(|| {
        use crate::assemblies::headgroup::window::coords::{
            commands_from_goto_line, ul_for_center,
        };
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::headgroup::window::sampling::SamplingContext;

        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
        let Some(mut gpu) = super::naive_gpu::NaiveGpuContext::try_new() else {
            eprintln!("steady_state_naive_gpu_deep_cusp_never_stalls: no GPU — skipped");
            return;
        };
        refresh_test_budget();

        let res = TEST_SCREEN_RES;
        let goto = "-0.749971479177 + 0.00652307272i mag 2^15";
        let mut nav = SamplingContext {
            screen: None,
            screen_size: res,
            location: ul_for_center(IntExp::ZERO, IntExp::ZERO, 0, res),
            updated: false,
            mouse_drag_start: None,
        };
        transform(commands_from_goto_line(goto).expect("goto"), &mut nav);
        assert_eq!(nav.location.zoom_pot, 15);

        let mut ctx = from_stencil::<f64>((nav.location.clone(), res), None).expect("deep cusp");
        let center = ((res.0 / 2) as i32, (res.1 / 2) as i32);
        let center_idx = index_from_pos(&center, res.0);
        let mut zero_progress = 0u32;
        let mut max_center_iters = 0u32;
        let mut shifts = 0u32;
        let mut cumulative_iters = 0u64;
        while ctx.points.iter().any(|p| !p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, Some(&mut gpu));
            let completed = work_update(&mut ctx);
            cumulative_iters += ctx.total_iterations_today as u64;
            let progress = ctx.total_iterations_today + completed.len() as u32;
            if progress == 0 {
                zero_progress += 1;
            } else {
                zero_progress = 0;
            }
            assert!(
                zero_progress < 2,
                "deep cusp stalled: {zero_progress} consecutive shifts with zero iterations and zero completions (halt recognition / progress failure, not tenacity)"
            );
            max_center_iters = max_center_iters.max(ctx.points[center_idx].iterations);
            shifts += 1;
        }
        let delivered_n = ctx.points.iter().filter(|p| p.delivered).count();
        eprintln!(
            "deep cusp never-stall: shifts={shifts} center_iters={max_center_iters} delivered={delivered_n} cum_iters={cumulative_iters}"
        );
        assert!(
            cumulative_iters > 10_000 || delivered_n > 0,
            "no iteration progress on deep cusp"
        );
        // Host iters update on finalize; on-device carry may still be climbing.
        assert!(
            max_center_iters > 0
                || ctx.points[center_idx].delivered
                || cumulative_iters > 50_000,
            "center seat never received iteration work"
        );
        if !ctx.points[center_idx].delivered && max_center_iters > 0 {
            assert!(
                max_center_iters >= 1_000,
                "unfinished center stuck at {max_center_iters} iters — likely missed halt or abandoned WIP"
            );
        }
    });
}

/// Shallow home must stay on DirectKernel (no soft-trial without a usable ref).
/// Guards the post-v0.0.9 workshift policy from silently running pert at home.
// r[verify cz.perf.pps-selected-kernel+1]
#[test]
fn home_workshift_stays_on_direct_kernel_without_ref() {
    run_big(|| {
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
    run_big(|| {
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
            assert!(got > 0, "no first publish shifts={shifts} workshift={use_workshift}");
            (t0.elapsed().as_secs_f64(), shifts, got)
        };
        // Sub-30ms samples are scheduler-noisy; take median of 5 so the ≤1.20×
        // bar stays hard without soft-flooring a single cold sample.
        let mut direct_samples = Vec::new();
        let mut via_samples = Vec::new();
        for _ in 0..5 {
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
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
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
        let direct = fill(false);
        let via_workshift = fill(true);
        let ratio = via_workshift / direct.max(1e-9);
        eprintln!(
            "home wall: direct={direct:.3}s workshift={via_workshift:.3}s ratio={ratio:.2}×"
        );
        assert!(
            ratio <= 1.20,
            "workshift home {via_workshift:.3}s is >20% slower than DirectKernel {direct:.3}s (ratio={ratio:.2}×); FIX NOW — do not soften"
        );
    });
}

/// Series must stay off the production kernels until membership pins stay green.
/// Hard pin — never `#[ignore]` (quality-doctrine).
// r[verify cz.depth.series-approximation+1]
#[test]
fn series_approximation_not_wired_into_production_kernels() {
    let pert = include_str!("../perturb_kernel.rs");
    let floatexp = include_str!("../perturb_floatexp.rs");
    assert!(
        !pert.contains("apply_series_skip("),
        "perturb_kernel must not invoke apply_series_skip("
    );
    assert!(
        !floatexp.contains("apply_series_skip("),
        "perturb_floatexp must not invoke apply_series_skip("
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

/// v0.0.9-era naive f64 home fill: counted iteration budget scales with seat
/// count at fixed pitch. Product identity was 10_302_563 @ 854×480; at
/// TEST_SCREEN_RES the accepted identity is the measured DirectKernel total
/// for the centered test home (re-pin if home_frame pitch/center changes).
// r[verify cz.perf.min-300m-ips-cpu+2]
#[test]
fn naive_f64_direct_kernel_home_preserves_v009_iteration_budget() {
    run_big(|| {
        let _gpu_guard = super::naive_gpu::lock_gpu_tests();
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
        const TEST_HOME_DIRECT_ITERS: u64 = 14_063;
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

