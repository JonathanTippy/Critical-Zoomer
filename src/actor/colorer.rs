use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;
use crate::actor::updater::*;
use crate::actor::work_controller::*;

use crate::action::utils::*;


#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: Vec<(u8,u8,u8)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}


pub(crate) struct ColorerState {
    pub(crate) values:Option<ResultsPackage>,
}

pub async fn run(
    actor: SteadyActorShadow,
    values_in: SteadyRx<ResultsPackage>,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    screens_out: SteadyTx<ZoomerScreen>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in, &values_in], [&screens_out]),
        values_in,
        updates_in,
        screens_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    values_in: SteadyRx<ResultsPackage>,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    screens_out: SteadyTx<ZoomerScreen>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = values_in.lock().await;
    let mut updates_in = updates_in.lock().await;
    let mut screens_out = screens_out.lock().await;

    let mut state = state.lock(|| ColorerState {
        values: None
    }).await;

    // Lock all channels for exclusive access within this actor.

    let max_sleep = Duration::from_millis(100);

    // Main processing loop.
    // The actor runs until all input channels are closed and empty, and the output channel is closed.
    while actor.is_running(
        || i!(true)
    ) {
        // Wait for all required conditions:
        // - A periodic timer
        await_for_any!(//#!#//
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut values_in, 1),
            actor.wait_avail(&mut updates_in, 1),
        );


        // do stuff

        match actor.try_take(&mut values_in) {
            Some(v) => {
                let mut rng = rand::thread_rng();
                info!("recieved values");
                state.values = Some(v);
                let rp = state.values.as_ref().unwrap();
                let r = &rp.results;
                let len = r.len();
                let mut output = vec!();

                for i in 0..r.len() {
                    let value = &r[i%len];
                    let color:(u8,u8,u8) = match value {
                        ScreenValue::Inside{loop_period: _} => {(0, 0, 0)}
                        ScreenValue::Outside { escape_time: e } => {((e * 10 % 192) as u8 + 64, (e * 10 % 192) as u8 + 64, (e * 10 % 192) as u8 + 64)}
                    };
                    //let color = (255, 255, 255);
                    output.push(color);
                }

                info!("done coloring. result is {} pixels long.", output.len());


                actor.try_send(&mut screens_out, ZoomerScreen{
                    pixels: output
                    , screen_size: state.values.as_ref().unwrap().screen_res.clone()
                    , objective_location:  state.values.as_ref().unwrap().originating_relative_transforms.clone()
                    , dummy: state.values.as_ref().unwrap().dummy.clone()
                    , complete: state.values.as_ref().unwrap().complete.clone()
                });
                //info!("sent colors to window");

            }
            None => {}
        }



    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}