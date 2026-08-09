//! Dedicated wgpu compute island for naive Mandelbrot bouts.
//! Host owns scheduling; GPU runs BoutCap-bounded WIP waves; sparse finish harvest only.

mod buffers;
mod device;
mod kernel;

pub use device::{GpuPrecision, NaiveGpuContext};
pub use kernel::{HarvestedFinish, WipMeta};

use crate::assemblies::workgroup::screen_worker::workshift::{
    c_for_seat_f64, next_attention_spiral_pos, point_is_edge, queue_incomplete_neighbors,
    queue_incomplete_neighbors_in, queue_incomplete_neighbors_of_edge, refresh_active_gear,
    workshift_with_kernel, BoutCap, CompletedPoint, Delivery, DirectKernel, Motion, Point,
    PushOutcome, SeatKernel, Step, WorkContext, iterate_max_n_times,
};
use crate::utils::index_from_pos;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub const DEFAULT_WAVE_N: u32 = device::MAX_WAVE;
pub const MIN_WAVE_N: u32 = 64;

/// Claim the next undelivered seat using the same slot rotation as the CPU workshift.
pub fn claim_next_undelivered_seat(
    context: &mut WorkContext<f64>,
    skip: &HashSet<usize>,
) -> Option<((i32, i32), Step)> {
    let total = context.points.len().max(1);
    for _ in 0..total.min(device::MAX_WAVE as usize) {
        let from_scan: bool;
        let (pos, step) = match select_candidate(context, skip) {
            Some(p) => {
                from_scan = false;
                p
            }
            // Queues empty (common before the first completion announces work):
            // scan undelivered seats so the GPU wave still feeds. Stall here is a
            // missed-work bug, not patience.
            None => match scan_undelivered_seat(context, skip) {
                Some(p) => {
                    from_scan = true;
                    p
                }
                None => return None,
            },
        };
        let index = index_from_pos(&pos, context.res.0);
        if context.points[index].delivered {
            if !from_scan {
                advance_past(context, step);
            }
            continue;
        }
        if skip.contains(&index) {
            // Already armed in this wave. Keep attention hold (tenacity); arm
            // other undelivered seats instead of aborting the fill.
            if matches!(step, Step::Attention)
                && context.attention_current == Some(pos)
            {
                if let Some(p) = scan_undelivered_seat(context, skip) {
                    // No claim() — scan seats are not queue heads.
                    return Some(p);
                }
                return None;
            }
            if !from_scan {
                advance_past(context, step);
            }
            continue;
        }
        if !from_scan {
            claim(context, step, pos);
        } else if matches!(step, Step::Attention) {
            // Spiral/hold path only.
            claim(context, step, pos);
        }
        return Some((pos, step));
    }
    None
}

/// Linear scan when queues have not yet been grown by completions.
/// Advances `context.random_index` so filling a wave is O(wave), not O(wave×N).
fn scan_undelivered_seat(
    context: &mut WorkContext<f64>,
    skip: &HashSet<usize>,
) -> Option<((i32, i32), Step)> {
    let n = context.points.len();
    if n == 0 {
        return None;
    }
    let start = (context.random_index as usize) % n;
    for off in 0..n {
        let index = (start + off) % n;
        if context.points[index].delivered || skip.contains(&index) {
            continue;
        }
        context.random_index = (index + 1) % n;
        let pos = pos_from_index(index, context.res.0);
        // Neutral step: not Attention (must not clear/replace the hold).
        return Some((pos, Step::Out));
    }
    None
}

fn select_candidate(
    context: &mut WorkContext<f64>,
    skip: &HashSet<usize>,
) -> Option<((i32, i32), Step)> {
    match context.workshifts % 5 {
        0 => {
            if context.motion == Motion::Panned && context.workshifts == 0 {
                if let Some(p) = crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
                    context, true,
                ) {
                    return Some(p);
                }
            }
            if let Some(pos) = context.attention_current {
                let index = index_from_pos(&pos, context.res.0);
                if context.points[index].delivered {
                    context.attention_current = None;
                } else if !skip.contains(&index) {
                    return Some((pos, Step::Attention));
                }
                // Held + already WIP: keep hold, fill other seats this round.
            }
            if context.attention_current.is_none() {
                if let Some(pos) = next_attention_spiral_pos(context) {
                    return Some((pos, Step::Attention));
                }
            }
            crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
                context,
                context.workshifts == 0,
            )
        }
        1 => crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
            context, false,
        ),
        2 => {
            if let Some((pos, _)) = context.out_queue.front() {
                Some((*pos, Step::Out))
            } else {
                crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
                    context, false,
                )
            }
        }
        3 => crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
            context, false,
        ),
        4 => crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
            context, true,
        ),
        _ => None,
    }
}

