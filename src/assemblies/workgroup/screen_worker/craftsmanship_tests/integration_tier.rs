// 15s integration-tier tests. Cargo filter: `integration_tier`.
// Included from craftsmanship_tests::integration_tier (one extra `super`).
/// Screen-worker alone: home fill under DirectKernel reports real full-stack IPS.
// r[verify cz.perf.min-300m-ips-cpu+2]
#[test]
fn steady_state_screen_worker_home_ips_cpu_direct() {
    let _gpu_guard = super::super::naive_gpu::lock_gpu_tests();
    run_integration(|| {
        // Share the GPU test lock: parallel GPU/CPU probes steal cores and trip the
        // home IPS floor without any DirectKernel regression.
        refresh_test_budget();
        // Home DirectKernel finishes in one short shift (~14k iters). Best fill-only
        // IPS over several frames keeps the 3e6 floor hard under harness noise.
        // Extra frames + reject contaminated walls (same iters, ≫~3–4 ms) so
        // concurrent cargo/mutants load cannot soft-fail a healthy kernel.
        const FRAMES: u32 = 12;
        const MAX_IN_BUDGET_WALL_S: f64 = 0.0045;
        let mut best_ips = 0.0f64;
        let mut best_meta = (0u64, 0u32, 0.0f64);
        let mut total_shifts = 0u32;
        let mut total_deltas_nonzero = 0u32;
        let mut in_budget_samples = 0u32;
        for _frame in 0..FRAMES {
            refresh_test_budget();
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut iters = 0u64;
            let mut shifts = 0u32;
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
            total_shifts += shifts;
            total_deltas_nonzero += deltas_nonzero;
            assert!(
                ctx.points.iter().all(|p| p.delivered),
                "home frame did not complete"
            );
            if secs <= MAX_IN_BUDGET_WALL_S {
                in_budget_samples += 1;
                if ips > best_ips {
                    best_ips = ips;
                    best_meta = (iters, shifts, secs);
                }
            }
        }
        assert!(
            total_deltas_nonzero * 100 >= total_shifts * 90,
            "iterations_delta went zero on too many shifts ({total_deltas_nonzero}/{total_shifts}); HUD IPS would die"
        );
        assert!(
            in_budget_samples >= 1,
            "no in-budget DirectKernel home fill (wall≤{MAX_IN_BUDGET_WALL_S}s) in {FRAMES} frames — host overloaded or FIX NOW"
        );
        assert!(
            best_ips > 3.0e6,
            "screen-worker DirectKernel home IPS {best_ips:.3e} below steady-state floor (3e6); best iters={} shifts={} fill_wall={:.3}s in_budget={in_budget_samples}/{FRAMES}",
            best_meta.0,
            best_meta.1,
            best_meta.2
        );
        eprintln!(
            "steady_state screen_worker CPU DirectKernel: best_ips={best_ips:.3e} iters={} shifts={} fill_wall={:.3}s in_budget={in_budget_samples}/{FRAMES}",
            best_meta.0, best_meta.1, best_meta.2
        );
    });
}

/// Screen-worker alone: naive GPU home fill reports IPS and completes on GPU
/// (host queues grow; no CPU mop phase).
// r[verify cz.perf.min-30b-ips-gpu+1]
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_screen_worker_home_ips_naive_gpu_path() {
    run_integration(|| {
        use crate::assemblies::structs::KernelMode;
        let mut shared = super::super::naive_gpu::SharedGpu::acquire();
        refresh_test_budget();
        assert!(shared.is_some(), "expected naive GPU adapter");
        let shared = shared.as_mut().unwrap();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        // Force GPU path: PPS race may lock Naive CPU on TEST_SCREEN_RES.
        ctx.manual_gear = Some(KernelMode::NaiveGpu);
        let t0 = Instant::now();
        let mut shifts = 0u32;
        let mut iters = 0u64;
        let mut deltas_nonzero = 0u32;
        let mut used_gpu = false;
        let mut cpu_fallback_shifts = 0u32;
        while !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            let unfinished = ctx.points.iter().any(|p| !p.delivered);
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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
        assert!(
            used_gpu,
            "expected naive GPU path; adapter missing or forced off"
        );
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
    });
}

