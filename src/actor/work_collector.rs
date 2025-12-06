use eframe::epaint::Color32;
use steady_state::*;

use crate::actor::window::*;
use crate::action::workshift::*;
use crate::action::sampling::*;
use crate::actor::screen_worker::*;


use rand::prelude::SliceRandom;
use crate::action::utils::*;

pub(crate) enum WorkerCommand {
    Update
    , Replace{context: WorkContext}
}
#[derive(Clone, Debug)]

pub(crate) enum ScreenValue {
    Outside{escape_time: u32}
    , Inside{loop_period: u32}
}
#[derive(Clone, Debug)]

pub(crate) struct ResultsPackage {
    pub(crate) results: Vec<CompletedPoint>
    , pub(crate) screen_res: (u32, u32)
    , pub(crate) location: ObjectivePosAndZoom
    , pub(crate) complete: bool
}

pub(crate) struct WorkCollectorState {
    completed_work: Vec<ResultsPackage>
}


pub(crate) const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub(crate) const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub(crate) const PIXELS_PER_UNIT_POT:i32 = 9;
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);



pub async fn run(
    actor: SteadyActorShadow,
    from_worker: SteadyRx<WorkUpdate>,
    points_out: SteadyTx<ResultsPackage>,
    state: SteadyState<WorkCollectorState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_worker], [&points_out]),
        from_worker,
        points_out,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_worker: SteadyRx<WorkUpdate>,
    values_out: SteadyTx<ResultsPackage>,
    state: SteadyState<WorkCollectorState>,
) -> Result<(), Box<dyn Error>> {

    let mut values_out = values_out.lock().await;
    let mut from_worker = from_worker.lock().await;

    let mut state = state.lock(|| WorkCollectorState {
        completed_work: vec!()
    }).await;

    let max_sleep = Duration::from_millis(50);




    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {
        if actor.avail_units(&mut from_worker) > 0 {
            let U =actor.try_take(&mut from_worker).expect("work update seemed available but wasn't...");
            if state.completed_work.len() > 0 {
                if let Some(f) = U.frame_info {
                    state.completed_work[0] = sample_old_values(&state.completed_work[0], f.0, f.1);
                } else {
                    //let j = U.completed_points;
                    let l = U.completed_points.len();
                    //
                    /*for i in j..j+l {
                        if i-j < vs.len() && i < state.completed_work[0].results.len() {
                            state.completed_work[0].results[i] = vs[i-j].clone();
                        }
                    }*/
                    let vs = U.completed_points;
                    for i in 0..l {
                        let W = vs[i].clone();
                        state.completed_work[0].results[W.1] = W.0;
                    }
                    actor.try_send(&mut values_out, state.completed_work[0].clone());
                }
            } else {
                let f = U.frame_info.expect("work collector recieved an initial work update without any info");
                state.completed_work.push(
                    ResultsPackage {
                        results: vec![CompletedPoint::Dummy{}; (f.1.0 * f.1.1) as usize]
                        , screen_res: f.1
                        , location: f.0
                        , complete: false
                    }
                );
                let l = U.completed_points.len();
                let vs = U.completed_points;
                for i in 0..l {
                    let W = vs[i].clone();
                    state.completed_work[0].results[W.1] = W.0;
                }
                actor.try_send(&mut values_out, state.completed_work[0].clone());
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

fn sample_old_values(old_package: &ResultsPackage, new_location: ObjectivePosAndZoom, new_res: (u32, u32)) -> ResultsPackage {
    let mut returned = ResultsPackage{
        results: vec!()
        , screen_res: new_res
        , location: new_location.clone()
        , complete: false
    };

    let old_size = old_package.screen_res.0 * old_package.screen_res.1;

    //let old_package_pixel_width = old_package.location.zoom_pot

    let relative_pos = (
        old_package.location.pos.0.clone()-new_location.pos.0.clone()
        , old_package.location.pos.1.clone()-new_location.pos.1.clone()
    );

    let relative_pos_in_pixels:(i32, i32) = (
        relative_pos.0.shift(new_location.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
        , relative_pos.1.shift(new_location.zoom_pot).shift(crate::actor::work_controller::PIXELS_PER_UNIT_POT).into()
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
fn sample_value(
    pixels: &Vec<CompletedPoint>
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> CompletedPoint {
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