use steady_state::*;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::screen_worker::AnswerTilePublish;

pub struct GpuUploaderState {
    unsent: Option<GPUTile>
}

pub async fn run(
    actor: SteadyActorShadow
    , tiles_in: SteadyRx<AnswerTilePublish>
    , tiles_out: SteadyTx<GPUTile>
    , state: SteadyState<GpuUploaderState>
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&tiles_in], [&tiles_out])
        , tiles_in
        , tiles_out
        , state
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A
    , tiles_in: SteadyRx<AnswerTilePublish>
    , tiles_out: SteadyTx<GPUTile>
    , state: SteadyState<GpuUploaderState>
) -> Result<(), Box<dyn Error>> {
    let mut tiles_in = tiles_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
    let mut state = state.lock(|| GpuUploaderState { unsent: None }).await;

    let max_sleep = Duration::from_millis(2);

    while actor.is_running(
        || i!(tiles_out.mark_closed())
    ) {
        // Prefer draining publish backlog over sleeping when work is waiting.
        if actor.avail_units(&mut tiles_in) == 0 && state.unsent.is_none() {
            await_for_any!(
                actor.wait_periodic(max_sleep)
                , actor.wait_avail(&mut tiles_in, 1)
            );
        }

        if let Some(tile) = state.unsent.take() {
            match actor.try_send(&mut tiles_out, tile) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(tile)
                | SendOutcome::Timeout(tile)
                | SendOutcome::Closed(tile) => {
                    state.unsent = Some(tile);
                    continue;
                }
            }
        }

        while actor.avail_units(&mut tiles_in) > 0 {
            let Some(publish) = actor.try_take(&mut tiles_in) else {
                break;
            };
            let gpu_tile = GPUTile::from_answer_tile(
                &publish.tile
                , publish.screen_res
                , publish.location
            );
            match actor.try_send(&mut tiles_out, gpu_tile) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(tile)
                | SendOutcome::Timeout(tile)
                | SendOutcome::Closed(tile) => {
                    state.unsent = Some(tile);
                    break;
                }
            }
        }
    }

    info!("GPU uploader shutting down.");
    Ok(())
}