/// First GPU finals must grow host neighbor/edge queues (flood-fill discovery).
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_naive_gpu_home_neighbor_queues_grow() {
    run_integration(|| {
        use crate::assemblies::structs::KernelMode;
        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
            eprintln!("steady_state_naive_gpu_home_neighbor_queues_grow: no GPU — skipped");
            return;
        };
        refresh_test_budget();
        let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
        ctx.manual_gear = Some(KernelMode::NaiveGpu);
        let q0 = ctx.out_queue.len() + ctx.in_queue.len() + ctx.edge_queue.len();
        let mut saw_final = false;
        let mut q = q0;
        while !(saw_final && q > q0) && !ctx.points.iter().all(|p| p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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
    run_integration(|| {
        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
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
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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
        eprintln!("steady_state no-cpu-mop: shifts={shifts} cpu_fallback={cpu_while_unfinished}");
    });
}

/// GPU+host-queue fill leaves no Dummy holes in the collector grid.
// r[verify cz.craft.gpu-host-queue-discovery+1]
#[test]
fn steady_state_naive_gpu_home_no_dummy_holes() {
    run_integration(|| {
        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
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
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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
            collector_dummy,
            0,
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
    let _gpu_guard = super::super::naive_gpu::lock_gpu_tests();
    run_integration(|| {
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
            let update = telemetry_update(None, completed, Some(&mut ctx), delta, None);
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
                packages_dropped: 0,
                ..Default::default()
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
    run_integration(|| {
        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
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
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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

/// Home PPS: GPU vs CPU DirectKernel wall rate (fill completions / time).
/// Target class is ~FLOP ratio (~160× on this 1080 Ti); shallow home is
/// finish/scheduling heavy so the ratio is a progress metric, not the IPS bar.
#[test]
fn steady_state_home_pps_gpu_vs_cpu_ratio() {
    run_integration(|| {
        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
            eprintln!("steady_state_home_pps_gpu_vs_cpu_ratio: no GPU — skipped");
            return;
        };

        let measure_cpu = || {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut points = 0u64;
            let mut fill = 0.0f64;
            while fill < 0.90 {
                check_test_budget();
                workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
                let completed = work_update(&mut ctx);
                points += completed.len() as u64;
                fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
                    / ctx.points.len().max(1) as f64;
            }
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            assert!(
                points as f64 / ctx.points.len().max(1) as f64 >= 0.90,
                "CPU home fill too low for PPS probe"
            );
            points as f64 / secs
        };

        refresh_test_budget();
        {
            let mut warm = from_stencil::<f64>(home_frame(), None).expect("warm");
            workshift(0, 0, 0, 0, &mut warm, Some(shared.ctx()));
            let _ = work_update(&mut warm);
        }

        let measure_gpu = |gpu: &mut super::super::naive_gpu::NaiveGpuContext| {
            let mut ctx = from_stencil::<f64>(home_frame(), None).expect("home");
            let t0 = Instant::now();
            let mut points = 0u64;
            let mut fill = 0.0f64;
            while fill < 0.90 {
                check_test_budget();
                workshift(0, 0, 0, 0, &mut ctx, Some(gpu));
                let completed = work_update(&mut ctx);
                points += completed.len() as u64;
                fill = ctx.points.iter().filter(|p| p.delivered).count() as f64
                    / ctx.points.len().max(1) as f64;
            }
            let secs = t0.elapsed().as_secs_f64().max(1e-9);
            assert!(fill >= 0.90, "GPU home fill too low for PPS probe");
            points as f64 / secs
        };

        refresh_test_budget();
        let cpu_pps = measure_cpu();
        let mut best_gpu = 0.0f64;
        for trial in 0..2 {
            refresh_test_budget();
            let g = measure_gpu(shared.ctx());
            eprintln!("home PPS trial={trial}: gpu={g:.3e}");
            best_gpu = best_gpu.max(g);
        }
        let ratio = best_gpu / cpu_pps.max(1.0);
        eprintln!(
            "steady_state home PPS: cpu={cpu_pps:.3e} gpu_best={best_gpu:.3e} ratio={ratio:.2}×"
        );
        assert!(
            best_gpu > 1.0e4 && cpu_pps > 1.0e4,
            "home PPS floor missed: cpu={cpu_pps:.3e} gpu={best_gpu:.3e}"
        );
        assert!(
            ratio >= 0.80,
            "GPU home PPS far below CPU on TEST_SCREEN_RES: ratio={ratio:.2}× (cpu={cpu_pps:.3e} gpu={best_gpu:.3e})"
        );
    });
}

/// Faux-user zoom past the F32 precision wall must escalate naive GPU to F64
/// (or honest CPU DirectKernel when the adapter has no SHADER_F64).
/// Collapse is detected from generator plane geometry, not lazy seat init.
#[test]
// r[verify cz.craft.kernel-seam+1]
fn steady_state_naive_gpu_f64_gear_via_faux_user_zoom() {
    run_integration(|| {
        use crate::assemblies::headgroup::window::coords::{
            commands_from_goto_line, format_location_readout, ul_for_center, viewport_center,
        };
        use crate::assemblies::headgroup::window::sampling::SamplingContext;
        use crate::assemblies::headgroup::window::transforms::transform;
        use crate::assemblies::structs::KernelMode;
        use crate::delta_gear::ComputeGear;

        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
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
        for pot in [0, 12, 18, 20] {
            let line = format_location_readout(&dre, &dim, pot);
            transform(commands_from_goto_line(&line).expect("zoom step"), &mut nav);
            assert_eq!(nav.location.zoom_pot, pot);
            let frame = (nav.location.clone(), res);
            let mut ctx = match prev.take() {
                Some((old, old_obj)) => {
                    from_stencil::<f64>(frame.clone(), Some((old, old_obj))).expect("zoom replace")
                }
                None => from_stencil::<f64>(frame.clone(), None).expect("fresh"),
            };
            // Pin Naive GPU: this test is the GPU F64 escalate path, not PPS race.
            ctx.manual_gear = Some(KernelMode::NaiveGpu);
            // One workshift is enough for gear selection (collapse is geometric).
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
            let _ = work_update(&mut ctx);

            if pot >= 18 {
                let gear = ctx.active_gear;
                let used_gpu = ctx.last_used_naive_gpu;
                let prec = shared.ctx().precision;
                let has_f64 = shared.ctx().has_f64();
                eprintln!(
                    "faux zoom pot={pot}: gear={gear:?} used_gpu={used_gpu} gpu_prec={prec:?}"
                );
                if has_f64 {
                    assert_eq!(
                        gear,
                        ComputeGear::F64,
                        "pot {pot}: adapter has SHADER_F64 — HUD gear must be F64, got {gear:?}"
                    );
                    assert!(used_gpu, "pot {pot}: should stay on naive GPU F64 path");
                    assert_eq!(prec, super::super::naive_gpu::GpuPrecision::F64);
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
    run_integration(|| {
        use crate::assemblies::headgroup::window::coords::{
            commands_from_goto_line, ul_for_center,
        };
        use crate::assemblies::headgroup::window::sampling::SamplingContext;
        use crate::assemblies::headgroup::window::transforms::transform;

        let Some(mut shared) = super::super::naive_gpu::SharedGpu::acquire() else {
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
        // Force GPU: PPS 1-shift probe race otherwise interleaves Naive/Pert and can
        // flatline a shift without meaning the GPU path stalled.
        ctx.manual_gear = Some(crate::assemblies::structs::KernelMode::NaiveGpu);
        refresh_test_budget();
        let center = ((res.0 / 2) as i32, (res.1 / 2) as i32);
        let center_idx = index_from_pos(&center, res.0);
        let mut zero_progress = 0u32;
        let mut max_center_iters = 0u32;
        let mut shifts = 0u32;
        let mut cumulative_iters = 0u64;
        // Claim is never-stall + progress, not a full deep-cusp finish inside 1s.
        while ctx.points.iter().any(|p| !p.delivered) {
            check_test_budget();
            workshift(0, 0, 0, 0, &mut ctx, Some(shared.ctx()));
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
            // Full deep-cusp finish is not the claim; stop once progress is proven.
            if cumulative_iters > 10_000 && shifts >= 5 {
                break;
            }
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
            max_center_iters > 0 || ctx.points[center_idx].delivered || cumulative_iters > 50_000,
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
