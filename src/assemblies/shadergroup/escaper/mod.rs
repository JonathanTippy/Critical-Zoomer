use std::ops::{Add, Mul, Sub};
use rand::Rng;
use steady_state::*;
use crate::assemblies::headgroup::window::sampling::*;

use crate::utils::*;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::assemblies::workgroup::screen_worker::workshift::CompletedPoint;
use crate::assemblies::workgroup::work_collector::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::settings::*;

use crate::assemblies::structs::*;
use std::sync::Arc;

pub mod gpu;

pub const BAILOUT_MAX_ITERATIONS:usize = 100;




pub enum ScreenValue {
    Outside{
        big_time:u32
        , small_time: u32
        , smallness:f64
        , gradient_angle: f32
    },
    Inside{
        small_time: u32
        , loop_period: u32
        , smallness:f64
    }
}

#[derive(Clone, Debug)]

pub struct ZoomerScreen {
    pub pixels: Vec<(u8, u8, u8)>
    , pub screen_size: (u32, u32)
    , pub objective_location: ObjectivePosAndZoom
}

pub struct ZoomerValuesScreen {
    pub values: Vec<ScreenValue>
    , pub res: (u32, u32)
    , pub objective_location: ObjectivePosAndZoom
    // r[impl cz.depth.gear-hud+2]
    , pub hud: crate::assemblies::structs::ViewHud
}


pub struct EscaperState<T> {
    pub values: Option<ResultsPackage<T>>,
    pub settings: Settings,
    /// Full-frame packages discarded by drain-to-newest when the shade path
    /// falls behind (`r[cz.craft.shade-coalesce-drop-count+1]`).
    pub packages_dropped: u64,
    pub gpu: Option<Arc<gpu::GpuEscaper>>,
    /// New answers package → re-upload resident GPU buffer.
    pub answers_dirty: bool,
}

/// How many queued inputs to drop when keeping only the newest.
/// `avail == 0` → 0; `avail == 1` → 0; `avail == n` → n−1.
// r[impl cz.craft.shade-coalesce-drop-count+1]
pub fn coalesce_drop_count(avail_units: usize) -> usize {
    avail_units.saturating_sub(1)
}

/// One full-frame escape pass — the only shade-path body the escaper runs.
/// Animated bailout uses this same path; only `radius` / settings numbers change.
/// (`docs/assistant/design/shadergroup-virtues.md`)
// r[impl cz.craft.shade-single-path+1]
pub fn escape_frame<T>(
    package: &ResultsPackage<T>,
    radius: f32,
    settings: &Settings,
) -> ZoomerValuesScreen
where
    T: Sub<Output = T>
        + Add<Output = T>
        + Mul<Output = T>
        + Into<f64>
        + PartialOrd
        + Finite
        + Gt
        + Abs
        + From<f32>
        + Copy,
{
    let r = &package.results;
    let mut output = Vec::with_capacity(r.len());
    for i in 0..r.len() {
        let point = &r[i];
        let pos = pos_from_index(i, package.screen_res.0);
        output.push(get_value_from_point(
            point,
            radius,
            pos,
            r,
            package.screen_res,
            settings,
        ));
    }
    ZoomerValuesScreen {
        values: output,
        res: package.screen_res,
        objective_location: package.location.clone(),
        hud: package.hud,
    }
}

