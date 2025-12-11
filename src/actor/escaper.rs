use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;
use crate::actor::updater::*;

use crate::action::utils::*;
use crate::action::workshift::CompletedPoint;
use crate::actor::work_collector::*;
use crate::action::workshift::*;

#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: Vec<(u8,u8,u8)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}

pub(crate) struct ZoomerValuesScreen {
    pub(crate) values: Vec<ScreenValue>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}


pub(crate) struct EscaperState {
    pub(crate) values:Option<ResultsPackage>,
    pub(crate) start:Instant
}

pub async fn run(
    actor: SteadyActorShadow,
    points_in: SteadyRx<ResultsPackage>,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    values_out: SteadyTx<ZoomerValuesScreen>,
    state: SteadyState<EscaperState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&updates_in, &points_in], [&values_out]),
        points_in,
        updates_in,
        values_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    points_in: SteadyRx<ResultsPackage>,
    updates_in: SteadyRx<ZoomerSettingsUpdate>,
    values_out: SteadyTx<ZoomerValuesScreen>,
    state: SteadyState<EscaperState>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = points_in.lock().await;
    let mut updates_in = updates_in.lock().await;
    let mut screens_out = values_out.lock().await;

    let mut state = state.lock(|| EscaperState {
        values: None
        , start: Instant::now()
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


        // do stuff

        let elapsed = state.start.elapsed().as_millis();

        //let radius:f64 = 2.0 + (((elapsed % 10000) as f64 / 10000.0) * 4.0);

        let r_1:f64 = 2.0;
        let r_2:f64 = 2.0f64.powf(8.0);
        let t_p = 10000;
        let t = ((elapsed % t_p) as f64 / t_p as f64);
        let t_pi = t * 6.28;
        let t_sin = (t_pi.sin() + 1.0)/2.0;


        /*let r_diff:f64 = r_2-r_1;
        let radius = (r_1-1.0) + 2.0f64.powf(
            r_diff.log(2.0) * t_sin
        );*/

        // correct from first principles: linear motion in log(log(radius))
        let loglog_r1 = (r_1.ln()).ln();
        let loglog_r2 = (r_2.ln()).ln();
        let loglog_r = loglog_r1 + (loglog_r2 - loglog_r1) * t_sin;
        let radius = (loglog_r.exp()).exp();



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

        if let Some(v) = &state.values {
            //let rp = v
            let r = &v.results;
            let len = r.len();
            let mut output = vec!();

            for i in 0..r.len() {
                let point = &r[i%len];
                let value = get_value_from_point(point, radius as f32);
                output.push(value);
            }

            //info!("done escaping. result is {} pixels long.", output.len());


            actor.try_send(&mut screens_out, ZoomerValuesScreen{
                values: output
                , screen_size: v.screen_res
                , objective_location:  v.location.clone()
            });
            //info!("sent colors to window");
        }
    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}

fn get_value_from_point(p: &CompletedPoint, r: f32) -> ScreenValue {
    match p {
        CompletedPoint::Escapes{escape_time: t, escape_location: z, start_location: c} => {
            /*let z = (
                i16_to_f32(z.0)
                , i16_to_f32(z.1)
                );
            let c = (
                i16_to_f32(c.0)
                , i16_to_f32(c.1)
            );*/

            //let r:f32 = 256.0;
            let r_squared = r*r;
            let mut p = PointF32{
                c: *c
                , z: *z
                , real_squared: z.0 * z.0
                , imag_squared: z.1 * z.1
                , iterations: t.clone()
                , real_imag: z.0 * z.1
                , loop_detection_points: [(0.0, 0.0);5]
                , done: (false, false)
                , delivered: false
                };

            while !bailout_point_f32(&p, r_squared) {
                iterate_f32(&mut p);
                update_point_results_f32(&mut p);
            }
            ScreenValue::Outside{escape_time: p.iterations}

        }
        CompletedPoint::Repeats{period: p} => {
            ScreenValue::Inside{loop_period:*p}
        }
        CompletedPoint::Dummy{} => {
            //panic!("completed point was not completed");
            ScreenValue::Inside{loop_period:0}
        }
    }
}