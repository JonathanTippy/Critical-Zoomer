use std::cmp::min;
use steady_state::*;
use crate::assemblies::headgroup::window::sampling::{index_from_relative_location, relative_location_i32_row_and_seat, transform_relative_location_i32};
use crate::assemblies::workgroup::c_generator::{
    admit_generator_with_margin, CGenerator, Mandelbrotable,
};
use crate::assemblies::structs::{AttentionFocus, KernelMode};
use crate::assemblies::workgroup::reference_worker::{
    select_reference_request, PublishedReference, ReferenceRequest,
};
use crate::delta_gear::ComputeGear;
use crate::copy_intexp::CopyIntExp1;
use crate::utils::{ObjectivePosAndZoom, pos_from_index, IntExp};
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
    /// View iterations-per-point (mean seat iterations). Final when the view is done.
    pub ipp: u32,
    pub ipp_final: bool,
    /// Oldest pending controller Replace emission time (HUD `ctrl:`).
    pub controller_emitted_at: Option<std::time::Instant>,
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
    // True after the first lazy init attempt (success or miss) so we do not
    // respawn wgpu every workshift when the adapter is absent.
    , naive_gpu_init_tried: bool
    // Latest debug manual-gear override from settings (`None` = auto policy).
    , manual_gear: Option<crate::assemblies::structs::KernelMode>
    // C-generator render margin bits from settings (default 1).
    , c_generator_margin_bits: u32
    // OG naive f32 host when the bit-count gate admits f32 (CPU DirectKernel).
    , f32_live: Option<LiveTarget<f32>>
    // OG naive one-word CopyIntExp after absolute f64 fails (CPU DirectKernel).
    , cie_live: Option<LiveTarget<CopyIntExp1>>
    // Controller Replace emission Instant awaiting the next successful WorkUpdate put.
    , pending_controller_emitted_at: Option<std::time::Instant>
    // O(1) park predicate: avoid scanning ~410k seats on every wake after fill.
    // Updated after each workshift / Replace; true while any seat is undelivered.
    , seats_need_work: bool
}

/// The single live render target: a work context and the frame_info it was
/// built from. The pairing is structural — the two can never be set or cleared
/// independently.
#[derive(Clone)]
pub struct LiveTarget<T: Mandelbrotable> {
    pub context: WorkContext<T>,
    pub frame_info: (ObjectivePosAndZoom, (u32, u32)),
}

fn relative_shell_ok(manual_gear: Option<KernelMode>) -> bool {
    matches!(manual_gear, Some(KernelMode::Pert))
}

fn og_f32_naive_admits(
    gear: Option<KernelMode>,
    loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    margin: u32,
) -> bool {
    gear == Some(KernelMode::Naive)
        && CGenerator::<f32>::new_with_margin(loc, zoom_pot, res, margin).is_some()
}

fn og_copy_intexp1_naive_admits(
    gear: Option<KernelMode>,
    loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    margin: u32,
) -> bool {
    // r[impl cz.depth.c-generator-fails-closed+1]
    if gear != Some(KernelMode::Naive) {
        return false;
    }
    CGenerator::<CopyIntExp1>::new_with_margin(loc, zoom_pot, res, margin).is_some()
        && CGenerator::<f64>::new_with_margin(loc, zoom_pot, res, margin).is_none()
}

