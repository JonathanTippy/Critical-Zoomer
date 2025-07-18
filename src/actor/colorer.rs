use rand::Rng;
use steady_state::*;




use crate::actor::window::*;
use crate::actor::updater::*;
use crate::actor::worker::*;

use crate::action::settings::*;

#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: Vec<(u8,u8,u8)>
    , pub(crate) location: (String, String)
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) zoom_factor_pot: i64
    , pub(crate) state_revision: u64
}


pub(crate) struct ColorerState {
    //pub(crate) zoomer_state: ZoomerState,
    //pub(crate) settings: ZoomerSettingsState,
    pub(crate) values:ZoomerScreenValues,
    pub(crate) bucket:Vec<Vec<(u8,u8,u8)>>
}

pub async fn run(
    actor: SteadyActorShadow,
    values_in: SteadyRx<ZoomerScreenValues>,
    buckets_in: SteadyRx<Vec<Vec<(u8,u8,u8)>>>,
    updates_in: SteadyRx<ZoomerUpdate>,
    screens_out: SteadyTx<(ZoomerScreen)>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in, &values_in], [&screens_out]),
        values_in,
        buckets_in,
        updates_in,
        screens_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    values_in: SteadyRx<ZoomerScreenValues>,
    buckets_in: SteadyRx<Vec<Vec<(u8,u8,u8)>>>,
    updates_in: SteadyRx<ZoomerUpdate>,
    screens_out: SteadyTx<ZoomerScreen>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = values_in.lock().await;
    let mut buckets_in = buckets_in.lock().await;
    let mut updates_in = updates_in.lock().await;
    let mut screens_out = screens_out.lock().await;

    let mut state = state.lock(|| ColorerState {
        values: ZoomerScreenValues {
        values: vec!()
        , location: ("0".to_string(), "0".to_string())
        , screen_size: (2, 2)
        , zoom_factor_pot: 1
        , state_revision: 0
        },
        bucket: vec!()
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
        await_for_any!(  //#!#//
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut values_in, 1),
            actor.wait_avail(&mut updates_in, 1),
            actor.wait_avail(&mut buckets_in, 1)
        );


        // do stuff

        match actor.try_take(&mut buckets_in) {
            Some(mut b) => {
                state.bucket.push(b.pop().unwrap());
            }
            None => {}
        }

        match actor.try_take(&mut values_in) {
            Some(v) => {
                info!("recieved values");
                state.values = v;
                let len = state.values.values.len();
                let mut output = vec!();

                for i in 0..state.values.values.len() {
                    let value = state.values.values[i%len];
                    let color:(u8,u8,u8) = if value == u32::MAX {
                        (0, 0, 0)
                    } else {
                        ((value * 10 % 192) as u8 + 64, (value * 10 % 192) as u8 + 64, (value * 10 % 192) as u8 + 64)
                    };
                    //let color = (255, 255, 255);
                    output.push(color);
                }

                info!("done coloring");

                actor.try_send(&mut screens_out, ZoomerScreen{
                    pixels: output
                    , location: state.values.location.clone()
                    , screen_size: state.values.screen_size
                    , state_revision: state.values.state_revision
                    , zoom_factor_pot: state.values.zoom_factor_pot.clone()
                });
                info!("sent colors to window");

            }
            None => {}
        }



    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}