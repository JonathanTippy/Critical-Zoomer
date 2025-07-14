use steady_state::*;
use crate::actor::window::*;
use crate::operation::settings::*;

pub(crate) struct ZoomerUpdate {
    pub(crate) settings: ZoomerSettingsState,
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
    state_in: SteadyRx<Vec<ZoomerStateUpdate>>,
    settings_in: SteadyRx<ZoomerSettingsUpdate>,
    updates_out: SteadyTx<ZoomerUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&state_in, &settings_in], [&updates_out]),
        state_in,
        settings_in,
        updates_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    state_in: SteadyRx<Vec<ZoomerStateUpdate>>,
    settings_in: SteadyRx<ZoomerSettingsUpdate>,
    updates_out: SteadyTx<ZoomerUpdate>,
    state: SteadyState<UpdaterState>,
) -> Result<(), Box<dyn Error>> {
    let mut state_in = state_in.lock().await;
    let mut settings_in = settings_in.lock().await;
    let mut updates_out = updates_out.lock().await;
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
            actor.wait_avail(&mut state_in, 1),
            actor.wait_avail(&mut settings_in, 1)
        );


        // do updater stuff

        let mut update = ZoomerUpdate{
            settings: ZoomerSettingsState{}
            , state: ZoomerState{settings_window_open: false}
            , settings_changes:vec!()
            , state_changes:vec!()
        };

        let mut updated = false;


        match actor.try_take(&mut state_in) {
            Some(state_updates) => {
                for state_update in state_updates {
                    info!("state update");
                    update.state_changes.push(state_update);
                    updated = true;
                    info!("inserted state change into update")
                }
            }
            None => {}
        }

        if updated {
            actor.try_send(&mut updates_out, update);
        }






    }

    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}