fn ensure_naive_gpu_if_needed(state: &mut WorkerState<f64>) {
    use crate::assemblies::structs::KernelMode;
    if state.naive_gpu.is_some() || state.naive_gpu_init_tried {
        return;
    }
    let need = match state.manual_gear {
        Some(KernelMode::NaiveGpu) | None => true,
        Some(_) => false,
    };
    if !need {
        return;
    }
    state.naive_gpu_init_tried = true;
    // Init wgpu off the async executor — pollster::block_on nested in async can fail.
    let created = std::thread::Builder::new()
        .name("cz-naive-gpu-init".into())
        .spawn(|| naive_gpu::NaiveGpuContext::try_new())
        .ok()
        .and_then(|h| h.join().ok())
        .flatten();
    if created.is_some() {
        eprintln!("screen_worker: NaiveGpuContext ready");
    } else {
        eprintln!("screen_worker: NaiveGpu unavailable; CPU DirectKernel for naive");
    }
    state.naive_gpu = created;
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
    attention_in: SteadyRx<AttentionFocus>,
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
    attention_in: SteadyRx<AttentionFocus>,
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
        , naive_gpu_init_tried: false
        , manual_gear: Some(crate::assemblies::structs::KernelMode::Naive)
        , c_generator_margin_bits: crate::assemblies::workgroup::c_generator::DEFAULT_C_GENERATOR_MARGIN_BITS
        , pending_controller_emitted_at: None
        , seats_need_work: false
        , f32_live: None
        , cie_live: None
    }).await;

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        // r[impl cz.craft.load-proportional-ignorance+1]
        // Park when no undelivered seats remain. Use the cached flag — scanning
        // every seat on each wake costs tens of %CPU after a full home fill.
        let working = state.seats_need_work;

        if !working {
            // Warm continuum: still wake on the same input classes (commands,
            // attention, settings, references) plus a slow heartbeat. Wakes must
            // stay cheap (O(1) park check) — do not silence the pipe.
            let max_sleep = Duration::from_millis(50);
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut commands_in, 1),
                actor.wait_avail(&mut attention_in, 1),
                actor.wait_avail(&mut settings_in, 1),
                actor.wait_avail(&mut references_in, 1),
            );
            crate::debug_agent::note_worker_park_wake();
        } else {
            crate::debug_agent::note_worker_working();
        }

        let _loop_cpu = crate::debug_agent::busy_worker_loop();

        if actor.avail_units(&mut settings_in) > 0 {
            while actor.avail_units(&mut settings_in) > 1 {
                let _ = actor.try_take(&mut settings_in).expect("internal error");
            }
            if let Some(settings) = actor.try_take(&mut settings_in) {
                state.manual_gear = settings.manual_gear_override();
                let margin = settings.c_generator_margin_bits.min(48);
                let margin_changed = state.c_generator_margin_bits != margin;
                state.c_generator_margin_bits = margin;
                let gear = state.manual_gear;
                let mut reject_live = false;
                if let Some(live) = &mut state.f32_live {
                    live.context.manual_gear = gear;
                    if margin_changed {
                        live.context.c_generator_margin_bits = margin;
                        let compute_loc = (
                            live.frame_info.0.pos.0.clone(),
                            IntExp::ZERO - live.frame_info.0.pos.1.clone(),
                        );
                        if og_f32_naive_admits(
                            gear,
                            &compute_loc,
                            live.frame_info.0.zoom_pot as i64,
                            live.frame_info.1,
                            margin,
                        ) {
                            if let Some(g) = CGenerator::<f32>::new_with_margin(
                                &compute_loc,
                                live.frame_info.0.zoom_pot as i64,
                                live.frame_info.1,
                                margin,
                            ) {
                                let view_center = view_center_compute(
                                    &compute_loc,
                                    live.frame_info.0.zoom_pot,
                                    live.frame_info.1,
                                );
                                let generation = live.context.generator_generation;
                                apply_generator_admission(
                                    &mut live.context,
                                    crate::assemblies::workgroup::c_generator::GeneratorAdmission::Absolute(g),
                                    view_center,
                                    generation,
                                );
                            }
                        } else {
                            reject_live = true;
                        }
                    } else if gear != Some(KernelMode::Naive) {
                        reject_live = true;
                    }
                }
                if reject_live {
                    state.f32_live = None;
                    if state.work_context.is_none() && state.cie_live.is_none() {
                        state.seats_need_work = false;
                    }
                    reject_live = false;
                }
                if let Some(live) = &mut state.cie_live {
                    live.context.manual_gear = gear;
                    if margin_changed {
                        live.context.c_generator_margin_bits = margin;
                        let compute_loc = (
                            live.frame_info.0.pos.0.clone(),
                            IntExp::ZERO - live.frame_info.0.pos.1.clone(),
                        );
                        if og_copy_intexp1_naive_admits(
                            gear,
                            &compute_loc,
                            live.frame_info.0.zoom_pot as i64,
                            live.frame_info.1,
                            margin,
                        ) {
                            if let Some(g) = CGenerator::<CopyIntExp1>::new_with_margin(
                                &compute_loc,
                                live.frame_info.0.zoom_pot as i64,
                                live.frame_info.1,
                                margin,
                            ) {
                                let view_center = view_center_compute(
                                    &compute_loc,
                                    live.frame_info.0.zoom_pot,
                                    live.frame_info.1,
                                );
                                let generation = live.context.generator_generation;
                                apply_generator_admission(
                                    &mut live.context,
                                    crate::assemblies::workgroup::c_generator::GeneratorAdmission::Absolute(g),
                                    view_center,
                                    generation,
                                );
                            }
                        } else {
                            reject_live = true;
                        }
                    } else if gear != Some(KernelMode::Naive) {
                        reject_live = true;
                    }
                }
                if reject_live {
                    state.cie_live = None;
                    if state.work_context.is_none() {
                        state.seats_need_work = false;
                    }
                    reject_live = false;
                }
                if let Some(live) = &mut state.work_context {
                    live.context.manual_gear = gear;
                    if margin_changed {
                        live.context.c_generator_margin_bits = margin;
                        let compute_loc = (
                            live.frame_info.0.pos.0.clone(),
                            IntExp::ZERO - live.frame_info.0.pos.1.clone(),
                        );
                        let zoom_pot = live.frame_info.0.zoom_pot as i64;
                        let res = live.frame_info.1;
                        let view_center = view_center_compute(&compute_loc, live.frame_info.0.zoom_pot, res);
                        let admitted = if relative_shell_ok(gear) {
                            let relative_anchor = live.context.latest_reference.as_ref().map(|r| &r.c);
                            admit_generator_with_margin(
                                &compute_loc,
                                zoom_pot,
                                res,
                                relative_anchor,
                                &view_center,
                                margin,
                            )
                        } else {
                            CGenerator::new_with_margin(&compute_loc, zoom_pot, res, margin)
                                .map(crate::assemblies::workgroup::c_generator::GeneratorAdmission::Absolute)
                        };
                        if let Some(admission) = admitted {
                            let generation = live.context.generator_generation;
                            apply_generator_admission(
                                &mut live.context,
                                admission,
                                view_center,
                                generation,
                            );
                        } else {
                            // New margin does not admit this stencil in the live type.
                            reject_live = true;
                        }
                    }
                }
                if reject_live {
                    state.work_context = None;
                    state.seats_need_work = false;
                }
            }
        }

        if actor.avail_units(&mut attention_in) > 0 {
            while actor.avail_units(&mut attention_in) > 1 {
                let stuff = actor.try_take(&mut attention_in).expect("internal error");
                drop(stuff);
            };
            let attention = actor.try_take(&mut attention_in).expect("internal error");
            if let Some(live) = &mut state.f32_live {
                set_attention(&mut live.context, attention.pointer);
                set_gaze(&mut live.context, attention.gaze);
            } else if let Some(live) = &mut state.cie_live {
                set_attention(&mut live.context, attention.pointer);
                set_gaze(&mut live.context, attention.gaze);
            } else if let Some(live) = &mut state.work_context {
                set_attention(&mut live.context, attention.pointer);
                set_gaze(&mut live.context, attention.gaze);
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
                    .f32_live
                    .as_ref()
                    .is_some_and(|live| live.context.coords_are_relative)
                || state
                    .cie_live
                    .as_ref()
                    .is_some_and(|live| live.context.coords_are_relative)
                || state
                    .work_context
                    .as_ref()
                    .is_some_and(|live| live.context.coords_are_relative);
            if install {
                state.pending_reference = Some(newest.clone());
                if let Some(live) = &mut state.f32_live {
                    let keep_longer_bootstrap = newest.orbit.escaped
                        && live.context.coords_are_relative
                        && live.context.latest_reference.as_ref().is_some_and(|cur| {
                            cur.orbit.iterates.len() > newest.orbit.iterates.len()
                        });
                    if !keep_longer_bootstrap {
                        live.context.remember_reference(newest.clone());
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
                } else if let Some(live) = &mut state.cie_live {
                    let keep_longer_bootstrap = newest.orbit.escaped
                        && live.context.coords_are_relative
                        && live.context.latest_reference.as_ref().is_some_and(|cur| {
                            cur.orbit.iterates.len() > newest.orbit.iterates.len()
                        });
                    if !keep_longer_bootstrap {
                        live.context.remember_reference(newest.clone());
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
                } else if let Some(live) = &mut state.work_context {
                    let zoom_pot = live.frame_info.0.zoom_pot;
                    let keep_longer_bootstrap = newest.orbit.escaped
                        && live.context.coords_are_relative
                        && live.context.latest_reference.as_ref().is_some_and(|cur| {
                            cur.orbit.iterates.len() > newest.orbit.iterates.len()
                        });
                    if keep_longer_bootstrap {
                    } else {
                    live.context.remember_reference(newest.clone());
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

                WorkerCommand::Replace { frame_info, emitted_at } => {
                    state.pending_controller_emitted_at = Some(emitted_at);
                    // r[impl cz.craft.pivot-two-message-order+1]
                    // r[impl cz.craft.stencil-only-replace+2]
                    let request = if let Some(live) = state.f32_live.as_ref() {
                        select_reference_request(
                            Some((&live.context, &live.frame_info)),
                            &frame_info,
                        )
                    } else if let Some(live) = state.cie_live.as_ref() {
                        select_reference_request(
                            Some((&live.context, &live.frame_info)),
                            &frame_info,
                        )
                    } else {
                        select_reference_request(
                            state
                                .work_context
                                .as_ref()
                                .map(|live| (&live.context, &live.frame_info)),
                            &frame_info,
                        )
                    };
                    actor.try_send(&mut reference_requests_out, request);

                    let relative_ok = relative_shell_ok(state.manual_gear);
                    let compute_loc = (
                        frame_info.0.pos.0.clone(),
                        IntExp::ZERO - frame_info.0.pos.1.clone(),
                    );
                    let view_center = view_center_compute(
                        &compute_loc,
                        frame_info.0.zoom_pot,
                        frame_info.1,
                    );
                    let want_f32 = og_f32_naive_admits(
                        state.manual_gear,
                        &compute_loc,
                        frame_info.0.zoom_pot as i64,
                        frame_info.1,
                        state.c_generator_margin_bits,
                    );
                    let want_cie = og_copy_intexp1_naive_admits(
                        state.manual_gear,
                        &compute_loc,
                        frame_info.0.zoom_pot as i64,
                        frame_info.1,
                        state.c_generator_margin_bits,
                    );
                    let stencil_ok = want_f32 || want_cie || if relative_ok {
                        let relative_anchor = state
                            .work_context
                            .as_ref()
                            .and_then(|l| l.context.latest_reference.as_ref().map(|r| r.c.clone()));
                        admit_generator_with_margin::<f64>(
                            &compute_loc,
                            frame_info.0.zoom_pot as i64,
                            frame_info.1,
                            relative_anchor.as_ref(),
                            &view_center,
                            state.c_generator_margin_bits,
                        )
                        .is_some()
                    } else {
                        CGenerator::<f64>::new_with_margin(
                            &compute_loc,
                            frame_info.0.zoom_pot as i64,
                            frame_info.1,
                            state.c_generator_margin_bits,
                        )
                        .is_some()
                    };
                    if !stencil_ok {
                        // Fail closed: keep the last legal generator; do not
                        // install a type the margin rejects.
                        continue;
                    }

                    if let Some(gpu) = state.naive_gpu.as_mut() {
                        gpu.bump_generation();
                    }

                    if let Some(mut live) = state.f32_live.take() {
                        let iters = live.context.total_iterations_today;
                        let U = work_update(&mut live.context);
                        if !U.is_empty() {
                            let update = work_update_to_f64(telemetry_update(
                                None,
                                U,
                                Some(&mut live.context),
                                iters as u64,
                                state.pending_controller_emitted_at.take(),
                            ));
                            if let Err(u) = send_update_waiting(
                                &mut actor,
                                &mut updates_out,
                                update,
                            )
                            .await
                            {
                                if let Some(at) = u.controller_emitted_at {
                                    state.pending_controller_emitted_at = Some(at);
                                }
                            }
                        }
                    }

                    if let Some(mut live) = state.cie_live.take() {
                        let iters = live.context.total_iterations_today;
                        let U = work_update(&mut live.context);
                        if !U.is_empty() {
                            let update = work_update_to_f64(telemetry_update(
                                None,
                                U,
                                Some(&mut live.context),
                                iters as u64,
                                state.pending_controller_emitted_at.take(),
                            ));
                            if let Err(u) = send_update_waiting(
                                &mut actor,
                                &mut updates_out,
                                update,
                            )
                            .await
                            {
                                if let Some(at) = u.controller_emitted_at {
                                    state.pending_controller_emitted_at = Some(at);
                                }
                            }
                        }
                    }

                    let previous = state.work_context.take();
                    let previous_for_shell = match previous {
                        Some(mut live) => {
                            let old_zoom = live.frame_info.0.clone();
                            let iters = live.context.total_iterations_today;
                            let U = work_update(&mut live.context);
                            if !U.is_empty() {
                                let update = telemetry_update(
                                    None,
                                    U,
                                    Some(&mut live.context),
                                    iters as u64,
                                    state.pending_controller_emitted_at.take(),
                                );
                                // r[impl cz.craft.wait-on-channel-full+1]
                                if let Err(u) = send_update_waiting(
                                    &mut actor,
                                    &mut updates_out,
                                    update,
                                )
                                .await
                                {
                                    if let Some(at) = u.controller_emitted_at {
                                        state.pending_controller_emitted_at = Some(at);
                                    }
                                    restage_unsent_batch(
                                        &mut live.context,
                                        u.completed_points,
                                    );
                                }
                            }
                            Some((live.context, old_zoom))
                        }
                        None => None,
                    };

                    if want_f32 {
                        state.work_context = None;
                        state.cie_live = None;
                        if let Some(mut new_ctx) = from_stencil_with_margin::<f32>(
                            frame_info.clone(),
                            None,
                            state.c_generator_margin_bits,
                            false,
                        ) {
                            new_ctx.manual_gear = state.manual_gear;
                            new_ctx.c_generator_margin_bits = state.c_generator_margin_bits;
                            if let Some(pending) = state.pending_reference.clone() {
                                new_ctx.remember_reference(pending.clone());
                                rebuild_generator_for_reference(
                                    &mut new_ctx,
                                    &compute_loc,
                                    frame_info.0.zoom_pot as i64,
                                    frame_info.1,
                                    pending.as_ref(),
                                );
                            }
                            state.f32_live = Some(LiveTarget {
                                context: new_ctx,
                                frame_info: frame_info.clone(),
                            });
                            state.seats_need_work = true;
                            let ctrl = state.pending_controller_emitted_at.take();
                            let update = work_update_to_f64(telemetry_update(
                                Some(frame_info),
                                vec!(),
                                state.f32_live.as_mut().map(|l| &mut l.context),
                                0,
                                ctrl,
                            ));
                            if let Err(u) = send_update_waiting(
                                &mut actor,
                                &mut updates_out,
                                update,
                            )
                            .await
                            {
                                if let Some(at) = u.controller_emitted_at {
                                    state.pending_controller_emitted_at = Some(at);
                                }
                            }
                        } else {
                            state.seats_need_work = false;
                        }
                    } else if want_cie {
                        state.work_context = None;
                        state.f32_live = None;
                        if let Some(mut new_ctx) = from_stencil_with_margin::<CopyIntExp1>(
                            frame_info.clone(),
                            None,
                            state.c_generator_margin_bits,
                            false,
                        ) {
                            new_ctx.manual_gear = state.manual_gear;
                            new_ctx.c_generator_margin_bits = state.c_generator_margin_bits;
                            if let Some(pending) = state.pending_reference.clone() {
                                new_ctx.remember_reference(pending.clone());
                                rebuild_generator_for_reference(
                                    &mut new_ctx,
                                    &compute_loc,
                                    frame_info.0.zoom_pot as i64,
                                    frame_info.1,
                                    pending.as_ref(),
                                );
                            }
                            state.cie_live = Some(LiveTarget {
                                context: new_ctx,
                                frame_info: frame_info.clone(),
                            });
                            state.seats_need_work = true;
                            let ctrl = state.pending_controller_emitted_at.take();
                            let update = work_update_to_f64(telemetry_update(
                                Some(frame_info),
                                vec!(),
                                state.cie_live.as_mut().map(|l| &mut l.context),
                                0,
                                ctrl,
                            ));
                            if let Err(u) = send_update_waiting(
                                &mut actor,
                                &mut updates_out,
                                update,
                            )
                            .await
                            {
                                if let Some(at) = u.controller_emitted_at {
                                    state.pending_controller_emitted_at = Some(at);
                                }
                            }
                        } else {
                            state.seats_need_work = false;
                        }
                    } else if let Some(mut new_ctx) = from_stencil_with_margin(
                        frame_info.clone(),
                        previous_for_shell,
                        state.c_generator_margin_bits,
                        relative_ok,
                    ) {
                        new_ctx.manual_gear = state.manual_gear;
                        new_ctx.c_generator_margin_bits = state.c_generator_margin_bits;
                        if let Some(pending) = state.pending_reference.clone() {
                            // Greedy keep: install pending even when off-screen.
                            new_ctx.remember_reference(pending.clone());
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
                        }
                        state.work_context = Some(LiveTarget { context: new_ctx, frame_info: frame_info.clone() });
                        state.seats_need_work = true;
                        let ctrl = state.pending_controller_emitted_at.take();
                        let update = telemetry_update(
                            Some(frame_info),
                            vec!(),
                            state.work_context.as_mut().map(|l| &mut l.context),
                            0,
                            ctrl,
                        );
                        // r[impl cz.craft.wait-on-channel-full+1]
                        if let Err(u) = send_update_waiting(
                            &mut actor,
                            &mut updates_out,
                            update,
                        )
                        .await
                        {
                            if let Some(at) = u.controller_emitted_at {
                                state.pending_controller_emitted_at = Some(at);
                            }
                        }
                    } else {
                        state.seats_need_work = false;
                    }
                }
            }
        }

        let token_budget = state.workshift_token_budget.clone();
        let iteration_token_cost = state.iteration_token_cost.clone();
        let bout_token_cost = state.bout_token_cost.clone();
        let point_token_cost = state.point_token_cost.clone();

        // Re-check after Replace/commands — a fresh shell must work this turn.
        // When still complete, do not run a workshift (that burned ~10 ms/spin
        // and kept the actor hot despite the park wait above).
        // O(1) cached flag; refresh from seats after each shift below.
        let still_working = state.seats_need_work;

        let mut iters_delta = 0u64;
        if still_working {
            // r[impl cz.craft.wait-on-channel-full+1]
            // Calm down: do not invent more completions while the collector is behind.
            if actor.is_full(&mut updates_out) {
                if !actor.wait_vacant(&mut updates_out, 1).await {
                    continue;
                }
            }

            // Split borrows: take GPU handle, then mutate live context.
            // Compute wgpu stays off until Naive GPU or the PPS race needs it —
            // a resident compute device starves shade (colorer/escaper) cadence.
            ensure_naive_gpu_if_needed(&mut state);
            let mut gpu = state.naive_gpu.take();
            if let Some(live) = &mut state.f32_live {
                let _shift_cpu = crate::debug_agent::busy_worker_shift();
                workshift_with_kernel(
                    token_budget,
                    iteration_token_cost,
                    point_token_cost,
                    bout_token_cost,
                    &mut live.context,
                    &DirectKernel,
                );
                iters_delta = live.context.total_iterations_today as u64;
                state.seats_need_work = live.context.points.iter().any(|p| !p.delivered);
            } else if let Some(live) = &mut state.cie_live {
                let _shift_cpu = crate::debug_agent::busy_worker_shift();
                workshift_with_kernel(
                    token_budget,
                    iteration_token_cost,
                    point_token_cost,
                    bout_token_cost,
                    &mut live.context,
                    &DirectKernel,
                );
                iters_delta = live.context.total_iterations_today as u64;
                state.seats_need_work = live.context.points.iter().any(|p| !p.delivered);
            } else if let Some(live) = &mut state.work_context {
                // workshift zeros `total_iterations_today` then counts only this shift.
                // Do not subtract a leftover prior-shift total (that zeroed IPS on the HUD).
                let _shift_cpu = crate::debug_agent::busy_worker_shift();
                workshift(
                    token_budget,
                    iteration_token_cost,
                    bout_token_cost,
                    point_token_cost,
                    &mut live.context,
                    gpu.as_mut(),
                );
                iters_delta = live.context.total_iterations_today as u64;
                state.seats_need_work = live.context.points.iter().any(|p| !p.delivered);
            } else {
                state.seats_need_work = false;
            }
            state.naive_gpu = gpu;
            state.total_workshifts += 1;
            if state.total_workshifts % 1 == 0 {
                let ctrl = state.pending_controller_emitted_at.take();
                let mut restore_ctrl = None;
                if let Some(live) = &mut state.f32_live {
                    let c = work_update(&mut live.context);
                    if !c.is_empty() || iters_delta > 0 {
                        let update = work_update_to_f64(telemetry_update(
                            None,
                            c,
                            Some(&mut live.context),
                            iters_delta,
                            ctrl,
                        ));
                        match send_update_waiting(&mut actor, &mut updates_out, update).await {
                            Ok(()) => {}
                            Err(u) => {
                                restore_ctrl = u.controller_emitted_at;
                            }
                        }
                    } else {
                        restore_ctrl = ctrl;
                    }
                } else if let Some(live) = &mut state.cie_live {
                    let c = work_update(&mut live.context);
                    if !c.is_empty() || iters_delta > 0 {
                        let update = work_update_to_f64(telemetry_update(
                            None,
                            c,
                            Some(&mut live.context),
                            iters_delta,
                            ctrl,
                        ));
                        match send_update_waiting(&mut actor, &mut updates_out, update).await {
                            Ok(()) => {}
                            Err(u) => {
                                restore_ctrl = u.controller_emitted_at;
                            }
                        }
                    } else {
                        restore_ctrl = ctrl;
                    }
                } else if let Some(live) = &mut state.work_context {
                    let c = work_update(&mut live.context);
                    // Send even when this shift only advanced iterations (no finals):
                    // otherwise HUD IPS drops to 0 on iterate-heavy interior work.
                    if !c.is_empty() || iters_delta > 0 {
                        // r[impl cz.craft.emergent-cadence+1]
                        // r[impl cz.depth.gear-hud+2]
                        // r[impl cz.craft.wait-on-channel-full+1]
                        let update = telemetry_update(
                            None,
                            c,
                            Some(&mut live.context),
                            iters_delta,
                            ctrl,
                        );
                        match send_update_waiting(&mut actor, &mut updates_out, update).await {
                            Ok(()) => {}
                            Err(u) => {
                                restore_ctrl = u.controller_emitted_at;
                                restage_unsent_batch(&mut live.context, u.completed_points);
                            }
                        }
                    } else {
                        restore_ctrl = ctrl;
                    }
                } else {
                    restore_ctrl = ctrl;
                }
                if let Some(at) = restore_ctrl {
                    state.pending_controller_emitted_at = Some(at);
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn completed_to_f64<T: Mandelbrotable>(p: CompletedPoint<T>) -> CompletedPoint<f64> {
    match p {
        CompletedPoint::Repeats {
            period,
            smallness,
            small_time,
        } => CompletedPoint::Repeats {
            period,
            smallness: smallness.to_f64(),
            small_time,
        },
        CompletedPoint::Escapes {
            escape_time,
            escape_location,
            escape_derivative,
            start_location,
            smallness,
            small_time,
        } => CompletedPoint::Escapes {
            escape_time,
            escape_location: (escape_location.0.to_f64(), escape_location.1.to_f64()),
            escape_derivative: (escape_derivative.0.to_f64(), escape_derivative.1.to_f64()),
            start_location: (start_location.0.to_f64(), start_location.1.to_f64()),
            smallness: smallness.to_f64(),
            small_time,
        },
        CompletedPoint::Dummy {} => CompletedPoint::Dummy {},
    }
}

fn work_update_to_f64<T: Mandelbrotable + 'static>(u: WorkUpdate<T>) -> WorkUpdate<f64> {
    WorkUpdate {
        frame_info: u.frame_info,
        completed_points: u
            .completed_points
            .into_iter()
            .map(|(p, i)| (completed_to_f64(p), i))
            .collect(),
        active_gear: u.active_gear,
        host_stack: u.host_stack,
        kernel_mode: u.kernel_mode,
        reference_status: u.reference_status,
        iterations_delta: u.iterations_delta,
        ipp: u.ipp,
        ipp_final: u.ipp_final,
        controller_emitted_at: u.controller_emitted_at,
    }
}

pub(crate) fn telemetry_update<T>(
    frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    completed_points: Vec<(CompletedPoint<T>, usize)>,
    mut ctx: Option<&mut WorkContext<T>>,
    iterations_delta: u64,
    controller_emitted_at: Option<std::time::Instant>,
) -> WorkUpdate<T>
where
    T: Mandelbrotable + 'static,
{
    use crate::assemblies::structs::{HostStack, KernelMode, ReferenceStatus};
    let (host_stack, kernel_mode, reference_status, active_gear, ipp, ipp_final) =
        match ctx.as_mut() {
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
                    c.active_gear,
                    c.view_ipp().round() as u32,
                    c.view_ipp_final(),
                )
            }
            None => (
                HostStack::F64,
                KernelMode::Naive,
                ReferenceStatus::Wip,
                ComputeGear::F64,
                0,
                false,
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
        ipp,
        ipp_final,
        controller_emitted_at,
    }
}

/// Host stack admitted for this `WorkContext` monomorphization.
pub fn host_stack_for_context<T: Mandelbrotable + 'static>() -> crate::assemblies::structs::HostStack {
    use crate::assemblies::structs::HostStack;
    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<crate::floatexp::FloatExp>() {
        HostStack::FloatExp
    } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>() {
        HostStack::F32
    } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<CopyIntExp1>() {
        HostStack::CopyI64
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
/// Drain per-shift completions LIFO (newest first) for the collector channel.
pub(crate) fn work_update<T: Mandelbrotable>(
    ctx: &mut WorkContext<T>,
) -> Vec<(CompletedPoint<T>, usize)> {
    let mut returned = Vec::with_capacity(ctx.completed_points.len());
    while let Some(x) = ctx.completed_points.pop() {
        returned.push(x);
    }
    returned
}

/// Send a WorkUpdate to the collector, waiting when the channel is full.
/// Returns `Err(update)` only on shutdown interrupt or closed channel — never
/// clears `delivered` (that painted Dummy holes / black streaks).
// r[impl cz.craft.wait-on-channel-full+1]
async fn send_update_waiting<A, T>(
    actor: &mut A,
    updates_out: &mut T,
    mut update: WorkUpdate<f64>,
) -> Result<(), WorkUpdate<f64>>
where
    A: SteadyActor,
    T: TxCore<MsgOut = WorkUpdate<f64>, MsgSize = usize>,
    for<'a> T: TxCore<MsgIn<'a> = WorkUpdate<f64>, MsgOut = WorkUpdate<f64>, MsgSize = usize>,
{
    loop {
        match actor.try_send(updates_out, update) {
            SendOutcome::Success => return Ok(()),
            SendOutcome::Blocked(u) | SendOutcome::Timeout(u) => {
                update = u;
                if !actor.wait_vacant(updates_out, 1).await {
                    return Err(update);
                }
            }
            SendOutcome::Closed(u) => return Err(u),
        }
    }
}

/// Put an unsent batch back into staging without clearing `delivered`.
/// Steady full-channel waits retry the same update in place; restage is only
/// for shutdown interrupt so the next boot (or rare resume) can flush again.
// r[impl cz.craft.wait-on-channel-full+1]
pub(crate) fn restage_unsent_batch<T: Mandelbrotable>(
    ctx: &mut WorkContext<T>,
    batch: Vec<(CompletedPoint<T>, usize)>,
) {
    for item in batch.into_iter().rev() {
        ctx.completed_points.push(item);
    }
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

#[cfg(test)]
mod mutant_kill {
    use super::*;
    use crate::assemblies::structs::{KernelMode, ReferenceStatus};
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::constants::{HOME_POSITION, TEST_SCREEN_RES};
    use crate::floatexp::FloatExp;
    use crate::utils::{IntExp, ObjectivePosAndZoom};

    #[test]
    fn mutant_kill_relative_location_from_index_mod_div() {
        assert_eq!(relative_location_from_index(TEST_SCREEN_RES, 0), (0, 0));
        assert_eq!(relative_location_from_index(TEST_SCREEN_RES, TEST_SCREEN_RES.0 as usize - 1), ((TEST_SCREEN_RES.0 - 1) as i32, 0));
        assert_eq!(relative_location_from_index(TEST_SCREEN_RES, TEST_SCREEN_RES.0 as usize), (0, 1));
        assert_eq!(relative_location_from_index(TEST_SCREEN_RES, TEST_SCREEN_RES.0 as usize + 1), (1, 1));
        assert_ne!(relative_location_from_index(TEST_SCREEN_RES, TEST_SCREEN_RES.0 as usize + 1), (0, (TEST_SCREEN_RES.0 as i32) + 1));
        assert_ne!(relative_location_from_index(TEST_SCREEN_RES, TEST_SCREEN_RES.0 as usize + 1), ((TEST_SCREEN_RES.0 as i32) + 1, 0));
    }

    #[test]
    fn mutant_kill_classify_usable_ref_and_host_stack() {
        assert_eq!(host_stack_for_context::<f32>(), crate::assemblies::structs::HostStack::F32);
        assert_eq!(host_stack_for_context::<f64>(), crate::assemblies::structs::HostStack::F64);
        assert_eq!(
            host_stack_for_context::<crate::copy_intexp::CopyIntExp1>(),
            crate::assemblies::structs::HostStack::CopyI64
        );
        assert_eq!(
            host_stack_for_context::<FloatExp>(),
            crate::assemblies::structs::HostStack::FloatExp
        );
        assert_ne!(
            host_stack_for_context::<f64>(),
            crate::assemblies::structs::HostStack::FloatExp
        );

        let home = ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::ZERO - IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: -2,
        };
        let mut ctx = from_stencil::<f64>((home, TEST_SCREEN_RES), None).expect("home");
        ctx.manual_gear = None;
        ctx.pps_locked_kernel = None;
        ctx.pps_probe_queue.clear();
        ctx.coords_are_relative = false;
        ctx.reference_floor_active = false;
        ctx.last_used_naive_gpu = false;
        ctx.latest_reference = None;
        assert!(!usable_reference(&ctx));
        assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Wip);
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Naive);

        ctx.last_used_naive_gpu = true;
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::NaiveGpu);
        ctx.reference_floor_active = true;
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Pert);
        // Priority: probe > required, lock > probe, manual > lock.
        ctx.pps_probe_queue.push(KernelMode::Naive);
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Naive);
        ctx.pps_locked_kernel = Some(KernelMode::NaiveGpu);
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::NaiveGpu);
        ctx.manual_gear = Some(KernelMode::Pert);
        assert_eq!(classify_kernel_mode(&ctx), KernelMode::Pert);

        // Absolute: escaped ref is not usable; relative: any Some is usable.
        ctx.manual_gear = None;
        ctx.pps_locked_kernel = None;
        ctx.pps_probe_queue.clear();
        ctx.reference_floor_active = false;
        ctx.coords_are_relative = false;
        // Build a minimal PublishedReference-shaped gate via latest_reference None stays unusable.
        assert!(!usable_reference(&ctx));
        ctx.coords_are_relative = true;
        assert!(!usable_reference(&ctx)); // still None
        // direct_only undelivered keeps status Wip even with usable relative ref absent.
        ctx.coords_are_relative = false;
        ctx.points[0].direct_only = true;
        ctx.points[0].delivered = false;
        assert_eq!(classify_reference_status(&ctx), ReferenceStatus::Wip);
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn og_copy_intexp1_naive_flips_on_after_home_f64_wall() {
        use crate::constants::DEFAULT_WINDOW_RES;
        let loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        let naive = Some(KernelMode::Naive);
        assert!(!og_copy_intexp1_naive_admits(naive, &loc, 38, res, 1));
        assert!(og_copy_intexp1_naive_admits(naive, &loc, 42, res, 1));
        assert!(og_copy_intexp1_naive_admits(naive, &loc, 49, res, 1));
        assert!(!og_copy_intexp1_naive_admits(Some(KernelMode::Pert), &loc, 49, res, 1));
        let tip = (
            crate::assemblies::headgroup::window::coords::decimal_str_to_intexp(
                crate::constants::NORTH_TIP_RE,
            )
            .unwrap(),
            crate::assemblies::headgroup::window::coords::decimal_str_to_intexp(
                crate::constants::NORTH_TIP_IM,
            )
            .unwrap(),
        );
        assert!(CGenerator::<f64>::new_with_margin(&tip, 38, res, 1).is_some());
        assert!(!og_copy_intexp1_naive_admits(naive, &tip, 38, res, 1));
    }

    #[test]
    fn mutant_kill_transform_index_exclusive_zero_and_upper() {
        // Identity transform (rel=0, zoom=0): seat (0,0) maps to (0,0) and is
        // rejected by exclusive >0 on both axes; interior seats pass.
        let in_res = (4u32, 3u32);
        let out_res = (4u32, 3u32);
        let len = 12usize;
        assert!(transform_index(0, in_res, out_res, len, (0, 0), 0).is_none()); // (0,0)
        assert!(transform_index(1, in_res, out_res, len, (0, 0), 0).is_none()); // (1,0) y==0
        assert!(transform_index(4, in_res, out_res, len, (0, 0), 0).is_none()); // (0,1) x==0
        assert_eq!(
            transform_index(5, in_res, out_res, len, (0, 0), 0),
            Some(5)
        ); // (1,1)
        // Upper inclusive res-1: (3,2) is index 11.
        assert_eq!(
            transform_index(11, in_res, out_res, len, (0, 0), 0),
            Some(11)
        );
        // >→>= on zero would admit the origin row/col.
        assert_ne!(transform_index(0, in_res, out_res, len, (0, 0), 0), Some(0));
    }
}