pub async fn run(
    actor: SteadyActorShadow,
    answers_in: SteadyRx<View<Answer>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<ZoomerValuesScreen>,
    state: SteadyState<EscaperState<f64>>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&settings_in, &answers_in], [&values_out]),
        answers_in,
        settings_in,
        values_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor, T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + From<f64> + Into<f64> + Finite + Gt + Abs + From<f32> + Into<f64> + Copy + Send>(
    mut actor: A,
    answers_in: SteadyRx<View<Answer>>,
    settings_in: SteadyRx<Settings>,
    values_out: SteadyTx<ZoomerValuesScreen>,
    state: SteadyState<EscaperState<T>>,
) -> Result<(), Box<dyn Error>> {
    let mut values_in = answers_in.lock().await;
    let mut screens_out = values_out.lock().await;
    let mut settings_in = settings_in.lock().await;

    let mut state = state.lock(|| EscaperState {
        values: None,
        settings: Settings::DEFAULT,
        packages_dropped: 0,
        gpu: gpu::GpuEscaper::shared(),
        answers_dirty: true,
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

        if actor.avail_units(&mut settings_in) > 0 {
            while actor.avail_units(&mut settings_in) > 1 {
                let stuff = actor.try_take(&mut settings_in).expect("internal error");
                drop(stuff);
            };
            match actor.try_take(&mut settings_in) {
                Some(s) => {
                    let mut rng = rand::thread_rng();
                    let _ = rng;
                    state.settings = s;
                    // Bailout / max_extra may have changed — re-escape.
                    state.answers_dirty = true;
                }
                None => {}
            }
        }

        // Mutate in place so animated bailout can latch `Animable::start`.
        let mut radius = state.settings.bailout_radius.determine();
        if radius.is_infinite() || radius < 2.0 {
            panic!("invalid radius");
        }
        let radius_dirty = state.settings.bailout_radius.animated;

        let avail = actor.avail_units(&mut values_in);
        if avail > 0 {
            let drops = coalesce_drop_count(avail);
            state.packages_dropped = state.packages_dropped.saturating_add(drops as u64);
            for _ in 0..drops {
                let stuff = actor.try_take(&mut values_in).expect("internal error");
                drop(stuff);
            }
            match actor.try_take(&mut values_in) {
                Some(v) => {
                    let location_f64: (f64, f64) = (
                        v.stencil.location.clone().0.into(),
                        (v.stencil.location.clone().1).into(),
                    );
                    let space_f64: f64 = IntExp::from(1)
                        .shift(-v.stencil.location.2 - PIXELS_PER_UNIT_POT)
                        .into();

                    let mut hud = v.hud;
                    hud.packages_dropped = state.packages_dropped;
                    state.values = Some(ResultsPackage {
                        results: v
                            .data
                            .into_iter()
                            .enumerate()
                            .map(|(i, x)| -> CompletedPoint<T> {
                                match x.result {
                                    MandelbrotResult::Inside { period } => {
                                        CompletedPoint::<T>::Repeats {
                                            period: period as u32,
                                            smallness: x.min_magnitude.into(),
                                            small_time: x.min_magnitude_time as u32,
                                        }
                                    }
                                    MandelbrotResult::Outside {
                                        escape_time_r2,
                                        escape_z,
                                        escape_dc,
                                    } => CompletedPoint::<T>::Escapes {
                                        escape_time: escape_time_r2 as u32,
                                        escape_location: (escape_z.0.into(), escape_z.1.into()),
                                        escape_derivative: (escape_dc.0.into(), escape_dc.1.into()),
                                        smallness: x.min_magnitude.into(),
                                        small_time: x.min_magnitude_time as u32,
                                        start_location: (
                                            (location_f64.0
                                                + v.stencil.clone().seat_and_row(i).0 as f64
                                                    * space_f64)
                                                .into(),
                                            (location_f64.1
                                                - v.stencil.clone().seat_and_row(i).1 as f64
                                                    * space_f64)
                                                .into(),
                                        ),
                                    },
                                }
                            })
                            .collect(),
                        screen_res: (
                            v.stencil.resolution.0 as u32,
                            v.stencil.resolution.1 as u32,
                        ),
                        location: ObjectivePosAndZoom {
                            pos: (v.stencil.location.0, IntExp::ZERO - v.stencil.location.1),
                            zoom_pot: v.stencil.location.2,
                        },
                        hud,
                    });
                    state.answers_dirty = true;
                }
                None => {}
            }
        }

        // Same path every wake (including animated bailout): only numbers change.
        // Static + unchanged: skip try_send (mechanical sympathy / small channels).
        let want_gpu = matches!(
            state.settings.resolved_escape_gear(),
            EscaperMode::Gpu
        );
        let dirty = state.answers_dirty || radius_dirty;
        if dirty {
            if let Some(v) = &state.values {
                let upload = state.answers_dirty;
                let (mut screen, escape_hud) = gpu::escape_with_gear(
                    v,
                    radius as f32,
                    &state.settings,
                    want_gpu,
                    &state.gpu,
                    upload,
                );
                screen.hud.packages_dropped = state.packages_dropped;
                screen.hud.escape = escape_hud;
                if !actor.is_full(&mut screens_out) {
                    screen.hud.escape_emitted_at = Some(std::time::Instant::now());
                    match actor.try_send(&mut screens_out, screen) {
                        SendOutcome::Success => {
                            state.answers_dirty = false;
                            if let Some(v) = &mut state.values {
                                v.hud.clear_emission_stamps();
                            }
                        }
                        SendOutcome::Blocked(_)
                        | SendOutcome::Timeout(_)
                        | SendOutcome::Closed(_) => {}
                    }
                }
            }
        }
    }

    // Final shutdown log, reporting all statistics.
    info!("Colorer shutting down.");
    Ok(())
}

pub fn get_value_from_point<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T>+ Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy>
    (p: &CompletedPoint<T>, r: f32, pos:(i32, i32), points: &Vec<CompletedPoint<T>>, res: (u32, u32), settings:&Settings) -> ScreenValue {
    match p {
        CompletedPoint::Escapes{escape_time: t, escape_location: z, escape_derivative: escape_dc, start_location: c , smallness:s, small_time:st} => {

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
                        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {
                            
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
                delta_c: *c
                , c: *c
                , z: *z
                , dc: *escape_dc
                , real_squared: z.0 * z.0
                , imag_squared: z.1 * z.1
                , iterations: t.clone()
                , real_imag: z.0 * z.1
                , loop_detection_point: ((0.0.into(), 0.0.into()), 0)
                , escapes: false
                , repeats: false
                , delivered: false
                , initialized: true
                , period: 0
                , smallness_squared:*s
                , small_time:*st
                , delta: None
                , direct_only: false
                , bound_zero_generation: 0
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
                        //let closeness = 1.0/((p.delta_c.0 - (-2.0)).abs());
                        //let closeness = 1.0/p.smallness;
                        //p.iterations = og_count + closeness.exp().exp() as u32;

                        let nudge = (p.delta_c.0 - (2.0f32.into())).abs();
                        let additional_iterations = (r as f64 /nudge.into()).log(4.0) as u32;
                        p.iterations+=additional_iterations;
                    }*/
                    break;
                }
                iterate(&mut p);
                update_point_results(&mut p);
                c+=1;
            }

            let zr: f64 = p.z.0.into();
            let zi: f64 = p.z.1.into();
            let dr: f64 = p.delta_c.0.into();
            let di: f64 = p.delta_c.1.into();
            // arg(z / dc), reflected because screen y grows downward.
            let gradient_angle = (-(zi * dr - zr * di)).atan2(zr * dr + zi * di) as f32;
            ScreenValue::Outside{
                big_time: p.iterations,
                smallness:<T as Into<f64>>::into(*s),
                small_time:*st,
                gradient_angle,
            }
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
                        CompletedPoint::Escapes{escape_time: t, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {}
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
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {
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
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
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
            CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
            CompletedPoint::Dummy{} => {100.0f32.into()}
        }.into();

        let s2:f64 = match points[index_from_pos(&n2, res.0)] {
            CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
            CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
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
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {st}
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
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {st}
                CompletedPoint::Dummy{} => {0}
            };

            let st2 = match points[index_from_pos(&n2, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {st}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {st}
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
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
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
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns12:f64 = match points[index_from_pos(&n.0.1, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns21:f64 = match points[index_from_pos(&n.1.0, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
                CompletedPoint::Dummy{} => {100.0f32.into()}
            }.into();
            let ns22:f64 = match points[index_from_pos(&n.1.1, res.0)] {
                CompletedPoint::Repeats{period: np, smallness:s, small_time:st} => {s}
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
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
        CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:s, small_time:st, ..} => {s}
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
                CompletedPoint::Escapes{escape_time: nt, escape_location: z, start_location: c, smallness:ns, small_time:st, ..} => {
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

#[cfg(test)]
mod mutant_kill {
    use super::*;

    /// Thought-killed pins for escaper neighbor diffs (shadergroup helpers only).
    #[test]
    fn mutant_kill_escaper_diff_helpers() {
        assert_eq!(diff((5, 7), (2, 3)), (3, 4));
        assert_eq!(diff((2, 3), (5, 7)), (-3, -4));
        assert_ne!(diff((5, 7), (2, 3)), (5 + 2, 7 + 3)); // -→+
        assert_ne!(diff((5, 7), (2, 3)), (5 * 2, 7 * 3));

        assert_eq!(difff32((1.5, -2.0), (0.5, 1.0)), (1.0, -3.0));
        assert_ne!(difff32((1.5, -2.0), (0.5, 1.0)), (2.0, -1.0)); // -→+
    }

    // r[verify cz.craft.shade-coalesce-drop-count+1]
    #[test]
    fn coalesce_drop_count_keeps_newest_only() {
        assert_eq!(coalesce_drop_count(0), 0);
        assert_eq!(coalesce_drop_count(1), 0);
        assert_eq!(coalesce_drop_count(2), 1);
        assert_eq!(coalesce_drop_count(50), 49);
    }
}