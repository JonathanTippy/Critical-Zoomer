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

pub(crate) enum ScreenValue {
    Outside{escape_time: u32}
    , Inside{loop_period: u32}
}

pub(crate) struct ResultsPackage {
    pub(crate) results: Vec<ScreenValue>
    , pub(crate) screen_res: (u32, u32)
    , pub(crate) location: ObjectivePosAndZoom
    , pub(crate) complete: bool
}

pub(crate) struct WorkCollectorState {
    completed_work: Option<ResultsPackage>
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
    values_out: SteadyTx<ResultsPackage>,
    state: SteadyState<WorkCollectorState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_worker], [&values_out]),
        from_worker,
        values_out,
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
        completed_work: None
        , worker_res: (u32, u32)
        , percent_completed:u16
    }).await;

    let max_sleep = Duration::from_millis(50);

    let res = state.worker_res.clone();
    //let ctx = handle_sampler_stuff(&mut state, (
    //    (IntExp::from(0), IntExp::from(0)), res));
    //actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {
        let percent_completed = 0;

        if let Some(w) = state.completed_work {

        }


        if actor.avail_units(&mut from_worker) > 0 {
            let mut u = actor.try_take(&mut from_worker).unwrap();

            if let Some(f) = u.frame_info {


                state.completed_work = vec!();
                state.last_sampler_location = Some(f);
            }

            state.completed_work.append(&mut u.completed_points);
            let res = state.worker_res;
            let c = state.percent_completed==u16::MAX;
            let r = determine_arvs_dummy(&state.completed_work, res);
            info!("got work update. results length is now {}", r.len());
            if r.len() == (res.0*res.1) as usize {
                actor.try_send(&mut values_out, ResultsPackage{
                    results:r
                    ,screen_res:res
                    ,location: state.last_sampler_location.clone().expect("WC recieved work from worker but somehow don't have a sampler location")
                    ,complete:c
                });
            }
        }

    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}



fn determine_arvs_dummy(points: &Vec<CompletedPoint>, res: (u32, u32)) -> Vec<ScreenValue> {
    let mut returned = vec!();
    for p in points {
        returned.push(
            match p {
                CompletedPoint::Escapes{escape_time: t, escape_location: _} => {
                    ScreenValue::Outside{escape_time:*t}
                }
                CompletedPoint::Repeats{period: p} => {
                    ScreenValue::Inside{loop_period:*p}
                }
                CompletedPoint::Dummy{} => {
                    ScreenValue::Outside{escape_time:2}
                }
            }
        )
    }

    returned
}

fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}

fn overlay_results(
    old_results: &mut Vec<CompletedPoint>
    , old_results_obj: ObjectivePosAndZoom
    , new_results: &mut Vec<CompletedPoint>
    , new_results_obj: ObjectivePosAndZoom
) {
    for row in 0..size.1 as usize {
        for seat in 0..size.0 as usize {
            bucket.push(
                crate::action::sampling::sample_color(
                    data
                    , min_side
                    , data_size
                    , data_len
                    , row
                    , seat
                    //, res_recip
                    , min_side_recip
                    , relative_pos_in_pixels
                    , relative_zoom as i64
                )
            );
            //i+=1;
        }
    }
}


#[inline]
fn sample_value(
    pixels: &Vec<CompletedPoint>
    , min_side: u32
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    //, res_recip: (u32, u32)
    , min_side_recip: i64
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