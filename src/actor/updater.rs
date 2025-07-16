use steady_state::*;
use crate::actor::window::*;
use crate::action::settings::*;



pub(crate) struct ZoomerUpdate {
    pub(crate) settings: SettingsState,
    pub(crate) state: ZoomerState,
    pub(crate) settings_changes: Vec<ZoomerSettingsUpdate>,
    pub(crate) state_changes: Vec<ZoomerStateUpdate>
}

pub(crate) enum ZoomerSettingsUpdate {

}

pub(crate) enum ZoomerStateUpdate {
    OpenSettingsWindow,
    CloseSettingsWindow,
}



pub(crate) struct UpdaterState {
}


pub async fn run(
    actor: SteadyActorShadow,
    updates_in: SteadyRx<ZoomerUpdate>,
    updates_out_colorer: SteadyTx<ZoomerUpdate>,
    updates_out_worker: SteadyTx<ZoomerUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in], [&updates_out_colorer, &updates_out_worker]),
        updates_in,
        updates_out_colorer,
        updates_out_worker,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    updates_in: SteadyRx<ZoomerUpdate>,
    updates_out_colorer: SteadyTx<ZoomerUpdate>,
    updates_out_worker: SteadyTx<ZoomerUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {
    let mut updates_in = updates_in.lock().await;
    let mut updates_out_colorer = updates_out_colorer.lock().await;
    let mut updates_out_worker = updates_out_worker.lock().await;
    let mut state = state.lock(|| UpdaterState {
    }).await;

    // Lock all channels for exclusive access within this actor.

    let max_latency = Duration::from_millis(40);

    // Main processing loop.
    // The actor runs until all input channels are closed and empty, and the output channel is closed.
    while actor.is_running(
        || i!(true)
    ) {
        // Wait for all required conditions:
        // - A periodic timer
        await_for_any!(  //#!#//
            actor.wait_periodic(max_latency),
            actor.wait_avail(&mut updates_in, 1),
        );


        // do updater stuff

        let mut update = ZoomerUpdate{
            settings: SettingsState{}
            , state: ZoomerState{settings_window_open: false}
            , settings_changes:vec!()
            , state_changes:vec!()
        };

        let mut updated = false;
    }

    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}