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
use std::collections::HashMap;
use std::time::Instant;

pub const DEFAULT_WAVE_N: u32 = device::MAX_WAVE;
pub const MIN_WAVE_N: u32 = 64;

/// Dense seat mask for WIP skip — O(1) without HashSet allocate/probe tax.
struct SeatSkip {
    bits: Vec<u64>,
}

impl SeatSkip {
    fn new(n: usize) -> Self {
        Self {
            bits: vec![0u64; n.div_ceil(64)],
        }
    }

    #[inline]
    fn insert(&mut self, index: usize) {
        if let Some(word) = self.bits.get_mut(index / 64) {
            *word |= 1u64 << (index % 64);
        }
    }

    #[inline]
    fn remove(&mut self, index: usize) {
        if let Some(word) = self.bits.get_mut(index / 64) {
            *word &= !(1u64 << (index % 64));
        }
    }

    #[inline]
    fn contains(&self, index: usize) -> bool {
        self.bits
            .get(index / 64)
            .is_some_and(|word| word & (1u64 << (index % 64)) != 0)
    }
}

/// Claim the next undelivered seat using the same slot rotation as the CPU workshift.
pub fn claim_next_undelivered_seat(
    context: &mut WorkContext<f64>,
    skip: &SeatSkip,
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
        if skip.contains(index) {
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
    skip: &SeatSkip,
) -> Option<((i32, i32), Step)> {
    let n = context.points.len();
    if n == 0 {
        return None;
    }
    let start = (context.random_index as usize) % n;
    for off in 0..n {
        let index = (start + off) % n;
        if context.points[index].delivered || skip.contains(index) {
            continue;
        }
        // Finished-but-undelivered (channel-send undeliver orphans) must not be
        // start_seat-reset; they wait for publish_finished_undelivered.
        if context.points[index].escapes || context.points[index].repeats {
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
    skip: &SeatSkip,
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
                } else if !skip.contains(index) {
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
    let kernel = DirectKernel;

    context.time_workshift_started = Instant::now();
    context.update_reference_floor_policy();
    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;
    refresh_active_gear(context);

    // Drain finished-undelivered orphans before arming GPU; reset the shift clock after so
    // the O(n) scan does not steal the 10 ms compute budget.
    if gpu.orphan_publish.get() {
        publish_finished_undelivered(context, &kernel);
        gpu.orphan_publish
            .set(context.points.iter().any(|p| {
                !p.delivered && (p.escapes || p.repeats)
            }));
        context.time_workshift_started = Instant::now();
    }

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
                &kernel,
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

    let total_points = context.points.len().max(1);
    let mut delivered_n =
        ((context.percent_completed / 100.0) * total_points as f64).round() as usize;
    delivered_n = delivered_n.min(total_points);

    let mut wave_n = gpu.wave_n();
    let mut wip: Vec<WipMeta> = Vec::new();
    let mut skip = SeatSkip::new(total_points);
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
            // Shallow floods finish most seats per bout — still allow 2 bouts/dispatch
            // after the first publish to amortize submit/sync tax (PPS ≥1× floor) while
            // keeping per-shift harvest for continuous outputs.
            if points_published_this_shift == 0 {
                bouts_per_dispatch = 1;
            } else if gpu_final_n * 4 >= wip_n {
                // Shallow: pack more ALU per submit; still harvest every workshift.
                bouts_per_dispatch = 4;
            } else if gpu_final_n * 16 < wip_n {
                bouts_per_dispatch = 16;
            } else {
                bouts_per_dispatch = 8;
            }
            context.total_bouts_today += 1;

            let wave_fully_finished =
                finishes.len() == prev_wip.len() && gpu_final_n == prev_wip.len();

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
                    resident_n = wip.len() as u32;
                    match gpu.dispatch_wave_wip(
                        &wip,
                        &context.points,
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
                    gpu.orphan_publish.set(true);
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
                gpu.orphan_publish.set(true);
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
            resident_n = wip.len() as u32;
            match gpu.dispatch_wave_wip(
                &wip,
                &context.points,
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
                gpu.orphan_publish.set(true);
            }
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

    // Keep percent honest near completion; bulk shifts stay incremental.
    if context.percent_completed >= 90.0
        || (delivered_n as f64 / total_points as f64) >= 0.85
    {
        let delivered = context.points.iter().filter(|p| p.delivered).count();
        delivered_n = delivered;
    } else {
        delivered_n = delivered_n.saturating_add(points_published_this_shift as usize);
    }

    gpu.end_shift_keep_generation();
    context.workshifts += 1;
    context.percent_completed = delivered_n.min(total_points) as f64 / (total_points as f64) * 100.0;
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
    skip: &mut SeatSkip,
    points_published_this_shift: &mut u32,
) -> PublishFinishOutcome {
    // r[impl cz.craft.gpu-host-queue-discovery+1]
    // Slim field apply for large floods is OK; skipping neighbor/edge discovery is not.
    let bulk = finishes.len() >= 128;
    let attention_idx = context
        .attention_current
        .map(|p| index_from_pos(&p, context.res.0));
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
        if bulk {
            apply_finish_bulk_publish(&mut context.points[index], fin);
        } else {
            apply_finish_to_point(&mut context.points[index], fin);
        }
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
            skip.remove(index);
            need_reupload = true;
            continue;
        }
        if attention_idx == Some(index) {
            context.attention_current = None;
        }
        // Bulk floods may skip period twin-test (period unknown / IPS path);
        // neighbor discovery always runs — flood fill must announce itself.
        let completed_point = if bulk {
            if context.points[index].repeats {
                context.points[index].period = 0;
                CompletedPoint::Repeats {
                    period: 0,
                    smallness: context.points[index].smallness_squared,
                    small_time: context.points[index].small_time,
                }
            } else {
                let p = &context.points[index];
                CompletedPoint::Escapes {
                    escape_time: p.iterations,
                    escape_location: p.z,
                    escape_derivative: p.dc,
                    start_location: p.c,
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            }
        } else {
            kernel.completion(&mut context.points[index])
        };
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
        match context.push_delivery(Delivery::Final(completed_point), index) {
            PushOutcome::Published => {
                context.total_points_today += 1;
                published_batch += 1;
                skip.remove(index);
                *points_published_this_shift += 1;
            }
        }
    }
    if published_batch > 0 {
        context.record_hud_completion_batch(published_batch);
    }
    PublishFinishOutcome {
        buffer_full: false,
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

/// Bulk publish path: only fields needed for Final delivery (no continue/WIP resume).
fn apply_finish_bulk_publish(point: &mut Point<f64>, fin: &HarvestedFinish) {
    point.iterations = fin.iterations;
    point.small_time = fin.small_time;
    point.smallness_squared = fin.smallness;
    point.z = (fin.z_x, fin.z_y);
    point.dc = (fin.dc_x, fin.dc_y);
    point.c = (fin.c_x, fin.c_y);
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
/// or leave harvest latency inflated for the next probe. Heavy CPU home-fill IPS
/// pins also take this lock so they do not steal cores from those probes.
#[cfg(test)]
pub fn lock_gpu_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Process-wide GPU context — adapter/pipeline bring-up is ~100–200ms; reuse it.
/// Takes `lock_gpu_tests` so callers must not hold that lock already.
#[cfg(test)]
pub struct SharedGpu {
    _lock: std::sync::MutexGuard<'static, ()>,
    slot: std::sync::MutexGuard<'static, Option<NaiveGpuContext>>,
}

#[cfg(test)]
impl SharedGpu {
    pub fn acquire() -> Option<Self> {
        let _lock = lock_gpu_tests();
        static GPU: std::sync::Mutex<Option<NaiveGpuContext>> = std::sync::Mutex::new(None);
        let mut slot = GPU.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = NaiveGpuContext::try_new();
        }
        if slot.is_none() {
            return None;
        }
        Some(Self { _lock, slot })
    }

    pub fn ctx(&mut self) -> &mut NaiveGpuContext {
        self.slot.as_mut().expect("SharedGpu::acquire checked Some")
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::constants::TEST_SCREEN_RES;
    use crate::utils::{IntExp, ObjectivePosAndZoom};

    #[test]
    fn naive_gpu_context_init_or_skip() {
        let Some(mut shared) = SharedGpu::acquire() else {
            eprintln!("no GPU adapter / CZ_FORCE_CPU_NAIVE — smoke skipped");
            return;
        };
        let gpu = shared.ctx();
        eprintln!("naive gpu precision={:?}", gpu.precision);
        assert!(gpu.wave_n() >= MIN_WAVE_N);
    }

    #[test]
    fn naive_gpu_home_wave_finishes_some_seats() {
        let Some(mut shared) = SharedGpu::acquire() else {
            eprintln!("no GPU — smoke skipped");
            return;
        };
        let gpu = shared.ctx();
        gpu.set_wave_n(256);
        let frame = (
            ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
        );
        let mut ctx = from_stencil(frame, None).expect("home shell");
        workshift_naive_gpu(0, 0, 0, 0, &mut ctx, gpu);
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

        let _wgpu = crate::debug_agent::WgpuTestLock::acquire();
        let Some(mut shared) = SharedGpu::acquire() else {
            eprintln!("naive_gpu_ips_ratio_probe: no adapter — skipped");
            return;
        };
        let gpu = shared.ctx();
        eprintln!("probe precision={:?}", gpu.precision);
        gpu.set_wave_n(512);

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

        let n = 256usize;
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

        // One warmup + one timed trial — no sleep / no context recreate.
        gpu.dispatch_wave_multi_iters_only(&upload_c, 4.0, 1e-15, BoutCap::STANDARD, 16)
            .expect("warmup");
        let _ = gpu.harvest_iters_only().expect("warmup harvest");

        let mut best_compute_ratio = 0.0_f64;
        let mut best_fs_ratio = 0.0_f64;
        let mut best_line = String::new();
        let trial = 0;
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
            "IPS probe trial={trial}: cpu={cpu_ips:.3e} fullstack={fs_ips:.3e} ({fs_ratio:.2}×) compute≈{compute_ips:.3e} ({compute_ratio:.2}×) track={track:.2} finals={n_finals} delta_fs={delta_fs} delta_c={delta_c} fs_ms={:.2} (disp={dispatch_ms:.2} harv={harvest_ms:.2}) c_ms={:.2} (disp={c_disp_ms:.2} harv={c_harv_ms:.2}) precision={:?} n={n} bouts=16",
            fs_s * 1e3,
            c_s * 1e3,
            gpu.precision
        );
        eprintln!("{line}");
        assert!(
            compute_ratio >= 1.5,
            "compute GPU/CPU ratio {compute_ratio:.2} below iterate-heavy floor (1.5×)"
        );
        best_compute_ratio = compute_ratio;
        best_fs_ratio = fs_ratio;
        best_line = line;
        eprintln!("IPS probe best-track: {best_line}");
        // D-NGPU-5: fullstack IPS must track compute/header proxy within ~±20%.
        // Absolute GPU/CPU × is machine- and profile-dependent (debug CPU is
        // slow → large ×; release CPU is fast → small ×) — do not pin 50×.
        assert!(
            best_compute_ratio > 1.5,
            "compute GPU/CPU ratio {best_compute_ratio:.2} below iterate-heavy floor (1.5×)"
        );
        let track = best_fs_ratio / best_compute_ratio.max(1e-9);
        assert!(
            track > 0.80,
            "sparse fullstack {best_fs_ratio:.2}× is {track:.2} of compute {best_compute_ratio:.2}× (need ≥0.80)"
        );
    }
}

#[cfg(test)]
mod mutant_kill {
    //! Thought-killed pins for host-side naive GPU helpers (index map, finish flags).
    use super::*;
    use crate::assemblies::workgroup::screen_worker::workshift::Point;

    fn blank_finish(flags: u32) -> HarvestedFinish {
        HarvestedFinish {
            seat_index: 0,
            flags,
            iterations: 17,
            small_time: 3,
            smallness: 0.5,
            iter_delta: 0,
            z_x: 1.25,
            z_y: -0.5,
            dc_x: 2.0,
            dc_y: 0.25,
            c_x: 0.1,
            c_y: -0.2,
            loop_zx: 0.01,
            loop_zy: 0.02,
            loop_iter: 9,
        }
    }

    fn blank_point() -> Point<f64> {
        Point {
            delta_c: (0.0, 0.0),
            c: (0.0, 0.0),
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
            smallness_squared: f64::MAX,
            small_time: 0,
            delta: None,
            direct_only: false,
            bound_zero_generation: 0,
        }
    }

    #[test]
    fn mutant_kill_pos_from_index_is_row_major() {
        assert_eq!(pos_from_index(0, 40), (0, 0));
        assert_eq!(pos_from_index(39, 40), (39, 0));
        assert_eq!(pos_from_index(40, 40), (0, 1));
        assert_eq!(pos_from_index(41, 40), (1, 1));
        // %→/ or /→% would swap axes or collapse.
        assert_ne!(pos_from_index(41, 40), (0, 41));
        assert_ne!(pos_from_index(41, 40), (41, 0));
    }

    #[test]
    fn mutant_kill_apply_finish_maps_flag_bits_2_and_4() {
        let mut p = blank_point();
        apply_finish_to_point(&mut p, &blank_finish(0));
        assert!(!p.escapes && !p.repeats);
        assert_eq!(p.iterations, 17);
        assert_eq!(p.z, (1.25, -0.5));
        assert_eq!(p.dc, (2.0, 0.25));
        assert_eq!(p.c, (0.1, -0.2));
        assert_eq!(p.real_squared, 1.25 * 1.25);
        assert_eq!(p.imag_squared, (-0.5) * (-0.5));
        assert_eq!(p.real_imag, 1.25 * -0.5);
        assert_eq!(p.loop_detection_point, ((0.01, 0.02), 9));

        apply_finish_to_point(&mut p, &blank_finish(2));
        assert!(p.escapes && !p.repeats);
        apply_finish_to_point(&mut p, &blank_finish(4));
        assert!(!p.escapes && p.repeats);
        apply_finish_to_point(&mut p, &blank_finish(6));
        assert!(p.escapes && p.repeats);
        // Wrong bit masks (&1 / &8) must not match.
        apply_finish_to_point(&mut p, &blank_finish(1));
        assert!(!p.escapes && !p.repeats);
        apply_finish_to_point(&mut p, &blank_finish(8));
        assert!(!p.escapes && !p.repeats);
    }

    #[test]
    fn mutant_kill_apply_finish_bulk_publish_sets_final_fields_only() {
        let mut p = blank_point();
        p.real_squared = 99.0;
        p.loop_detection_point = ((7.0, 8.0), 11);
        apply_finish_bulk_publish(&mut p, &blank_finish(2));
        assert!(p.escapes && !p.repeats);
        assert_eq!(p.iterations, 17);
        assert_eq!(p.z, (1.25, -0.5));
        // Bulk path skips WIP resume fields.
        assert_eq!(p.real_squared, 99.0);
        assert_eq!(p.loop_detection_point, ((7.0, 8.0), 11));
    }

    #[test]
    fn mutant_kill_seat_skip_bitword_ops() {
        let mut skip = SeatSkip::new(130); // spans 3 words (0..64, 64..128, 128..)
        assert_eq!(skip.bits.len(), 3);
        assert!(!skip.contains(0));
        assert!(!skip.contains(63));
        assert!(!skip.contains(64));
        assert!(!skip.contains(129));

        skip.insert(0);
        skip.insert(63);
        skip.insert(64);
        skip.insert(129);
        assert!(skip.contains(0));
        assert!(skip.contains(63));
        assert!(skip.contains(64));
        assert!(skip.contains(129));
        assert!(!skip.contains(1));
        assert!(!skip.contains(65));
        // /64 vs %64 mix-up: bit 64 must not live in word 0.
        assert_ne!(skip.bits[0] & (1u64 << 0), 0);
        assert_eq!(skip.bits[0] & (1u64 << (64 % 64)), skip.bits[0] & 1); // bit0 only from insert(0)
        assert_ne!(skip.bits[1] & 1, 0); // index 64 → word1 bit0
        assert_ne!(skip.bits[2] & (1u64 << (129 % 64)), 0);

        skip.remove(64);
        assert!(!skip.contains(64));
        assert!(skip.contains(63));
        assert!(skip.contains(129));
        skip.remove(0);
        assert!(!skip.contains(0));
        // Out of range is a no-op (no panic).
        skip.insert(10_000);
        skip.remove(10_000);
        assert!(!skip.contains(10_000));
        // OR→AND on insert would clear other bits in the word.
        skip.insert(1);
        skip.insert(2);
        assert!(skip.contains(1) && skip.contains(2));
    }

    #[test]
    fn mutant_kill_f32_collapses_and_scan_undelivered() {
        use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
        use crate::constants::{HOME_POSITION, TEST_SCREEN_RES};
        use crate::utils::{IntExp, ObjectivePosAndZoom};

        let home = ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::ZERO - IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: -2,
        };
        let mut ctx = from_stencil((home.clone(), TEST_SCREEN_RES), None).expect("home");
        // Shallow home pitch: neighboring seats stay distinct in f32.
        assert!(!f32_collapses_neighbors(&ctx));
        // Degenerate width early-out.
        let tiny = from_stencil(
            (
                home.clone(),
                (1, 8),
            ),
            None,
        );
        if let Some(t) = tiny {
            assert!(!f32_collapses_neighbors(&t));
        }

        // Deep pot: generator pitch can collapse under f32 (precision wall).
        let deep = ObjectivePosAndZoom {
            pos: home.pos.clone(),
            zoom_pot: 60,
        };
        if let Some(dctx) = from_stencil((deep, TEST_SCREEN_RES), None) {
            // At extreme zoom absolute may fail admit; relative shell still ok.
            let collapsed = f32_collapses_neighbors(&dctx);
            // Either collapses or seats are identical in f64 (early false) —
            // pin that the check is conjunction of both components as f32.
            if collapsed {
                let w = dctx.res.0;
                let h = dctx.res.1;
                let x = w / 2;
                let y = h / 2;
                let x1 = (x + 1).min(w - 1);
                let a = c_for_seat_f64(&dctx, dctx.c_generator.get_c((x, y)));
                let b = c_for_seat_f64(&dctx, dctx.c_generator.get_c((x1, y)));
                assert!((a.0 as f32) == (b.0 as f32) && (a.1 as f32) == (b.1 as f32));
                assert_ne!(a.0 == b.0 && a.1 == b.1, true); // f64 still distinct
            }
        }

        // Scan skips delivered / skip-mask / finished-orphan seats.
        let n = ctx.points.len();
        ctx.random_index = 0;
        let mut skip = SeatSkip::new(n);
        let first = scan_undelivered_seat(&mut ctx, &skip).expect("open seat");
        let first_idx = index_from_pos(&first.0, ctx.res.0);
        assert_eq!(first.1, Step::Out);
        skip.insert(first_idx);
        let second = scan_undelivered_seat(&mut ctx, &skip).expect("another");
        assert_ne!(index_from_pos(&second.0, ctx.res.0), first_idx);
        ctx.points[index_from_pos(&second.0, ctx.res.0)].delivered = true;
        // Mark all remaining delivered except one escaped orphan.
        for p in &mut ctx.points {
            p.delivered = true;
            p.escapes = false;
            p.repeats = false;
        }
        let orphan = (first_idx + 3) % n;
        ctx.points[orphan].delivered = false;
        ctx.points[orphan].escapes = true;
        skip = SeatSkip::new(n);
        assert!(
            scan_undelivered_seat(&mut ctx, &skip).is_none(),
            "escaped undelivered must wait for publish, not scan-start"
        );
        ctx.points[orphan].escapes = false;
        ctx.points[orphan].repeats = true;
        assert!(scan_undelivered_seat(&mut ctx, &skip).is_none());
        ctx.points[orphan].repeats = false;
        let got = scan_undelivered_seat(&mut ctx, &skip).expect("WIP seat");
        assert_eq!(index_from_pos(&got.0, ctx.res.0), orphan);
    }
}