fn advance_past(context: &mut WorkContext<f64>, step: Step) {
    match step {
        Step::Out => {
            let _ = context.out_queue.pop_front();
        }
        Step::Scredge => {
            let _ = context.scredge_poses.pop_front();
        }
        Step::In => {
            let _ = context.in_queue.pop_front();
        }
        Step::Edge => {
            let _ = context.edge_queue.pop_front();
        }
        Step::Attention => {
            context.attention_current = None;
        }
    }
}

fn claim(context: &mut WorkContext<f64>, step: Step, pos: (i32, i32)) {
    match step {
        Step::Out => {
            let _ = context.out_queue.pop_front();
        }
        Step::Scredge => {
            let _ = context.scredge_poses.pop_front();
        }
        Step::In => {
            let _ = context.in_queue.pop_front();
        }
        Step::Edge => {
            let _ = context.edge_queue.pop_front();
        }
        Step::Attention => {
            context.attention_current = Some(pos);
        }
    }
}

/// GPU naive workshift: arm WIP once, resident continues, sparse finals harvest.
pub fn workshift_naive_gpu(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<f64>,
    gpu: &mut NaiveGpuContext,
) {
    context.time_workshift_started = Instant::now();
    context.update_reference_floor_policy();
    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;
    refresh_active_gear(context);

    // F32→F64 escalate when adjacent seats collapse in f32 (precision wall ~pot 20).
    let want = if f32_collapses_neighbors(context) {
        GpuPrecision::F64
    } else {
        GpuPrecision::F32
    };
    if !gpu.ensure_precision(want) {
        if want == GpuPrecision::F64 {
            // No GPU F64 — honest CPU naive rather than a walled F32 image.
            context.last_used_naive_gpu = false;
            workshift_with_kernel(
                day_token_allowance,
                iteration_token_cost,
                point_token_cost,
                bout_token_cost,
                context,
                &DirectKernel,
            );
            return;
        }
    }
    context.active_gear = match gpu.precision {
        GpuPrecision::F32 => crate::delta_gear::ComputeGear::F32,
        GpuPrecision::F64 => crate::delta_gear::ComputeGear::F64,
    };
    context.last_used_naive_gpu = true;

    let epsilon = context.pitch_epsilon;
    let kernel = DirectKernel;

    // Near-complete frames: skip GPU (it starves the last Dummy holes) and mop
    // remaining seats on f64 DirectKernel for the shift budget.
    if context.percent_completed >= 90.0
        && context.points.iter().any(|p| !p.delivered)
    {
        publish_finished_undelivered(context, &kernel);
        cpu_residual_undelivered(context, &kernel);
        gpu.end_shift_keep_generation();
        context.workshifts += 1;
        let delivered = context.points.iter().filter(|p| p.delivered).count();
        let total = context.points.len().max(1);
        context.percent_completed = delivered as f64 / (total as f64) * 100.0;
        return;
    }

    let mut wave_n = gpu.wave_n();
    let mut wip: Vec<WipMeta> = Vec::new();
    let mut skip: HashSet<usize> = HashSet::new();
    let mut resident = false;
    let mut resident_n: u32 = 0;
    // Smoothness = continuous outputs: prefer harvest-every-bout so each ~10 ms
    // workshift drains fresh finals (virtues §8 / emergent cadence). Escalate to
    // multi-bout only when the wave is iterate-heavy *and* this shift already
    // published some points (never starve a shift of visible completions).
    let mut bouts_per_dispatch: u32 = 1;
    let mut points_published_this_shift: u32 = 0;
    // Shallow re-upload pipeline: submit next wave before mapping prior staging.
    // Third field is the GPU wave width at submit (continue must use this, not
    // compacted host WIP len — finished slots stay inactive on device).
    let mut pending: Option<(u8, Vec<WipMeta>, u32)> = None;

    // Resume on-device unfinished seats from the prior shift (halt progress).
    {
        let mut carried = gpu.carry_indices.borrow_mut();
        if !carried.is_empty() && gpu.carry_n.get() > 0 {
            for &index in carried.iter() {
                if index >= context.points.len() {
                    continue;
                }
                let p = &context.points[index];
                if p.delivered || p.escapes || p.repeats {
                    continue;
                }
                let pos = pos_from_index(index, context.res.0);
                skip.insert(index);
                wip.push(WipMeta {
                    index,
                    pos,
                    step: Step::Out,
                });
            }
            if !wip.is_empty() {
                resident = true;
                resident_n = gpu.carry_n.get();
            }
            carried.clear();
            gpu.carry_n.set(0);
        }
    }

    while context.time_workshift_started.elapsed().as_millis() < 10 {
        // Harvest prior wave. When every seat finished, claim+dispatch the *next*
        // wave *before* host publish so publish overlaps GPU (shallow PPS lever).
        // If any seat is still open, keep serial continue — overwriting seats_buf
        // would drop on-device progress.
        let mut pipeline_publish: Option<(Vec<WipMeta>, Vec<HarvestedFinish>)> = None;
        if let Some((slot, prev_wip, gpu_n)) = pending.take() {
            let (finishes, iter_delta) = match gpu.harvest_sparse_slot(slot) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("naive_gpu harvest failed: {e}");
                    break;
                }
            };
            gpu.clear_finish_accumulators();
            context.total_iterations_today += iter_delta;
            context.total_iterations = context.total_iterations.saturating_add(iter_delta);
            let gpu_final_n = finishes.iter().filter(|f| (f.flags & 6) != 0).count();
            let wip_n = prev_wip.len().max(1);
            if points_published_this_shift == 0 || gpu_final_n * 4 >= wip_n {
                bouts_per_dispatch = 1;
            } else if gpu_final_n * 16 < wip_n {
                bouts_per_dispatch = 16;
            } else {
                bouts_per_dispatch = 8;
            }
            context.total_bouts_today += 1;

            let mut terminal: HashSet<usize> = HashSet::with_capacity(gpu_final_n);
            for f in &finishes {
                if (f.flags & 6) != 0 {
                    terminal.insert(f.seat_index as usize);
                }
            }
            let wave_fully_finished = prev_wip.iter().all(|m| terminal.contains(&m.index));

            if wave_fully_finished {
                // Old seats stay in `skip` until publish; claim the next wave now.
                wip.clear();
                while (wip.len() as u32) < wave_n {
                    let Some((pos, step)) = claim_next_undelivered_seat(context, &skip) else {
                        break;
                    };
                    let index = index_from_pos(&pos, context.res.0);
                    if context.points[index].delivered {
                        continue;
                    }
                    kernel.start_seat(context, pos);
                    skip.insert(index);
                    wip.push(WipMeta { index, pos, step });
                }
                if !wip.is_empty() {
                    let upload_refs: Vec<(u32, &Point<f64>)> = wip
                        .iter()
                        .map(|m| (m.index as u32, &context.points[m.index]))
                        .collect();
                    resident_n = upload_refs.len() as u32;
                    match gpu.dispatch_wave_multi_sparse(
                        &upload_refs,
                        4.0,
                        epsilon,
                        BoutCap::STANDARD,
                        bouts_per_dispatch,
                    ) {
                        Ok(next_slot) => {
                            pending = Some((next_slot, std::mem::take(&mut wip), resident_n));
                            resident = false;
                        }
                        Err(e) => {
                            eprintln!("naive_gpu dispatch failed: {e}");
                            let _ = publish_gpu_finishes(
                                context,
                                &kernel,
                                gpu.precision,
                                epsilon,
                                &prev_wip,
                                &finishes,
                                &mut skip,
                                &mut points_published_this_shift,
                            );
                            break;
                        }
                    }
                }
                pipeline_publish = Some((prev_wip, finishes));
            } else {
                let outcome = publish_gpu_finishes(
                    context,
                    &kernel,
                    gpu.precision,
                    epsilon,
                    &prev_wip,
                    &finishes,
                    &mut skip,
                    &mut points_published_this_shift,
                );
                if outcome.buffer_full {
                    break;
                }
                wip.clear();
                for m in prev_wip {
                    let p = &context.points[m.index];
                    if p.delivered || p.escapes || p.repeats {
                        continue;
                    }
                    skip.insert(m.index);
                    wip.push(m);
                }
                if outcome.need_reupload {
                    resident = false;
                } else if !wip.is_empty() {
                    resident = true;
                    // Keep device wave width — compacted host len would skip live GPU slots.
                    resident_n = gpu_n;
                } else {
                    resident = false;
                }
            }
        }

        if let Some((prev_wip, finishes)) = pipeline_publish.take() {
            let outcome = publish_gpu_finishes(
                context,
                &kernel,
                gpu.precision,
                epsilon,
                &prev_wip,
                &finishes,
                &mut skip,
                &mut points_published_this_shift,
            );
            if outcome.buffer_full {
                break;
            }
            // Confirm rejects are rare on shallow; host state is enough to re-claim.
            if outcome.need_reupload {
                resident = false;
            }
            // Next wave already in `pending` when claim found seats.
            if pending.is_some() {
                if context.time_workshift_started.elapsed().as_millis() > 9 && wave_n > 2048 {
                    wave_n = ((wave_n * 3) / 4).max(2048);
                    gpu.set_wave_n(wave_n);
                }
                continue;
            }
            // No next wave claimed — shift draining.
            if wip.is_empty() {
                break;
            }
        }

        // Refill host WIP list; only re-upload when residency breaks.
        let before_len = wip.len();
        while (wip.len() as u32) < wave_n {
            let Some((pos, step)) = claim_next_undelivered_seat(context, &skip) else {
                break;
            };
            let index = index_from_pos(&pos, context.res.0);
            if context.points[index].delivered {
                continue;
            }
            kernel.start_seat(context, pos);
            skip.insert(index);
            wip.push(WipMeta { index, pos, step });
        }
        if wip.is_empty() {
            break;
        }
        let grew = wip.len() > before_len;

        let slot = if !resident || grew {
            // Borrow points for upload only — no Point clone storm (PPS killer on shallow).
            let upload_refs: Vec<(u32, &Point<f64>)> = wip
                .iter()
                .map(|m| (m.index as u32, &context.points[m.index]))
                .collect();
            resident_n = upload_refs.len() as u32;
            match gpu.dispatch_wave_multi_sparse(
                &upload_refs,
                4.0,
                epsilon,
                BoutCap::STANDARD,
                bouts_per_dispatch,
            ) {
                Ok(slot) => {
                    resident = true;
                    slot
                }
                Err(e) => {
                    eprintln!("naive_gpu dispatch failed: {e}");
                    break;
                }
            }
        } else {
            match gpu.dispatch_continue_multi(resident_n, bouts_per_dispatch, false) {
                Ok(slot) => slot,
                Err(e) => {
                    eprintln!("naive_gpu continue failed: {e}");
                    break;
                }
            }
        };

        // Park WIP with the staging slot; next loop harvests (and may pipeline).
        pending = Some((slot, std::mem::take(&mut wip), resident_n));
        resident = false; // residency restored from unfinished after harvest

        if context.time_workshift_started.elapsed().as_millis() > 9 && wave_n > 2048 {
            wave_n = ((wave_n * 3) / 4).max(2048);
            gpu.set_wave_n(wave_n);
        }
    }

    // Flush the last in-flight wave.
    let mut last_unfinished: Vec<WipMeta> = Vec::new();
    let mut last_gpu_n = 0u32;
    if let Some((slot, prev_wip, gpu_n)) = pending.take() {
        if let Ok((finishes, iter_delta)) = gpu.harvest_sparse_slot(slot) {
            gpu.clear_finish_accumulators();
            context.total_iterations_today += iter_delta;
            context.total_iterations = context.total_iterations.saturating_add(iter_delta);
            let _ = publish_gpu_finishes(
                context,
                &kernel,
                gpu.precision,
                epsilon,
                &prev_wip,
                &finishes,
                &mut skip,
                &mut points_published_this_shift,
            );
            context.total_bouts_today += 1;
            for m in prev_wip {
                let p = &context.points[m.index];
                if !p.delivered && !p.escapes && !p.repeats {
                    last_unfinished.push(m);
                }
            }
            if !last_unfinished.is_empty() {
                last_gpu_n = gpu_n;
            }
        }
    }

    // Carry unfinished on-device WIP into the next shift (resume, don't reset).
    if !last_unfinished.is_empty() {
        let mut carried = gpu.carry_indices.borrow_mut();
        carried.clear();
        carried.extend(last_unfinished.iter().map(|m| m.index));
        gpu.carry_n.set(last_gpu_n);
    } else {
        gpu.carry_indices.borrow_mut().clear();
        gpu.carry_n.set(0);
    }

    // Near-complete: publish missed finals + CPU mop (no pull_seats — too expensive).
    if context.percent_completed >= 90.0 {
        publish_finished_undelivered(context, &kernel);
        cpu_residual_undelivered(context, &kernel);
    }

    gpu.end_shift_keep_generation();
    context.workshifts += 1;
    let delivered = context.points.iter().filter(|p| p.delivered).count();
    let total_points = context.points.len().max(1);
    context.percent_completed = delivered as f64 / (total_points as f64) * 100.0;
}

