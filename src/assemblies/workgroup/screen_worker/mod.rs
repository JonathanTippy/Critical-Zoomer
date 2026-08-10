use std::cmp::min;
use steady_state::*;
use crate::assemblies::headgroup::window::sampling::{index_from_relative_location, relative_location_i32_row_and_seat, transform_relative_location_i32};
use crate::assemblies::workgroup::c_generator::Mandelbrotable;
use crate::assemblies::workgroup::reference_worker::{
    select_reference_request, PublishedReference, ReferenceRequest,
};
use crate::delta_gear::ComputeGear;
use crate::utils::ObjectivePosAndZoom;
//use crate::actor::work_collector::*;
use crate::assemblies::workgroup::work_controller::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;

pub mod workshift;
pub mod perturb_kernel;
pub mod perturb_floatexp;
pub mod naive_gpu;

#[cfg(test)]
mod craftsmanship_tests;

pub struct WorkUpdate<T> {
    pub frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    pub completed_points: (Vec<(CompletedPoint<T>, usize)>),
    /// Aggregate active compute gear for HUD.
    // r[impl cz.depth.gear-hud+2]
    pub active_gear: crate::delta_gear::ComputeGear,
    pub host_stack: crate::assemblies::structs::HostStack,
    pub kernel_mode: crate::assemblies::structs::KernelMode,
    pub reference_status: crate::assemblies::structs::ReferenceStatus,
    /// Iterations performed since the previous update.
    pub iterations_delta: u64,
}

pub struct WorkerState<T: Mandelbrotable> {
    /// At most one live render target. `LiveTarget` structurally pairs the
    /// context with its `frame_info`, so a second live target cannot exist
    /// without a deliberate second `LiveTarget` value.
    // r[impl cz.craft.stencil-only-replace+2]
    work_context: Option<LiveTarget<T>>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
    , total_workshifts: u32
    // Held when a reference arrives before the first Replace; installed into
    // the live context as soon as one exists.
    , pending_reference: Option<std::sync::Arc<PublishedReference>>
    // Optional Naive GPU compute island; None → CPU DirectKernel for naive.
    , naive_gpu: Option<naive_gpu::NaiveGpuContext>
    // Latest debug manual-gear override from settings (`None` = auto policy).
    , manual_gear: Option<crate::assemblies::structs::KernelMode>
}

/// The single live render target: a work context and the frame_info it was
/// built from. The pairing is structural — the two can never be set or cleared
/// independently.
#[derive(Clone)]
pub struct LiveTarget<T: Mandelbrotable> {
    pub context: WorkContext<T>,
    pub frame_info: (ObjectivePosAndZoom, (u32, u32)),
}

