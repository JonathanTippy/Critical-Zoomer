use eframe::epaint::Color32;
use steady_state::*;

use crate::assemblies::headgroup::window::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::workgroup::screen_worker::*;
use crate::constants::*;
use crate::constants::*;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::c_generator::Mandelbrotable;

use rand::prelude::SliceRandom;
use crate::utils::*;


#[derive(Clone, Debug)]

pub struct ResultsPackage<T> {
    pub results: Vec<CompletedPoint<T>>
    , pub screen_res: (u32, u32)
    , pub location: ObjectivePosAndZoom
}

pub struct WorkCollectorState<T> {
    completed_work: Option<ResultsPackage<T>>
    , surrounding_work: Option<ResultsPackage<T>>
}


pub const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub const WORKER_INIT_ZOOM_POT: i64 = -2;
pub const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub const PIXELS_PER_UNIT_POT:i32 = 9;
pub const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

fn map_results_to_answers<T: crate::assemblies::workgroup::c_generator::Mandelbrotable>(
    results: &[CompletedPoint<T>],
) -> Vec<Answer> {
    results
        .iter()
        .map(|x| match x {
            CompletedPoint::Escapes {
                escape_time,
                escape_location,
                escape_derivative,
                smallness,
                small_time,
                ..
            } => {
                let ez0 = escape_location.0.to_f64();
                let ez1 = escape_location.1.to_f64();
                let ed0 = escape_derivative.0.to_f64();
                let ed1 = escape_derivative.1.to_f64();
                let mag = smallness.to_f64();
                Answer {
                    result: MandelbrotResult::Outside {
                        escape_time_r2: *escape_time as u64,
                        escape_z: (ez0 as f32, ez1 as f32),
                        escape_dc: (ed0 as f32, ed1 as f32),
                    },
                    min_magnitude_time: *small_time as u64,
                    min_magnitude: mag,
                }
            }
            CompletedPoint::Repeats {
                period,
                smallness,
                small_time,
            } => {
                let mag = smallness.to_f64();
                Answer {
                    result: MandelbrotResult::Inside {
                        period: *period as u64,
                    },
                    min_magnitude_time: *small_time as u64,
                    min_magnitude: mag,
                }
            }
            CompletedPoint::Dummy {} => Answer {
                result: MandelbrotResult::Inside { period: 0 },
                min_magnitude_time: 0,
                min_magnitude: 0.0,
            },
        })
        .collect()
}

fn view_from_completed_work<T: Mandelbrotable>(
    completed_work: &ResultsPackage<T>,
) -> View<Answer> {
    View {
        stencil: PointStencil {
            location: (
                completed_work.location.clone().pos.0,
                IntExp::ZERO - completed_work.location.clone().pos.1,
                completed_work.location.zoom_pot,
            ),
            resolution: (
                completed_work.screen_res.0 as usize,
                completed_work.screen_res.1 as usize,
            ),
            serial_number: 0,
        },
        data: map_results_to_answers(&completed_work.results),
        bitmap: vec![0; completed_work.results.len()],
    }
}