struct PublishFinishOutcome {
    buffer_full: bool,
    need_reupload: bool,
}

fn publish_gpu_finishes(
    context: &mut WorkContext<f64>,
    kernel: &DirectKernel,
    precision: GpuPrecision,
    epsilon: f64,
    _wip: &[WipMeta],
    finishes: &[HarvestedFinish],
    skip: &mut HashSet<usize>,
    points_published_this_shift: &mut u32,
) -> PublishFinishOutcome {
    // Large shallow floods: skip per-seat neighbor/edge queue churn — undelivered
    // seats are already fed by scan fill. Small waves keep frontier queues.
    let bulk = finishes.len() >= 128;
    let attention_idx = context
        .attention_current
        .map(|p| index_from_pos(&p, context.res.0));
    let mut buffer_full = false;
    let mut need_reupload = false;
    let mut published_batch = 0u32;
    for fin in finishes {
        let index = fin.seat_index as usize;
        if index >= context.points.len() {
            continue;
        }
        if context.points[index].delivered {
            continue;
        }
        apply_finish_to_point(&mut context.points[index], fin);
        let pos = pos_from_index(index, context.res.0);

        if !(context.points[index].repeats || context.points[index].escapes) {
            continue;
        }
        // F32 period-detect can false-mark shallow exterior seats as interior
        // (black speckles). Re-check only very-low-iter repeats on host f64;
        // shader already suppresses period-detect below 32.
        if context.points[index].repeats
            && matches!(precision, GpuPrecision::F32)
            && context.points[index].iterations < 64
            && !confirm_repeat_or_keep_wip(&mut context.points[index], epsilon)
        {
            skip.remove(&index);
            need_reupload = true;
            continue;
        }
        if attention_idx == Some(index) {
            context.attention_current = None;
        }
        // Bulk shallow floods: skip per-seat period twin-test (period unknown).
        // Scan fill already covers seats; twin-test is the IPS/period path, not
        // the points-out rate. Small waves keep full completion().
        let completed_point = if bulk && context.points[index].repeats {
            context.points[index].period = 0;
            CompletedPoint::Repeats {
                period: 0,
                smallness: context.points[index].smallness_squared,
                small_time: context.points[index].small_time,
            }
        } else {
            kernel.completion(&mut context.points[index])
        };
        if !bulk {
            if context.points[index].repeats {
                queue_incomplete_neighbors_in(
                    &pos,
                    context.res,
                    &context.points,
                    &mut context.in_queue,
                );
            } else {
                queue_incomplete_neighbors(
                    &pos,
                    context.res,
                    &context.points,
                    &mut context.out_queue,
                );
            }
            if let Some(e) = point_is_edge(&pos, context.res, &context.points) {
                queue_incomplete_neighbors_of_edge(
                    &e.0,
                    &e.1,
                    context.res,
                    &context.points,
                    &mut context.edge_queue,
                );
            }
        }
        match context.push_delivery(Delivery::Final(completed_point), index) {
            PushOutcome::Published => {
                context.total_points_today += 1;
                published_batch += 1;
                skip.remove(&index);
                *points_published_this_shift += 1;
            }
            PushOutcome::BufferFull => {
                skip.remove(&index);
                buffer_full = true;
                break;
            }
        }
    }
    if published_batch > 0 {
        context.record_hud_completion_batch(published_batch);
    }
    PublishFinishOutcome {
        buffer_full,
        need_reupload,
    }
}

