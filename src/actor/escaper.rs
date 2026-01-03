use std::ops::{Add, Mul, Sub};
use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;

use crate::action::utils::*;
use crate::action::workshift::CompletedPoint;
use crate::actor::work_collector::*;
use crate::action::workshift::*;
use crate::action::settings::*;

use crate::action::serialize::*;

pub(crate) const BAILOUT_MAX_ITERATIONS:usize = 100;


pub(crate) enum ScreenValue {
    Outside{
        escape_time:u32
        , small_time: u32
        , smallness:f64
    },
    Inside{
        small_time: u32
        , loop_period: u32
        , smallness:f64
        , largeness:f64
        , big_time: u32
    }
}

#[derive(Clone, Debug)]

pub(crate) struct ZoomerScreen {
    pub(crate) pixels: Vec<(u8,u8,u8)>
    , pub(crate) screen_size: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}

pub(crate) struct ZoomerValuesScreen {
    pub(crate) values: Vec<ScreenValue>
    , pub(crate) res: (u32, u32)
    , pub(crate) objective_location: ObjectivePosAndZoom
}


pub(crate) struct EscaperState {
    pub(crate) settings:Settings
}

pub async fn run(
    actor: SteadyActorShadow,
    points_in: SteadyRx<Serial<CompletedPoint<f64>>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<Serial<ScreenValue>>,
    state: SteadyState<EscaperState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&settings_in, &points_in], [&values_out]),
        points_in,
        settings_in,
        values_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor, T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy + Send>(
    mut actor: A,
    points_in: SteadyRx<Serial<CompletedPoint<f64>>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<Serial<ScreenValue>>,
    state: SteadyState<EscaperState<T>>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = points_in.lock().await;
    let mut screens_out = values_out.lock().await;
    let mut settings_in = settings_in.lock().await;

    let mut state = state.lock(|| EscaperState {
        values: None
        , settings: Settings::DEFAULT
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
            actor.wait_avail(&mut settings_in, 1),
        );

        let mut radius = state.settings.bailout_radius.clone().determine();
        if radius.is_infinite() || radius<2.0 {panic!("invalid radius");radius=2.0};

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
                let pos = pos_from_index(i, v.res.0);
                let value = get_value_from_point(point, radius as f32, pos, &r, v.res, state.settings.clone());
                output.push(value);
            }

            //info!("done escaping. result is {} pixels long.", output.len());


            actor.try_send(&mut screens_out, ZoomerValuesScreen{
                values: output
                , res: v.res
                , objective_location:  v.location.clone()
            });
            //info!("sent colors to window");
        }
    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}

fn get_value_from_point<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
    (p: &CompletedPoint<T>, r: f32, pos:(i32, i32), points: &Vec<CompletedPoint<T>>, res: (u32, u32), settings:Settings) -> ScreenValue {
    match p {
        CompletedPoint::Escapes{escape_time: t, escape_location: z, start_location: c , smallness:s, small_time:st} => {

            let r_squared = r*r;
            let mut p = Point{
                c: *c
                , z: *z
                , real_squared: z.0 * z.0
                , imag_squared: z.1 * z.1
                , iterations: t.clone()
                , real_imag: z.0 * z.1
                , loop_detection_point: ((0.0.into(), 0.0.into()), 0)
                , escapes: false
                , repeats: false
                , delivered: false
                , period: 0
                , smallness_squared:*s
                , small_time:*st
                , largeness_squared: 0.0.into()
                , big_time: 0
            };

            let max = settings.bailout_max_additional_iterations;
            let mut c = 0;
            let og_count= p.iterations;
            while !bailout_point(&p, r_squared.into()) {
                if c<max {} else {
                    /*if settings.estimate_extra_iterations {
                        /*let real_squared:f64 = p.real_squared.into();
                        let imag_squared:f64 = p.imag_squared.into();
                        let bigness:f64 = (real_squared+imag_squared).sqrt();*/
                        //let shortness = r as f64-2.0;
                        //let closeness = 1.0/((p.c.0 - (-2.0)).abs());
                        //let closeness = 1.0/p.smallness;
                        //p.iterations = og_count + closeness.exp().exp() as u32;

                        let nudge = (p.c.0 - (2.0f32.into())).abs();
                        let additional_iterations = (r as f64 /nudge.into()).log(4.0) as u32;
                        p.iterations+=additional_iterations;
                    }*/
                    break;
                }
                iterate(&mut p);
                update_point_results(&mut p);
                c+=1;
            }

            ScreenValue::Outside{ escape_time: p.iterations, smallness:<T as Into<f64>>::into(*s), small_time:*st}
        }
        CompletedPoint::Repeats{period: p, smallness:s, small_time:st, large_time, largeness} => {

            ScreenValue::Inside{loop_period:*p, smallness:<T as Into<f64>>::into(*s), small_time:*st, big_time:*large_time, largeness:<T as Into<f64>>::into(*largeness)}

        }
        CompletedPoint::Dummy{} => {
            //panic!("completed point was not completed");
            ScreenValue::Inside{loop_period:0, smallness:100.0, small_time:0, big_time:0, largeness:0.0}
        }
    }
}

fn diff(a:(i32, i32), b:(i32, i32)) -> (i32, i32) {
    (a.0-b.0, a.1-b.1)
}
