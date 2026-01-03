use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;

use crate::action::utils::*;

use crate::actor::work_collector::*;

use crate::actor::escaper::*;

use crate::action::settings::*;

use crate::action::color::*;

use crate::action::serialize::*;
use crate::action::constants::*;
use crate::action::serialize::*;
use crate::action::workshift::CompletedPoint;

#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: DoubleBuffer<(u8, u8, u8)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}


pub(crate) struct ColorerState {
    pub(crate) settings:Settings
}

pub async fn run(
    actor: SteadyActorShadow,
    values_in: SteadyRx<Serial<ScreenValue>>,
    settings_in: SteadyRx<Settings>,
    colors_out: SteadyTx<Serial<(u8, u8, u8)>>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&settings_in, &values_in], [&colors_out]),
        values_in,
        settings_in,
        colors_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    values_in: SteadyRx<Serial<ScreenValue>>,
    settings_in: SteadyRx<Settings>,
    colors_out: SteadyTx<Serial<(u8,u8,u8)>>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = values_in.lock().await;
    let mut colors_out = colors_out.lock().await;
    let mut settings_in = settings_in.lock().await;

    let mut state = state.lock(|| ColorerState {
        values: None,
        start: Instant::now(),
        settings: Settings::DEFAULT
    }).await;

    // Lock all channels for exclusive access within this actor.

    let max_sleep = Duration::from_millis(8);

    // Main processing loop.
    // The actor runs until all input channels are closed and empty, and the output channel is closed.
    while actor.is_running(
        || i!(true)
    ) {
        await_for_any!(//#!#//
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut values_in, 1),
            actor.wait_avail(&mut settings_in, 1),
        );




        let elapsed = state.start.elapsed().as_millis();

        //let radius:f64 = 2.0 + (((elapsed % 10000) as f64 / 10000.0) * 4.0);

        /*let u_1:f64 = 10.0;
        let u_2:f64 = 100.0;
        let t_p = 10000;
        let t = ((elapsed % t_p) as f64 / t_p as f64);
        let t_pi = t * 6.28;
        let t_sin = (t_pi.sin() + 1.0)/2.0;

        //let u = u_1 + t_sin * (u_2 - u_1);

        let loglog_u1 = (u_1.ln()).ln();
        let loglog_u2 = (u_2.ln()).ln();
        let loglog_u = loglog_u1 + (loglog_u2 - loglog_u1) * t_sin;
        let u = (loglog_u.exp()).exp();

        let u = 25.0;*/

        // do stuff

        if actor.avail_units(&mut settings_in) > 0 {
            while actor.avail_units(&mut settings_in) > 1 {
                let stuff = actor.try_take(&mut settings_in).expect("internal error");
                drop(stuff);
            };
            match actor.try_take(&mut settings_in) {
                Some(s) => {
                    let mut rng = rand::thread_rng();
                    state.settings = s;
                }
                None => {}
            }
        }

        if actor.avail_units(&mut values_in) > 0 {
            while actor.avail_units(&mut values_in) > 1 {
                let stuff = actor.try_take(&mut values_in).expect("internal error");
                drop(stuff);
            };
            match actor.try_take(&mut values_in) {
                Some(v) => {
                    state.values = Some(v);
                }
                None => {}
            }
        }

        let mut settings = state.settings.clone();
        if let Some(v) = &mut state.values {
            let output = color(v, &mut settings);

            actor.try_send(&mut colors_out, ZoomerScreen{
                pixels: output
                , screen_size: v.res.clone()
                , objective_location:  v.objective_location.clone()
            });
            //info!("sent colors to window");
        }
        state.settings = settings;





    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}