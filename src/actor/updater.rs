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
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    updates_out_colorer: SteadyTx<ZoomerSettingsUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in], [&updates_out_colorer]),
        updates_in,
        updates_out_colorer,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    updates_out_colorer: SteadyTx<ZoomerSettingsUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {

    // Lock all channels for exclusive access within this actor.
    let updates_in = updates_in.lock().await;
    let updates_out_colorer = updates_out_colorer.lock().await;
    let state = state.lock(|| UpdaterState {
    }).await;

    // the updater runs at a precise rate, to control animations and stuff.
    // because of this, the code here should run extremely fast.
    // This shouldn't be hard to achieve because it won't need any n-long loops.
    // This actor's main job is to simply distribute settings updates
    // Its reason for existing, though, is to convert a changing setting
    // into individual value settings; for example, to animate colors.

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
        );


        // do updater stuff

        /*let mut update = ZoomerUpdate{
            settings: SettingsState{}
            , state: ZoomerState{settings_window_open: false}
            , settings_changes:vec!()
            , state_changes:vec!()
        };*/

        let updated = false;
    }

    // Final shutdown log, reporting all statistics.
    info!("Updater shutting down.");
    Ok(())
}