pub async fn run(
    actor: SteadyActorShadow,
    from_worker: SteadyRx<WorkUpdate<crate::floatexp::FloatExp>>,
    answers_out: SteadyTx<View<Answer>>,
    state: SteadyState<WorkCollectorState<crate::floatexp::FloatExp>>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_worker], [&answers_out]),
        from_worker,
        answers_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_worker: SteadyRx<WorkUpdate<crate::floatexp::FloatExp>>,
    answers_out: SteadyTx<View<Answer>>,
    state: SteadyState<WorkCollectorState<crate::floatexp::FloatExp>>,
) -> Result<(), Box<dyn Error>> {

    let mut values_out = answers_out.lock().await;
    let mut from_worker = from_worker.lock().await;

    let mut state = state.lock(|| WorkCollectorState {
        completed_work: None
        , surrounding_work: None
    }).await;

    let max_sleep = Duration::from_millis(50);

    let mut publish_pending = false;

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {

        await_for_any!(//#!#//
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_worker, 1),
        );

        // r[impl cz.craft.drain-to-newest+1]
        while actor.avail_units(&mut from_worker) > 0 {
            let U = actor
                .try_take(&mut from_worker)
                .expect("work update seemed available but wasn't...");

            if let Some(surrounding_work) = &mut state.surrounding_work {
                if let Some(mut f) = U.frame_info.clone() {
                    f.0.zoom_pot -= 1;
                    *surrounding_work = sample_old_values(&surrounding_work, f.0, f.1);
                }
            }

            if let Some(completed_work) = &mut state.completed_work {
                if let Some(f) = U.frame_info {
                    *completed_work = sample_old_values(&completed_work, f.0, f.1);
                    publish_pending = true;
                } else {
                    let vs = U.completed_points;
                    for W in vs {
                        completed_work.results[W.1] = W.0;
                    }
                    publish_pending = true;
                }
            } else if let Some(f) = U.frame_info {
                state.completed_work = Some(ResultsPackage {
                    results: vec![CompletedPoint::Dummy {}; (f.1.0 * f.1.1) as usize],
                    screen_res: f.1,
                    location: f.0,
                });
                if let Some(completed_work) = &mut state.completed_work {
                    let vs = U.completed_points;
                    for W in vs {
                        completed_work.results[W.1] = W.0;
                    }
                    publish_pending = true;
                }
            }
        }
        // Retry blocked escaper publishes on periodic wake as well as new worker data.
        // r[impl cz.craft.undeliver-on-full+1]
        if publish_pending {
            if let Some(completed_work) = &state.completed_work {
                if matches!(
                    actor.try_send(&mut values_out, view_from_completed_work(completed_work)),
                    SendOutcome::Success
                ) {
                    publish_pending = false;
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

// r[impl cz.craft.clamped-remap-smear+1]
// r[impl cz.craft.shared-remap-transform+1]
pub(crate) fn sample_old_values<T:Clone>(old_package: &ResultsPackage<T>, new_location: ObjectivePosAndZoom, new_res: (u32, u32)) -> ResultsPackage<T> {
    let mut returned = ResultsPackage{
        results: vec!()
        , screen_res: new_res
        , location: new_location.clone()
    };

    let old_size = old_package.screen_res.0 * old_package.screen_res.1;

    //let old_package_pixel_width = old_package.location.zoom_pot

    let relative_pos = (
        old_package.location.pos.0.clone()-new_location.pos.0.clone()
        , old_package.location.pos.1.clone()-new_location.pos.1.clone()
    );

    let relative_pos_in_pixels:(i32, i32) = (
        relative_pos.0.clone().shift(new_location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
        , relative_pos.1.clone().shift(new_location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
    );

    let relative_zoom = new_location.zoom_pot - old_package.location.zoom_pot;

    /*let relative_pos_in_pixels = (
        relative_pos_in_pixels.0 - shift(1, relative_zoom-1)
        , relative_pos_in_pixels.1 - shift(1, relative_zoom-1)
    );*/

    for row in 0..new_res.1 as usize {
        for seat in 0..new_res.0 as usize {
            returned.results.push(
                sample_value(
                    &old_package.results
                    , old_package.screen_res
                    , old_size as usize
                    , row
                    , seat
                    , relative_pos_in_pixels
                    , relative_zoom as i64
                )
            );
            //i+=1;
        }
    }
    returned
}


/*fn get_values_from_points(ps: Vec<(CompletedPoint, usize)>) -> Vec<(CompletedPoint, usize)> {
    let mut returned = vec!();
    for p in ps {
        returned.push(((p.0), p.1));
    }
    returned
}*/





fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}




#[inline]
fn sample_value<T: Clone>(
    pixels: &Vec<CompletedPoint<T>>
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> CompletedPoint<T> {
    let color =
        pixels[
            index_from_relative_location(
                transform_relative_location_i32(
                    relative_location_i32_row_and_seat(seat, row)
                    , (relative_pos.0, relative_pos.1)
                    , relative_zoom_pot
                )
                , data_res
                , data_len
            )
            ].clone();
    color
}