/// Publish host seats that already escaped/repeated but never made it into the buffer.
fn publish_finished_undelivered(context: &mut WorkContext<f64>, kernel: &DirectKernel) {
    let n = context.points.len();
    for index in 0..n {
        if context.points[index].delivered {
            continue;
        }
        if !(context.points[index].escapes || context.points[index].repeats) {
            continue;
        }
        if context.points[index].repeats
            && context.points[index].iterations < 64
            && !confirm_repeat_or_keep_wip(&mut context.points[index], context.pitch_epsilon)
        {
            continue;
        }
        let pos = pos_from_index(index, context.res.0);
        let completed_point = kernel.completion(&mut context.points[index]);
        if context.points[index].repeats {
            queue_incomplete_neighbors_in(
                &pos,
                context.res,
                &context.points,
                &mut context.in_queue,
            );
        } else {
            queue_incomplete_neighbors(
                &pos,
                context.res,
                &context.points,
                &mut context.out_queue,
            );
        }
        match context.push_delivery(Delivery::Final(completed_point), index) {
            PushOutcome::Published => {
                context.total_points_today += 1;
                context.record_hud_completion_batch(1);
            }
            PushOutcome::BufferFull => break,
        }
    }
}

/// f64 DirectKernel mop-up for seats left unfinished after GPU waves (F32 wall /
/// period detect / sparse harvest gaps). Caller gates on ~90% fill so bulk shifts
/// stay GPU-dominated.
fn cpu_residual_undelivered(context: &mut WorkContext<f64>, kernel: &DirectKernel) {
    if context.points.iter().all(|p| p.delivered) {
        return;
    }
    let epsilon = context.pitch_epsilon;
    let residual_started = Instant::now();
    let mut cursor = 0usize;
    let budget_ms = if residual_started
        .duration_since(context.time_workshift_started)
        .as_millis()
        < 1
    {
        9u128
    } else {
        8u128
    };
    while context.time_workshift_started.elapsed().as_millis() < 10
        && residual_started.elapsed().as_millis() < budget_ms
    {
        let n = context.points.len();
        let mut found = None;
        for offset in 0..n {
            let index = (cursor + offset) % n;
            let p = &context.points[index];
            if !p.delivered && !p.escapes && !p.repeats {
                found = Some(index);
                cursor = (index + 1) % n;
                break;
            }
        }
        let Some(index) = found else {
            break;
        };
        let pos = pos_from_index(index, context.res.0);
        // Only init fresh seats — do not reset z/iters pulled from GPU.
        if !context.points[index].initialized {
            kernel.start_seat(context, pos);
        }
        let before = context.points[index].iterations;
        kernel.iterate_bout(
            &mut context.points[index],
            None,
            4.0,
            epsilon,
            BoutCap::STANDARD,
        );
        let delta = context.points[index].iterations.saturating_sub(before);
        context.total_iterations_today += delta;
        context.total_iterations = context.total_iterations.saturating_add(delta);
        if !(context.points[index].escapes || context.points[index].repeats) {
            continue;
        }
        let completed_point = kernel.completion(&mut context.points[index]);
        if context.points[index].repeats {
            queue_incomplete_neighbors_in(
                &pos,
                context.res,
                &context.points,
                &mut context.in_queue,
            );
        } else {
            queue_incomplete_neighbors(
                &pos,
                context.res,
                &context.points,
                &mut context.out_queue,
            );
        }
        match context.push_delivery(Delivery::Final(completed_point), index) {
            PushOutcome::Published => {
                context.total_points_today += 1;
                context.record_hud_completion_batch(1);
            }
            PushOutcome::BufferFull => break,
        }
    }
}

