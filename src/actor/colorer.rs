use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;
use crate::actor::updater::*;

use crate::action::utils::*;

use crate::actor::work_collector::*;

use crate::actor::escaper::*;

#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: Vec<(u8,u8,u8)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}


pub(crate) struct ColorerState {
    pub(crate) values:Option<ZoomerValuesScreen>,
    pub(crate) start:Instant
}

pub async fn run(
    actor: SteadyActorShadow,
    values_in: SteadyRx<ZoomerValuesScreen>,
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
    values_in: SteadyRx<ZoomerValuesScreen>,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    screens_out: SteadyTx<ZoomerScreen>,
    state: SteadyState<ColorerState>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = values_in.lock().await;
    let mut updates_in = updates_in.lock().await;
    let mut screens_out = screens_out.lock().await;

    let mut state = state.lock(|| ColorerState {
        values: None,
        start: Instant::now()
    }).await;

    // Lock all channels for exclusive access within this actor.

    let max_sleep = Duration::from_millis(8);

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

        let elapsed = state.start.elapsed().as_millis();

        //let radius:f64 = 2.0 + (((elapsed % 10000) as f64 / 10000.0) * 4.0);

        let u_1:f64 = 10.0;
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

        let u = 25.0;

        // do stuff

        if actor.avail_units(&mut values_in) > 0 {
            while actor.avail_units(&mut values_in) > 1 {
                let stuff = actor.try_take(&mut values_in).expect("internal error");
                drop(stuff);
            };
            match actor.try_take(&mut values_in) {
                Some(v) => {
                    let mut rng = rand::thread_rng();
                    //info!("recieved values");
                    state.values = Some(v);
                }
                None => {}
            }
        }

        if let Some(v) = &mut state.values {
            let r = &v.values;
            let len = r.len();
            let mut output = vec!();

            let bright:f64 = 128.0;
            let dim:f64 = 64.0;
            let brim:f64 = bright-dim;

            for i in 0..r.len() {
                let value = &r[i%len];
                let color:(u8,u8,u8) = match value {
                    ScreenValue::Inside{loop_period: p} => {
                        ((p*10) as u8, 0, 0)
                    }
                    ScreenValue::Outside { escape_time: e } => {

                        let m = (*e as f64 % u)/u;
                        let m_pi = m * 6.28 + t_pi;
                        let e_sin = (m_pi.sin() + 1.0)/2.0;


                        let b =
                            (e_sin * brim+dim) as u8;
                        (b,b,b)
                    }
                };
                //let color = (255, 255, 255);
                output.push(color);
            }

            //info!("done coloring. result is {} pixels long.", output.len());


            actor.try_send(&mut screens_out, ZoomerScreen{
                pixels: output
                , screen_size: v.screen_size.clone()
                , objective_location:  v.objective_location.clone()
            });
            //info!("sent colors to window");
        }





    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}