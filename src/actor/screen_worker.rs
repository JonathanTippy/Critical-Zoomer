use steady_state::*;

use crate::action::workshift::*;
use crate::actor::work_controller::*;




pub(crate) struct WorkUpdate {
    pub(crate) completed_points: Vec<CompletedPoint>
}

#[derive(Clone)]
pub(crate) struct WorkerState {
    work_context: Option<WorkContext>
    , workshift_token_budget: u32
    , iteration_token_cost: u32
    , point_token_cost: u32
    , bout_token_cost: u32
    , workshift_token_cost: u32
}

pub async fn run(
    actor: SteadyActorShadow,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&commands_in], [&updates_out]),
        commands_in,
        updates_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    commands_in: SteadyRx<WorkerCommand>,
    updates_out: SteadyTx<WorkUpdate>,
    state: SteadyState<WorkerState>,
) -> Result<(), Box<dyn Error>> {

    actor.loglevel(LogLevel::Debug);

    let mut commands_in = commands_in.lock().await;
    let mut updates_out = updates_out.lock().await;

    let mut state = state.lock(|| WorkerState {
        work_context: None
        , workshift_token_budget: 1000000
        , iteration_token_cost: 2
        , bout_token_cost: 4
        , workshift_token_cost: 0
        , point_token_cost: 150
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(updates_out.mark_closed())
    ) {

        let working = if let Some(_) = state.work_context {true} else {false};

        if working {} else {
            await_for_any!(
                actor.wait_periodic(max_sleep),
                actor.wait_avail(&mut commands_in, 1),
            );
        }

        while actor.avail_units(&mut commands_in) > 0 {
            match actor.try_take(&mut commands_in).unwrap() {
                WorkerCommand::Update => {
                    if let Some(ctx) = &mut state.work_context {
                        actor.try_send(&mut updates_out, WorkUpdate{completed_points:work_update(ctx)});
                        if ctx.percent_completed == 100.0 {state.work_context = None;}
                        // ^ flush context if complete
                    } else {
                        actor.try_send(&mut updates_out, WorkUpdate{completed_points:vec!()});
                    }
                }
                WorkerCommand::Replace{context:ctx} => {
                    state.work_context = Some(ctx);
                }
            }
        }

        let token_budget = state.workshift_token_budget.clone();
        let iteration_token_cost = state.iteration_token_cost.clone();
        let bout_token_cost = state.bout_token_cost.clone();
        let point_token_cost = state.point_token_cost.clone();
        

        if let Some(ctx) = &mut state.work_context {
            let start = Instant::now();
            workshift (
                token_budget
                , iteration_token_cost
                , bout_token_cost
                , point_token_cost
                , ctx
            );
            //info!("workday completed. took {}ms.", start.elapsed().as_millis());
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn calculate_tokens(state: &mut WorkerState) {

}

fn work_update(ctx: &mut WorkContext) -> Vec<CompletedPoint> {
    let mut returned = vec!();
    for i in ctx.last_update..ctx.index {
        returned.push(ctx.completed_points[i].clone());
    }
    ctx.last_update = ctx.index;
    returned
}