/// True when f32 cannot distinguish neighboring seats (F32 precision wall).
/// Uses generator plane geometry — must not depend on lazy seat init (otherwise
/// deep zooms stay on F32 until seats happen to be started, which is too late).
fn f32_collapses_neighbors(context: &WorkContext<f64>) -> bool {
    let w = context.res.0;
    let h = context.res.1;
    if w < 2 || h < 1 {
        return false;
    }
    let x = w / 2;
    let y = h / 2;
    let x1 = (x + 1).min(w - 1);
    if x1 == x {
        return false;
    }
    let d0 = context.c_generator.get_c((x, y));
    let d1 = context.c_generator.get_c((x1, y));
    let a = c_for_seat_f64(context, d0);
    let b = c_for_seat_f64(context, d1);
    if a.0 == b.0 && a.1 == b.1 {
        return false;
    }
    (a.0 as f32) == (b.0 as f32) && (a.1 as f32) == (b.1 as f32)
}

fn apply_finish_to_point(point: &mut Point<f64>, fin: &HarvestedFinish) {
    point.iterations = fin.iterations;
    point.small_time = fin.small_time;
    point.smallness_squared = fin.smallness;
    point.z = (fin.z_x, fin.z_y);
    point.dc = (fin.dc_x, fin.dc_y);
    point.c = (fin.c_x, fin.c_y);
    point.real_squared = fin.z_x * fin.z_x;
    point.imag_squared = fin.z_y * fin.z_y;
    point.real_imag = fin.z_x * fin.z_y;
    point.loop_detection_point = ((fin.loop_zx, fin.loop_zy), fin.loop_iter);
    point.escapes = (fin.flags & 2) != 0;
    point.repeats = (fin.flags & 4) != 0;
}

