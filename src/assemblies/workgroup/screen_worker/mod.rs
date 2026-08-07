use std::cmp::min;
use steady_state::*;
use crate::assemblies::headgroup::window::sampling::{index_from_relative_location, relative_location_i32_row_and_seat, transform_relative_location_i32};
use crate::assemblies::workgroup::c_generator::Mandelbrotable;
use crate::assemblies::workgroup::reference_worker::{
    select_reference_request, PublishedReference, ReferenceRequest,
};
use crate::utils::ObjectivePosAndZoom;
//use crate::actor::work_collector::*;
use crate::assemblies::workgroup::work_controller::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;

pub mod workshift;
pub mod perturb_kernel;

#[cfg(test)]
mod craftsmanship_tests;

pub struct WorkUpdate<T> {
    pub frame_info: Option<(ObjectivePosAndZoom, (u32, u32))>,
    pub completed_points: (Vec<(CompletedPoint<T>, usize)>)
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
}

/// The single live render target: a work context and the frame_info it was
/// built from. The pairing is structural — the two can never be set or cleared
/// independently.
#[derive(Clone)]
pub struct LiveTarget<T: Mandelbrotable> {
    pub context: WorkContext<T>,
    pub frame_info: (ObjectivePosAndZoom, (u32, u32)),
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate<crate::floatexp::FloatExp>>,
    attention_in: SteadyRx<Option<(i32, i32)>>,
    reference_requests_out: SteadyTx<ReferenceRequest>,
    references_in: SteadyRx<PublishedReference>,
    state: SteadyState<WorkerState<crate::floatexp::FloatExp>>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight(
            [&commands_in, &attention_in, &references_in],
            [&updates_out, &reference_requests_out],
        ),
        commands_in,
        updates_out,
        attention_in,
        reference_requests_out,
        references_in,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate<crate::floatexp::FloatExp>>,
    attention_in: SteadyRx<Option<(i32, i32)>>,
    reference_requests_out: SteadyTx<ReferenceRequest>,
    references_in: SteadyRx<PublishedReference>,
    state: SteadyState<WorkerState<crate::floatexp::FloatExp>>,
) -> Result<(), Box<dyn Error>> {

    //actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;
    let mut attention_in = attention_in.lock().await;
    let mut reference_requests_out = reference_requests_out.lock().await;
    let mut references_in = references_in.lock().await;

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , workshift_token_budget: 16000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
        , total_workshifts: 0
        , pending_reference: None
    }).await;

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
            // Escaped references are proven bad; keep the zero-orbit floor until a
            // usable interior snapshot arrives.
            if !newest.orbit.escaped {
                state.pending_reference = Some(newest.clone());
                if let Some(live) = &mut state.work_context {
                    live.context.latest_reference = Some(newest);
                }
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
                    let previous_for_shell = match previous {
                        Some(mut live) => {
                            let old_zoom = live.frame_info.0.clone();
                            let U = work_update(&mut live.context);
                            if U.len() > 0 {
                                actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:U});
                            }
                            Some((live.context, old_zoom))
                        }
                        None => None,
                    };

                    if let Some(mut new_ctx) = from_stencil(frame_info.clone(), previous_for_shell) {
                        if let Some(pending) = state.pending_reference.clone() {
                            // r[impl cz.depth.reference-coverage+1]
                            if crate::assemblies::workgroup::reference_worker::reference_c_covers_frame(
                                &pending.c,
                                &frame_info,
                            ) {
                                new_ctx.latest_reference = Some(pending);
                            } else {
                                // Uncovered sticky refs cause classic glitch blobs when
                                // zooming into hard areas; drop to zero-orbit until the
                                // new-view reference arrives.
                                state.pending_reference = None;
                                new_ctx.latest_reference = None;
                            }
                        }
                        state.work_context = Some(LiveTarget { context: new_ctx, frame_info: frame_info.clone() });
                        actor.try_send(&mut updates_out, WorkUpdate{frame_info:Some(frame_info), completed_points:vec!()});
                    }
                }
            }
        }

        let token_budget = state.workshift_token_budget.clone();
        let iteration_token_cost = state.iteration_token_cost.clone();
        let bout_token_cost = state.bout_token_cost.clone();
        let point_token_cost = state.point_token_cost.clone();
        

        if let Some(live) = &mut state.work_context {
            //let start = Instant::now();
            workshift (
                token_budget
                , iteration_token_cost
                , bout_token_cost
                , point_token_cost
                , &mut live.context
            );
            state.total_workshifts+=1;
            //info!("workday completed. took {}ms.", start.elapsed().as_millis());
            //info!("workshift {}", state.total_workshifts);
        }


        if state.total_workshifts % 1 == 0 {
            if let Some(live) = &mut state.work_context {
                let c = work_update(&mut live.context);
                if c.len() > 0 {
                    // r[impl cz.craft.emergent-cadence+1]
                    actor.try_send(&mut updates_out, WorkUpdate{frame_info:None, completed_points:c});
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

// r[impl cz.craft.lifo-drain+1]
fn work_update<T: Mandelbrotable>(ctx: &mut WorkContext<T>) -> Vec<(CompletedPoint<T>, usize)> {


    //ctx.completed_points
    let update_start = ctx.last_update;
    let mut returned = vec!();
    for _ in 0..ctx.completed_points.len {
        returned.push(ctx.completed_points.try_pop().unwrap())
    }
    /*returned.append(&mut ctx.completed_points);
    ctx.completed_points = vec!();
    ctx.last_update = ctx.index;*/
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
        , index as i32 / (data_res.1) as i32
        )
}