/// Reopen seats delivered against an older reference generation (craftsmanship tests).
// r[impl cz.depth.reference-generation-restart+1]
pub fn invalidate_stale_deliveries<T: Mandelbrotable>(
    ctx: &mut WorkContext<T>,
    new_generation: u64,
) {
    for p in &mut ctx.points {
        if !p.delivered {
            continue;
        }
        let stale = p
            .delta
            .as_ref()
            .map(|d| d.generation != new_generation)
            .unwrap_or(true);
        if stale {
            p.delivered = false;
            p.initialized = false;
            p.delta = None;
            p.direct_only = false;
        }
    }
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate<f64>>,
    attention_in: SteadyRx<Option<(i32, i32)>>,
    reference_requests_out: SteadyTx<ReferenceRequest>,
    references_in: SteadyRx<PublishedReference>,
    settings_in: SteadyRx<crate::settings::Settings>,
    state: SteadyState<WorkerState<f64>>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight(
            [&commands_in, &attention_in, &references_in, &settings_in],
            [&updates_out, &reference_requests_out],
        ),
        commands_in,
        updates_out,
        attention_in,
        reference_requests_out,
        references_in,
        settings_in,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate<f64>>,
    attention_in: SteadyRx<Option<(i32, i32)>>,
    reference_requests_out: SteadyTx<ReferenceRequest>,
    references_in: SteadyRx<PublishedReference>,
    settings_in: SteadyRx<crate::settings::Settings>,
    state: SteadyState<WorkerState<f64>>,
) -> Result<(), Box<dyn Error>> {

    //actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;
    let mut attention_in = attention_in.lock().await;
    let mut reference_requests_out = reference_requests_out.lock().await;
    let mut references_in = references_in.lock().await;
    let mut settings_in = settings_in.lock().await;

    // Init wgpu off the async executor — pollster::block_on nested in async can fail.
    let mut naive_gpu = std::thread::Builder::new()
        .name("cz-naive-gpu-init".into())
        .spawn(|| naive_gpu::NaiveGpuContext::try_new())
        .ok()
        .and_then(|h| h.join().ok())
        .flatten();
    if naive_gpu.is_some() {
        eprintln!("screen_worker: NaiveGpuContext ready");
    } else {
        eprintln!("screen_worker: NaiveGpu unavailable; CPU DirectKernel for naive");
    }

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , workshift_token_budget: 16000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
        , total_workshifts: 0
        , pending_reference: None
        , naive_gpu: None
        , manual_gear: None
    }).await;
    // Inject after lock so a pre-existing empty SteadyState cannot drop the device.
    if state.naive_gpu.is_none() {
        state.naive_gpu = naive_gpu.take();
    }

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        // r[impl cz.craft.load-proportional-ignorance+1]
        let working = match &state.work_context {
            Some(live) => {live.context.percent_completed < 100.0}
            , None => {false}
        };

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut commands_in, 1),
            );
        }

        if actor.avail_units(&mut settings_in) > 0 {
            while actor.avail_units(&mut settings_in) > 1 {
                let _ = actor.try_take(&mut settings_in).expect("internal error");
            }
            if let Some(settings) = actor.try_take(&mut settings_in) {
                state.manual_gear = settings.manual_gear_override();
                let gear = state.manual_gear;
                if let Some(live) = &mut state.work_context {
                    live.context.manual_gear = gear;
                }
            }
        }

        if actor.avail_units(&mut attention_in) > 0 {
            while actor.avail_units(&mut attention_in) > 1 {
                let stuff = actor.try_take(&mut attention_in).expect("internal error");
                drop(stuff);
            };
            let attention = actor.try_take(&mut attention_in).expect("internal error");
            if let Some(live) = &mut state.work_context {
                set_attention(&mut live.context, attention);
            }
        }

        if actor.avail_units(&mut references_in) > 0 {
            while actor.avail_units(&mut references_in) > 1 {
                drop(actor.try_take(&mut references_in).expect("published reference"));
            }
            let newest = std::sync::Arc::new(
                actor.try_take(&mut references_in).expect("newest published reference"),
            );
            // Interior refs are preferred. Escaped refs are still installed on
            // relative (hard-bump) shells so the generator stays orbit-relative;
            // rejecting them left deep exterior on zero-orbit with blocky f64 c.
            let install = !newest.orbit.escaped
                || state
                    .work_context
                    .as_ref()
                    .is_some_and(|live| live.context.coords_are_relative);
            if install {
                state.pending_reference = Some(newest.clone());
                if let Some(live) = &mut state.work_context {
                    let zoom_pot = live.frame_info.0.zoom_pot;
                    let keep_longer_bootstrap = newest.orbit.escaped
                        && live.context.coords_are_relative
                        && live.context.latest_reference.as_ref().is_some_and(|cur| {
                            cur.orbit.iterates.len() > newest.orbit.iterates.len()
                        });
                    if keep_longer_bootstrap {
                    } else {
                    live.context.latest_reference = Some(newest.clone());
                    let compute_loc = (
                        live.frame_info.0.pos.0.clone(),
                        crate::utils::IntExp::ZERO - live.frame_info.0.pos.1.clone(),
                    );
                    rebuild_generator_for_reference(
                        &mut live.context,
                        &compute_loc,
                        live.frame_info.0.zoom_pot as i64,
                        live.frame_info.1,
                        newest.as_ref(),
                    );
                    }
                }
            } else {
            }
        }

        if actor.avail_units(&mut commands_in) > 0 {

            // r[impl cz.craft.drain-to-newest+1]
            while actor.avail_units(&mut commands_in) > 1 {
                let stuff = actor.try_take(&mut commands_in).expect("internal error");
                drop(stuff);
            };

            match actor.try_take(&mut commands_in).unwrap() {

                WorkerCommand::Replace{frame_info} => {
                    // r[impl cz.craft.pivot-two-message-order+1]
                    // r[impl cz.craft.stencil-only-replace+2]
                    let request = select_reference_request(
                        state
                            .work_context
                            .as_ref()
                            .map(|live| (&live.context, &live.frame_info)),
                        &frame_info,
                    );
                    actor.try_send(&mut reference_requests_out, request);

                    let previous = state.work_context.take();
                    if let Some(gpu) = state.naive_gpu.as_mut() {
                        gpu.bump_generation();
                    }
                    let previous_for_shell = match previous {
                        Some(mut live) => {
                            let old_zoom = live.frame_info.0.clone();
                            let iters = live.context.total_iterations_today;
                            let U = work_update(&mut live.context);
                            if U.len() > 0 {
                                actor.try_send(
                                    &mut updates_out,
                                    telemetry_update(None, U, Some(&mut live.context), iters as u64),
                                );
                            }
                            Some((live.context, old_zoom))
                        }
                        None => None,
                    };

                    if let Some(mut new_ctx) = from_stencil(frame_info.clone(), previous_for_shell) {
                        new_ctx.manual_gear = state.manual_gear;
                        if let Some(pending) = state.pending_reference.clone() {
                            // r[impl cz.depth.reference-coverage+1]
                            if crate::assemblies::workgroup::reference_worker::reference_c_covers_frame(
                                &pending.c,
                                &frame_info,
                            ) {
                                new_ctx.latest_reference = Some(pending.clone());
                                let compute_loc = (
                                    frame_info.0.pos.0.clone(),
                                    crate::utils::IntExp::ZERO - frame_info.0.pos.1.clone(),
                                );
                                rebuild_generator_for_reference(
                                    &mut new_ctx,
                                    &compute_loc,
                                    frame_info.0.zoom_pot as i64,
                                    frame_info.1,
                                    pending.as_ref(),
                                );
                            } else {
                                // Uncovered sticky refs cause classic glitch blobs when
                                // zooming into hard areas. Drop pending only — keep any
                                // view-center bootstrap from_stencil already installed.
                                state.pending_reference = None;
                            }
                        }
                        state.work_context = Some(LiveTarget { context: new_ctx, frame_info: frame_info.clone() });
                        actor.try_send(
                            &mut updates_out,
                            telemetry_update(
                                Some(frame_info),
                                vec!(),
                                state.work_context.as_mut().map(|l| &mut l.context),
                                0,
                            ),
                        );
                    }
                }
            }
        }

        let token_budget = state.workshift_token_budget.clone();
        let iteration_token_cost = state.iteration_token_cost.clone();
        let bout_token_cost = state.bout_token_cost.clone();
        let point_token_cost = state.point_token_cost.clone();
        

        let mut iters_delta = 0u64;
        // Split borrows: take GPU handle, then mutate live context.
        let mut gpu = state.naive_gpu.take();
        if let Some(live) = &mut state.work_context {
            // workshift zeros `total_iterations_today` then counts only this shift.
            // Do not subtract a leftover prior-shift total (that zeroed IPS on the HUD).
            workshift(
                token_budget,
                iteration_token_cost,
                bout_token_cost,
                point_token_cost,
                &mut live.context,
                gpu.as_mut(),
            );
            iters_delta = live.context.total_iterations_today as u64;
        }
        state.naive_gpu = gpu;
        state.total_workshifts += 1;
        if state.total_workshifts % 1 == 0 {
            if let Some(live) = &mut state.work_context {
                let c = work_update(&mut live.context);
                // Send even when this shift only advanced iterations (no finals):
                // otherwise HUD IPS drops to 0 on iterate-heavy interior work.
                if !c.is_empty() || iters_delta > 0 {
                    // r[impl cz.craft.emergent-cadence+1]
                    // r[impl cz.depth.gear-hud+2]
                    actor.try_send(
                        &mut updates_out,
                        telemetry_update(None, c, Some(&mut live.context), iters_delta),
                    );
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

pub(crate) fn telemetry_update<T>(
    frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    completed_points: Vec<(CompletedPoint<T>, usize)>,
    mut ctx: Option<&mut WorkContext<T>>,
    iterations_delta: u64,
) -> WorkUpdate<T>
where
    T: Mandelbrotable + 'static,
{
    use crate::assemblies::structs::{HostStack, KernelMode, ReferenceStatus};
    let (host_stack, kernel_mode, reference_status, active_gear) = match ctx.as_mut() {
        Some(c) => {
            let batch = completed_points.len() as u32;
            if batch > 0 {
                c.record_hud_completion_batch(batch);
            }
            let kernel_mode = classify_kernel_mode(c);
            (
                host_stack_for_context::<T>(),
                kernel_mode,
                classify_reference_status(c),
                // CPU naive is host f64 iterate. Naive GPU reports real device precision
                // (set on context.active_gear in workshift_naive_gpu).
                if kernel_mode == KernelMode::Naive {
                    ComputeGear::F64
                } else {
                    c.active_gear
                },
            )
        }
        None => (
            HostStack::F64,
            KernelMode::Naive,
            ReferenceStatus::Wip,
            ComputeGear::F64,
        ),
    };
    WorkUpdate {
        frame_info,
        completed_points,
        active_gear,
        host_stack,
        kernel_mode,
        reference_status,
        iterations_delta,
    }
}

/// Host stack admitted for this `WorkContext` monomorphization.
pub fn host_stack_for_context<T: Mandelbrotable + 'static>() -> crate::assemblies::structs::HostStack {
    use crate::assemblies::structs::HostStack;
    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<crate::floatexp::FloatExp>() {
        HostStack::FloatExp
    } else {
        HostStack::F64
    }
}

pub fn usable_reference<T: Mandelbrotable>(ctx: &WorkContext<T>) -> bool {
    // Relative hard-bump may use an escaped view-center orbit for precision.
    if ctx.coords_are_relative {
        return ctx.latest_reference.is_some();
    }
    ctx.latest_reference
        .as_ref()
        .is_some_and(|r| !r.orbit.escaped)
}

pub fn classify_kernel_mode<T: Mandelbrotable>(ctx: &WorkContext<T>) -> crate::assemblies::structs::KernelMode {
    use crate::assemblies::structs::KernelMode;
    if let Some(forced) = ctx.manual_gear {
        return forced;
    }
    if let Some(locked) = ctx.pps_locked_kernel {
        return locked;
    }
    if let Some(&probing) = ctx.pps_probe_queue.first() {
        return probing;
    }
    if ctx.perturbation_kernel_required() {
        KernelMode::Pert
    } else if ctx.last_used_naive_gpu {
        KernelMode::NaiveGpu
    } else {
        KernelMode::Naive
    }
}

/// Running snapshot: wip when no usable ref yet, or undelivered glitch seats await newer generation.
pub fn classify_reference_status<T: Mandelbrotable>(
    ctx: &WorkContext<T>,
) -> crate::assemblies::structs::ReferenceStatus {
    use crate::assemblies::structs::ReferenceStatus;
    if !usable_reference(ctx) {
        return ReferenceStatus::Wip;
    }
    if ctx
        .points
        .iter()
        .any(|p| p.direct_only && !p.delivered)
    {
        return ReferenceStatus::Wip;
    }
    ReferenceStatus::Complete
}

// r[impl cz.craft.lifo-drain+1]
fn work_update<T: Mandelbrotable>(ctx: &mut WorkContext<T>) -> Vec<(CompletedPoint<T>, usize)> {
    let mut returned = vec!();
    for _ in 0..ctx.completed_points.len {
        returned.push(ctx.completed_points.try_pop().unwrap())
    }
    returned
}

#[inline]
fn transform_index(
    i: usize
    , in_data_res: (u32, u32)
    , out_data_res: (u32, u32)
    , out_data_len: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> Option<usize> {

    let l = transform_relative_location_i32(
        relative_location_from_index(
            in_data_res, i
        )
        , relative_pos
        , relative_zoom_pot
    );

    if l.0 <= (out_data_res.0-1) as i32
        && l.0 > 0
        && l.1 > 0
        && l.1 <= (out_data_res.1-1) as i32
    {
        Some(index_from_relative_location(
            l
            , out_data_res
            , out_data_len
        ))
    } else {
        None
    }
}

#[inline]
pub fn relative_location_from_index(data_res: (u32, u32), index: usize) -> (i32, i32) {

    (
        index as i32 % (data_res.0) as i32
        , index as i32 / (data_res.0) as i32
        )
}