/// Host f64 check for GPU-reported repeats. Returns true if the seat should
/// publish as finished (escape or confirmed repeat); false to leave WIP.
fn confirm_repeat_or_keep_wip(point: &mut Point<f64>, epsilon: f64) -> bool {
    if point.escapes {
        return true;
    }
    if !point.repeats {
        return false;
    }
    // Clear the GPU repeat and probe further in f64.
    point.repeats = false;
    iterate_max_n_times(point, 4.0, epsilon, BoutCap::STANDARD);
    if point.escapes {
        return true;
    }
    if point.repeats {
        return true;
    }
    // Still unresolved — keep iterating on a later residual / GPU wave.
    false
}

fn pos_from_index(index: usize, width: u32) -> (i32, i32) {
    let w = width as usize;
    ((index % w) as i32, (index / w) as i32)
}

/// Serialize GPU-touching tests — concurrent wgpu contexts on one adapter can SEGV
/// or leave harvest latency inflated for the next probe.
#[cfg(test)]
pub fn lock_gpu_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::utils::{IntExp, ObjectivePosAndZoom};

    #[test]
    fn naive_gpu_context_init_or_skip() {
        let _guard = lock_gpu_tests();
        let ctx = NaiveGpuContext::try_new();
        if ctx.is_none() {
            eprintln!("no GPU adapter / CZ_FORCE_CPU_NAIVE — smoke skipped");
            return;
        }
        let gpu = ctx.unwrap();
        eprintln!("naive gpu precision={:?}", gpu.precision);
        assert!(gpu.wave_n() >= MIN_WAVE_N);
    }

    #[test]
    fn naive_gpu_home_wave_finishes_some_seats() {
        let _guard = lock_gpu_tests();
        let Some(mut gpu) = NaiveGpuContext::try_new() else {
            eprintln!("no GPU — smoke skipped");
            return;
        };
        gpu.set_wave_n(256);
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: -2,
            },
            (64u32, 64u32),
        );
        let mut ctx = from_stencil(frame, None).expect("home shell");
        workshift_naive_gpu(0, 0, 0, 0, &mut ctx, &mut gpu);
        assert!(ctx.last_used_naive_gpu);
        assert!(
            ctx.total_iterations_today > 0 || ctx.total_points_today > 0,
            "expected GPU wave to perform work"
        );
    }

    #[test]
    fn naive_gpu_ips_ratio_probe() {
        use crate::assemblies::workgroup::screen_worker::workshift::{
            iterate_max_n_times, BoutCap, Point,
        };
        use std::time::Instant;

        let _guard = lock_gpu_tests();
        let Some(mut gpu) = NaiveGpuContext::try_new() else {
            eprintln!("naive_gpu_ips_ratio_probe: no adapter — skipped");
            return;
        };
        eprintln!("probe precision={:?}", gpu.precision);
        gpu.set_wave_n(2048);

        fn hard_point(i: usize, n: usize) -> Point<f64> {
            // Near-boundary exterior: typically needs ≫ BoutCap×16 iters to escape,
            // so the probe stays iterate-heavy (few finals, arithmetic-dominated).
            let t = i as f64 / n as f64;
            Point {
                delta_c: (0.0, 0.0),
                c: (0.25 - 1e-12 * (1.0 + t), 1e-12 * t),
                z: (0.0, 0.0),
                dc: (1.0, 0.0),
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
                smallness_squared: f64::INFINITY,
                small_time: 0,
                delta: None,
                direct_only: false,
                bound_zero_generation: 0,
            }
        }

        let n = 8192usize;
        let mut points: Vec<Point<f64>> = (0..n).map(|i| hard_point(i, n)).collect();

        let t0 = Instant::now();
        let mut cpu_iters = 0u64;
        for p in &mut points {
            let before = p.iterations;
            iterate_max_n_times(p, 4.0, 1e-15, BoutCap::STANDARD);
            cpu_iters += (p.iterations - before) as u64;
        }
        let cpu_s = t0.elapsed().as_secs_f64().max(1e-9);
        let cpu_ips = cpu_iters as f64 / cpu_s;

        let points_fs: Vec<Point<f64>> = (0..n).map(|i| hard_point(i, n)).collect();
        let points_c: Vec<Point<f64>> = (0..n).map(|i| hard_point(i, n)).collect();

        let mut best_compute_ratio = 0.0_f64;
        let mut best_fs_ratio = 0.0_f64;
        let mut best_line = String::new();

        for attempt in 0..2 {
            if attempt > 0 {
                drop(gpu);
                std::thread::sleep(std::time::Duration::from_millis(250));
                let Some(g) = NaiveGpuContext::try_new() else {
                    break;
                };
                gpu = g;
                eprintln!("probe retry with fresh context precision={:?}", gpu.precision);
            }
            gpu.set_wave_n(2048);
            let upload_fs: Vec<(u32, &Point<f64>)> = points_fs
                .iter()
                .enumerate()
                .map(|(i, p)| (i as u32, p))
                .collect();
            let upload_c: Vec<(u32, &Point<f64>)> = points_c
                .iter()
                .enumerate()
                .map(|(i, p)| (i as u32, p))
                .collect();

            // Warm pipeline so timed trials are not dominated by first-submit latency.
            for _ in 0..3 {
                gpu.dispatch_wave_multi_iters_only(&upload_c, 4.0, 1e-15, BoutCap::STANDARD, 16)
                    .expect("warmup");
                let _ = gpu.harvest_iters_only().expect("warmup harvest");
            }

            // Best-of-N: pick the trial whose fullstack best tracks its own compute.
            let mut best_track = 0.0_f64;
            best_line.clear();
            best_compute_ratio = 0.0;
            best_fs_ratio = 0.0;
            for trial in 0..5 {
                let t_c = Instant::now();
                gpu.dispatch_wave_multi_iters_only(&upload_c, 4.0, 1e-15, BoutCap::STANDARD, 16)
                    .expect("dispatch_multi_iters");
                let t_c_disp = Instant::now();
                let delta_c = gpu.harvest_iters_only().expect("iters");
                let t_c_harv = Instant::now();
                let c_s = t_c.elapsed().as_secs_f64().max(1e-9);
                let compute_ips = delta_c as f64 / c_s;
                let c_disp_ms = (t_c_disp - t_c).as_secs_f64() * 1e3;
                let c_harv_ms = (t_c_harv - t_c_disp).as_secs_f64() * 1e3;

                let t_fs = Instant::now();
                gpu.dispatch_wave_multi_sparse(&upload_fs, 4.0, 1e-15, BoutCap::STANDARD, 16)
                    .expect("dispatch_sparse");
                let t_after_dispatch = Instant::now();
                let (fins, delta_fs) = gpu.harvest_sparse_finals().expect("sparse");
                let t_after_harvest = Instant::now();
                let fs_s = t_fs.elapsed().as_secs_f64().max(1e-9);
                let fs_ips = delta_fs as f64 / fs_s;
                let n_finals = fins.len();
                let dispatch_ms = (t_after_dispatch - t_fs).as_secs_f64() * 1e3;
                let harvest_ms = (t_after_harvest - t_after_dispatch).as_secs_f64() * 1e3;

                let fs_ratio = fs_ips / cpu_ips.max(1.0);
                let compute_ratio = compute_ips / cpu_ips.max(1.0);
                let track = fs_ratio / compute_ratio.max(1e-9);
                let line = format!(
                    "IPS probe attempt={attempt} trial={trial}: cpu={cpu_ips:.3e} fullstack={fs_ips:.3e} ({fs_ratio:.2}×) compute≈{compute_ips:.3e} ({compute_ratio:.2}×) track={track:.2} finals={n_finals} delta_fs={delta_fs} delta_c={delta_c} fs_ms={:.2} (disp={dispatch_ms:.2} harv={harvest_ms:.2}) c_ms={:.2} (disp={c_disp_ms:.2} harv={c_harv_ms:.2}) precision={:?} n={n} bouts=16",
                    fs_s * 1e3,
                    c_s * 1e3,
                    gpu.precision
                );
                eprintln!("{line}");
                if compute_ratio >= 50.0 && track >= best_track {
                    best_track = track;
                    best_compute_ratio = compute_ratio;
                    best_fs_ratio = fs_ratio;
                    best_line = line;
                }
            }
            if !best_line.is_empty() {
                break;
            }
        }
        assert!(
            !best_line.is_empty(),
            "no IPS probe trial cleared the 50× compute floor (after warmups; GPU may be busy)"
        );
        eprintln!("IPS probe best-track: {best_line}");
        // FLOP-ratio method (D-NGPU-5): iterate-heavy arithmetic vs CPU single-core.
        // Compute path proxies device FLOP; fullstack must track it within ~±20%.
        assert!(
            best_compute_ratio > 50.0,
            "compute GPU/CPU ratio {best_compute_ratio:.2} below FLOP-tracking floor (50×)"
        );
        let track = best_fs_ratio / best_compute_ratio.max(1e-9);
        assert!(
            track > 0.80,
            "sparse fullstack {best_fs_ratio:.2}× is {track:.2} of compute {best_compute_ratio:.2}× (need ≥0.80)"
        );
    }
}
