//! Dedicated wgpu compute island for naive Mandelbrot bouts.
//! Host owns scheduling; GPU runs BoutCap-bounded WIP waves; sparse finish harvest only.

mod buffers;
mod device;
mod kernel;

pub use device::{GpuPrecision, NaiveGpuContext};
pub use kernel::{HarvestedFinish, WipMeta};

use crate::assemblies::workgroup::screen_worker::workshift::{
    next_attention_spiral_pos, point_is_edge, queue_incomplete_neighbors,
    queue_incomplete_neighbors_in, queue_incomplete_neighbors_of_edge, refresh_active_gear,
    BoutCap, CompletedPoint, Delivery, DirectKernel, Motion, Point, PushOutcome, SeatKernel,
    Step, WorkContext,
};
use crate::utils::index_from_pos;
use std::collections::HashSet;
use std::time::Instant;

pub const DEFAULT_WAVE_N: u32 = 2048;
pub const MIN_WAVE_N: u32 = 64;

/// Claim the next undelivered seat using the same slot rotation as the CPU workshift.
pub fn claim_next_undelivered_seat(
    context: &mut WorkContext<f64>,
    skip: &HashSet<usize>,
) -> Option<((i32, i32), Step)> {
    let total = context.points.len().max(1);
    for _ in 0..total.min(8192) {
        let (pos, step) = select_candidate(context)?;
        let index = index_from_pos(&pos, context.res.0);
        if context.points[index].delivered || skip.contains(&index) {
            advance_past(context, step);
            continue;
        }
        claim(context, step, pos);
        return Some((pos, step));
    }
    None
}

fn select_candidate(context: &mut WorkContext<f64>) -> Option<((i32, i32), Step)> {
    match context.workshifts % 5 {
        0 => {
            if context.motion == Motion::Panned && context.workshifts == 0 {
                if let Some(p) = crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
                    context, true,
                ) {
                    return Some(p);
                }
                if let Some(pos) = context.attention_current {
                    return Some((pos, Step::Attention));
                }
                if let Some(pos) = next_attention_spiral_pos(context) {
                    return Some((pos, Step::Attention));
                }
                return crate::assemblies::workgroup::screen_worker::workshift::queue_fallback_pos_pub(
                    context, true,
                );
            }
            if let Some(pos) = context.attention_current {
                return Some((pos, Step::Attention));
            }
            if let Some(pos) = next_attention_spiral_pos(context) {
                return Some((pos, Step::Attention));
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

/// GPU naive workshift: arm WIP waves, BoutCap dispatch, sparse harvest into Delivery.
pub fn workshift_naive_gpu(
    _day_token_allowance: u32,
    _iteration_token_cost: u32,
    _point_token_cost: u32,
    _bout_token_cost: u32,
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
    context.last_used_naive_gpu = true;

    let epsilon = context.pitch_epsilon;
    let kernel = DirectKernel;
    let mut wave_n = gpu.wave_n();
    let mut wip: Vec<WipMeta> = Vec::new();
    let mut skip: HashSet<usize> = HashSet::new();

    while context.time_workshift_started.elapsed().as_millis() < 10 {
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

        let upload: Vec<(u32, &Point<f64>)> = wip
            .iter()
            .map(|m| (m.index as u32, &context.points[m.index]))
            .collect();

        // Re-borrow after collecting indices — upload needs points; do owned snapshot.
        let owned: Vec<(u32, Point<f64>)> = wip
            .iter()
            .map(|m| (m.index as u32, context.points[m.index].clone()))
            .collect();
        let upload_refs: Vec<(u32, &Point<f64>)> =
            owned.iter().map(|(i, p)| (*i, p)).collect();

        if let Err(e) = gpu.dispatch_wave(&upload_refs, 4.0, epsilon, BoutCap::STANDARD) {
            eprintln!("naive_gpu dispatch failed: {e}");
            break;
        }

        let (finishes, iter_delta) = match gpu.harvest_finishes() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("naive_gpu harvest failed: {e}");
                break;
            }
        };
        let _ = upload;
        context.total_iterations_today += iter_delta;
        context.total_iterations = context.total_iterations.saturating_add(iter_delta);

        let finished_indices: HashSet<usize> =
            finishes.iter().map(|f| f.seat_index as usize).collect();

        let mut buffer_full = false;
        for fin in &finishes {
            let index = fin.seat_index as usize;
            if index >= context.points.len() {
                continue;
            }
            let meta = wip.iter().find(|m| m.index == index).cloned();
            apply_finish_to_point(&mut context.points[index], fin);
            let pos = meta
                .as_ref()
                .map(|m| m.pos)
                .unwrap_or_else(|| pos_from_index(index, context.res.0));
            let step = meta.as_ref().map(|m| m.step).unwrap_or(Step::Out);

            if !(context.points[index].repeats || context.points[index].escapes) {
                continue;
            }
            if matches!(step, Step::Attention) {
                context.attention_current = None;
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
                    context.record_hud_completion_batch(1);
                    skip.remove(&index);
                }
                PushOutcome::BufferFull => {
                    skip.remove(&index);
                    buffer_full = true;
                    break;
                }
            }
        }
        if buffer_full {
            break;
        }

        let mut next_wip = Vec::new();
        for m in wip.drain(..) {
            if finished_indices.contains(&m.index) {
                continue;
            }
            match m.step {
                Step::Out => {
                    context
                        .out_queue
                        .push_back((m.pos, context.points[m.index].iterations));
                    skip.remove(&m.index);
                }
                Step::Scredge => {
                    let provisional = CompletedPoint::Repeats {
                        period: 0,
                        smallness: context.points[m.index].smallness_squared,
                        small_time: context.points[m.index].small_time,
                    };
                    match context.push_delivery(Delivery::Provisional(provisional), m.index) {
                        PushOutcome::Published => {
                            context.scredge_poses.push_back(m.pos);
                            skip.remove(&m.index);
                        }
                        PushOutcome::BufferFull => next_wip.push(m),
                    }
                }
                Step::Attention | Step::In | Step::Edge => next_wip.push(m),
            }
        }
        wip = next_wip;
        context.total_bouts_today += 1;

        if context.time_workshift_started.elapsed().as_millis() > 8 && wave_n > MIN_WAVE_N {
            wave_n = (wave_n / 2).max(MIN_WAVE_N);
            gpu.set_wave_n(wave_n);
        }
    }

    gpu.end_shift_keep_generation();
    context.workshifts += 1;
    let delivered = context.points.iter().filter(|p| p.delivered).count();
    let total_points = context.points.len().max(1);
    context.percent_completed = delivered as f64 / (total_points as f64) * 100.0;
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
    point.escapes = (fin.flags & 2) != 0;
    point.repeats = (fin.flags & 4) != 0;
}

fn pos_from_index(index: usize, width: u32) -> (i32, i32) {
    let w = width as usize;
    ((index % w) as i32, (index / w) as i32)
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::utils::{IntExp, ObjectivePosAndZoom};

    #[test]
    fn naive_gpu_context_init_or_skip() {
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
}

