use std::ops::{Add, Mul, Sub};
use rand::Rng;
use steady_state::*;
use crate::action::sampling::*;

use crate::action::utils::*;
use crate::action::workshift::CompletedPoint;
use crate::actor::work_collector::*;
use crate::action::workshift::*;
use crate::action::settings::*;


pub(crate) const BAILOUT_MAX_ITERATIONS:usize = 100;


pub(crate) enum ScreenValue {
    Outside{
        big_time:u32
        , small_time: u32
        , smallness:f64
    },
    Inside{
        small_time: u32
        , loop_period: u32
        , smallness:f64
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


pub(crate) struct EscaperState<T> {
    pub(crate) values:Option<ResultsPackage<T>>,
    pub(crate) settings:Settings
}

pub async fn run(
    actor: SteadyActorShadow,
    points_in: SteadyRx<ResultsPackage<f64>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<ZoomerValuesScreen>,
    state: SteadyState<EscaperState<f64>>,
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
    points_in: SteadyRx<ResultsPackage<T>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<ZoomerValuesScreen>,
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
                let pos = pos_from_index(i, v.screen_res.0);
                let value = get_value_from_point(point, radius as f32, pos, &r, v.screen_res, state.settings.clone());
                output.push(value);
            }

            //info!("done escaping. result is {} pixels long.", output.len());


            actor.try_send(&mut screens_out, ZoomerValuesScreen{
                values: output
                , res: v.screen_res
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

            let neighbors: [(i32, i32);4] =[
                (pos.0, pos.1-1)
                , (pos.0-1, pos.1)
                , (pos.0, pos.1+1)
                , (pos.0+1, pos.1)
            ];

            let mut sign:(Option<i32>, Option<i32>) = (None, None);
            let mut filament = false;
            //let derivative = get_derivative(pos, points, res, *t);

            for n in neighbors {
                if (
                    n.0 >= 0 && n.0 <= res.0 as i32 - 1
                        && n.1 >= 0 && n.1 <= res.1 as i32 - 1
                ) {
                    match points[index_from_pos(&n, res.0)] {
                        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {}
                        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {
                            
                            let difference = (nt as i32)-(*t as i32);
                            let direction = diff(n, pos);
                            let derivative = (direction.0 * difference, direction.1 * difference);
                            if derivative.0!=0 {
                                if let Some(s) = sign.0 {
                                    if s != derivative.0.signum()
                                    {filament = true;}
                                } else {
                                    sign.0 = Some(derivative.0.signum());
                                }
                            }
                            if derivative.1!=0 {
                                if let Some(s) = sign.1 {
                                    if s != derivative.1.signum()
                                    {filament = true;}
                                } else {
                                    sign.1 = Some(derivative.1.signum());
                                }
                            }
                        }
                        CompletedPoint::Dummy{} => {}
                    }
                }
            }

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

            ScreenValue::Outside{ big_time: p.iterations, smallness:<T as Into<f64>>::into(*s), small_time:*st}
        }
        CompletedPoint::Repeats{period: p, smallness:s, small_time:st} => {
            let neighbors: [(i32, i32);4] =[
                (pos.0, pos.1-1)
                , (pos.0-1, pos.1)
                , (pos.0, pos.1+1)
                , (pos.0+1, pos.1)
            ];

            let mut sum = (0, 0);

            let mut diff_sum = 0;

            for n in neighbors {
                if (
                    n.0 >= 0 && n.0 <= res.0 as i32 - 1
                        && n.1 >= 0 && n.1 <= res.1 as i32 - 1
                ) {
                    match points[index_from_pos(&n, res.0)] {
                        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {
                            let difference = (np as i32)-(*p as i32);
                            diff_sum+=difference;
                            let direction = diff(n, pos);
                            let derivative = (direction.0 * difference, direction.1 * difference);
                            sum = (sum.0+derivative.0, sum.1+derivative.1);
                        }
                        CompletedPoint::Escapes{escape_time: t, escape_location: z, start_location: c, smallness:s, small_time:st} => {}
                        CompletedPoint::Dummy{} => {}
                    }
                }
            }

            let avg_derivative = ((sum.0 as f32) / 2.0, (sum.1 as f32)/2.0);


            if diff_sum < 0 {
                ScreenValue::Inside{loop_period:*p, smallness:<T as Into<f64>>::into(*s), small_time:*st}
            } else {
                ScreenValue::Inside{loop_period:*p, smallness:<T as Into<f64>>::into(*s), small_time:*st}
            }

        }
        CompletedPoint::Dummy{} => {
            //panic!("completed point was not completed");
            ScreenValue::Inside{loop_period:0, smallness:100.0, small_time:0}
        }
    }
}

fn diff(a:(i32, i32), b:(i32, i32)) -> (i32, i32) {
    (a.0-b.0, a.1-b.1)
}

fn get_derivative<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
(pos:(i32, i32), points:&Vec<CompletedPoint<T>>,res:(u32,u32), escape_time: u32) -> (f32, f32) {
    let neighbors: [(i32, i32);4] =[
        (pos.0, pos.1-1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0+1, pos.1)
    ];

    let mut sum = (0, 0);

    for n in neighbors {
        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            match points[index_from_pos(&n, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {
                    let difference = (nt as i32)-(escape_time as i32);
                    let direction = diff(n, pos);
                    let derivative = (direction.0 * difference, direction.1 * difference);
                    sum = (sum.0+derivative.0, sum.1+derivative.1);
                }
                CompletedPoint::Dummy{} => {}
            }
        }
    }

    let avg_derivative = ((sum.0 as f32) / 2.0, (sum.1 as f32)/2.0);
    avg_derivative
}




fn is_node<T: From<f32> + Into<f64> + Copy>(pos:(i32, i32), points:&Vec<CompletedPoint<T>>,res:(u32,u32)) -> bool {

    let s:f64 = match points[index_from_pos(&pos, res.0)] {
        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
        CompletedPoint::Dummy{} => {100.0f32.into()}
    }.into();

    let r = 1;
    // Group neighbors by opposite pairs
    let pairs = [
        ((pos.0-r, pos.1), (pos.0+r, pos.1))     // left-right
        , ((pos.0, pos.1-r), (pos.0, pos.1+r))     // up-down
        , ((pos.0-r, pos.1-r), (pos.0+r, pos.1+r)) // diagonal
        , ((pos.0-r, pos.1+r), (pos.0+r, pos.1-r))  // anti-diagonal
        /*, ((pos.0-r, pos.1+r), (pos.0+r, pos.1)) // imperfect pi/8 hori right up
        , ((pos.0-r, pos.1), (pos.0+r, pos.1+r)) // imperfect pi/8 hori right down
        , ((pos.0-r, pos.1), (pos.0+r, pos.1+r)) // imperfect pi/8 hori left up
        , ((pos.0-r, pos.1-r), (pos.0+r, pos.1)) // imperfect pi/8 hori left down
        , ((pos.0, pos.1-r), (pos.0+r, pos.1+r)) // imperfect pi/8 verti right right
        , ((pos.0-r, pos.1-r), (pos.0, pos.1+r)) // imperfect pi/8 verti right left
        , ((pos.0, pos.1-r), (pos.0+r, pos.1+r)) // imperfect pi/8 verti left right
        , ((pos.0-r, pos.1-r), (pos.0, pos.1+r)) // imperfect pi/8 verti left left*/
    ];

    for (n1, n2) in pairs {
        // Check bounds for both neighbors
        if !(n1.0 >= 0 && n1.0 < res.0 as i32 && n1.1 >= 0 && n1.1 < res.1 as i32) {
            continue;
        }
        if !(n2.0 >= 0 && n2.0 < res.0 as i32 && n2.1 >= 0 && n2.1 < res.1 as i32) {
            continue;
        }

        let s1:f64 = match points[index_from_pos(&n1, res.0)] {
            CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
            CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
            CompletedPoint::Dummy{} => {100.0f32.into()}
        }.into();

        let s2:f64 = match points[index_from_pos(&n2, res.0)] {
            CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
            CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
            CompletedPoint::Dummy{} => {100.0f32.into()}
        }.into();

        // For local minimum, both directions should have higher or equal smallness
        if s1 > s && s2 > s {
            return true;
        }
    }

    false
}



fn is_node_tree<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
(pos:(i32, i32), points:&Vec<CompletedPoint<T>>,res:(u32,u32)) -> bool {

    let st = match points[index_from_pos(&pos, res.0)] {
        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {st}
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {st}
        CompletedPoint::Dummy{} => {0}
    };

    let neighbors: [(i32, i32);4] =[
        (pos.0, pos.1-1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0+1, pos.1)
    ];

    let mut sign:(Option<i32>, Option<i32>) = (None, None);
    //let derivative = get_derivative(pos, points, res, *t);

    let r = 1;
        // Group neighbors by opposite pairs
        let pairs = [
            ((pos.0-r, pos.1), (pos.0+r, pos.1))     // left-right
            , ((pos.0, pos.1-r), (pos.0, pos.1+r))     // up-down
            , ((pos.0-r, pos.1-r), (pos.0+r, pos.1+r)) // diagonal
            , ((pos.0-r, pos.1+r), (pos.0+r, pos.1-r))  // anti-diagonal
            /*, ((pos.0-r, pos.1+r), (pos.0+r, pos.1)) // imperfect pi/8 hori right up
            , ((pos.0-r, pos.1), (pos.0+r, pos.1+r)) // imperfect pi/8 hori right down
            , ((pos.0-r, pos.1), (pos.0+r, pos.1+r)) // imperfect pi/8 hori left up
            , ((pos.0-r, pos.1-r), (pos.0+r, pos.1)) // imperfect pi/8 hori left down
            , ((pos.0, pos.1-r), (pos.0+r, pos.1+r)) // imperfect pi/8 verti right right
            , ((pos.0-r, pos.1-r), (pos.0, pos.1+r)) // imperfect pi/8 verti right left
            , ((pos.0, pos.1-r), (pos.0+r, pos.1+r)) // imperfect pi/8 verti left right
            , ((pos.0-r, pos.1-r), (pos.0, pos.1+r)) // imperfect pi/8 verti left left*/
        ];

        for (n1, n2) in pairs {
            // Check bounds for both neighbors
            if !(n1.0 >= 0 && n1.0 < res.0 as i32 && n1.1 >= 0 && n1.1 < res.1 as i32) {
                continue;
            }
            if !(n2.0 >= 0 && n2.0 < res.0 as i32 && n2.1 >= 0 && n2.1 < res.1 as i32) {
                continue;
            }

            let st1 = match points[index_from_pos(&n1, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {st}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {st}
                CompletedPoint::Dummy{} => {0}
            };

            let st2 = match points[index_from_pos(&n2, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {st}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {st}
                CompletedPoint::Dummy{} => {0}
            };

            // For local minimum, both directions should have higher or equal smallness
            if st1 != st{// || st2 != st {
                return true;
            }
        };
    false
}

fn smallness_deriv_deriv_big <T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
(pos:(i32, i32), points:&Vec<CompletedPoint<T>>,res:(u32,u32)) -> bool {

    let s = match points[index_from_pos(&pos, res.0)] {
        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
        CompletedPoint::Dummy{} => {100.0f32.into()}
    };

    let r = 1;
    let neighbors: [(((i32, i32), (i32, i32)),((i32, i32), (i32, i32)));2] =[
        (((pos.0, pos.1-r), (pos.0, pos.1-r-1)), ((pos.0, pos.1+r), (pos.0, pos.1+r+1)))
        , (((pos.0-r, pos.1), (pos.0-r-1, pos.1)), ((pos.0+r, pos.1), (pos.0+r+1, pos.1)))
    ];

    let mut sign:(Option<i32>, Option<i32>) = (None, None);

    let mut happy = false;
    let mut sad = false;

    for n in neighbors {
        if (
            n.0.0.0 >= 0 && n.0.0.0 <= res.0 as i32 - 1
            && n.0.0.1 >= 0 && n.0.0.1 <= res.1 as i32 - 1
            && n.1.0.1 >= 0 && n.1.0.1 <= res.1 as i32 - 1
            && n.1.0.0 >= 0 && n.1.0.0 <= res.0 as i32 - 1
            && n.0.1.0 >= 0 && n.0.1.0 <= res.0 as i32 - 1
            && n.0.1.1 >= 0 && n.0.1.1 <= res.1 as i32 - 1
            && n.1.1.1 >= 0 && n.1.1.1 <= res.1 as i32 - 1
            && n.1.1.0 >= 0 && n.1.1.0 <= res.0 as i32 - 1
        ) {
            let ns11:f64 = match points[index_from_pos(&n.0.0, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns12:f64 = match points[index_from_pos(&n.0.1, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns21:f64 = match points[index_from_pos(&n.1.0, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns22:f64 = match points[index_from_pos(&n.1.1, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let slope1 = ns12-ns11;
            let slope2 = ns21-ns22;
            let slopeslope = slope2-slope1;
            if slopeslope>0.0 {happy=true} else if slopeslope<0.0 {sad=true};


            let avg_slope = (slope1.abs() + slope2.abs())/2.0;

            if slopeslope.abs()/avg_slope > 1.9{
                return true
            }

        }
    }
    false
}

fn difff32 (a:(f32, f32), b:(f32, f32)) -> (f32, f32) {
    (a.0-b.0, a.1-b.1)
}

fn get_smallness_derivative<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
(pos:(i32, i32), points:&Vec<CompletedPoint<T>>,res:(u32,u32)) -> (f32, f32) {

    let s:f64 = match points[index_from_pos(&pos, res.0)] {
        CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st} => {s}
        CompletedPoint::Dummy{} => {100.0f32.into()}
    }.into();

    let r = 1;
    let neighbors: [(i32, i32);4] =[
        (pos.0, pos.1-r)
        , (pos.0-r, pos.1)
        , (pos.0, pos.1+r)
        , (pos.0+r, pos.1)
    ];

    let mut sum = (0.0, 0.0);

    for n in neighbors {
        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let ns:f64 = match points[index_from_pos(&n, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:ns, small_time:st} => {
                    ns
                }
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let difference = ns-s;
            let direction = diff(n, pos);
            let derivative = (direction.0 as f64 * difference, direction.1 as f64 * difference);
            sum = (sum.0+derivative.0, sum.1+derivative.1);
        }
    }

    let avg_derivative = ((sum.0 as f32) / 2.0, (sum.1 as f32)/2.0);
    avg_derivative
}