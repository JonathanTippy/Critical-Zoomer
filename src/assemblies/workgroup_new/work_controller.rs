use steady_state::*;

use crate::assemblies::headgroup::window::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::structs::PointStencil;
use crate::assemblies::workgroup_new::screen_worker::*;
use crate::utils::*;
use crate::intexp::*;
use crate::constants::*;

pub enum WorkerCommand {
    /// Retarget the live TileSession. Legacy WorkContext was removed after the
    /// tile cutover; only frame_info drives the worker.
    Replace { frame_info: (ObjectivePosAndZoom, (u32, u32)) }
}


pub struct WorkControllerState {
    worker_res: (u32, u32)
    , last_sampler_location: Option<ObjectivePosAndZoom>
}


pub const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub const WORKER_INIT_ZOOM_POT: i64 = -2;
pub const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(PointStencil)>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&from_sampler], [&to_worker]),
        from_sampler,
        to_worker,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_sampler: SteadyRx<(PointStencil)>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut from_sampler = from_sampler.lock().await;
    let mut to_worker = to_worker.lock().await;

    let mut state = state.lock(|| WorkControllerState {
        worker_res: WORKER_INIT_RES
        , last_sampler_location: None
    }).await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(
        || i!(to_worker.mark_closed())
    ) {
        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );

        if actor.avail_units(&mut from_sampler) > 0 {
            while actor.avail_units(&mut from_sampler) > 1 {
                let stuff = actor.try_take(&mut from_sampler).expect("internal error");
                drop(stuff);
            };

            let stuff = actor.try_take(&mut from_sampler).expect("internal error");

            let frame_info = (
                ObjectivePosAndZoom {
                    pos: (stuff.homothety.0.clone(), IntExp::ZERO - stuff.homothety.1.clone())
                    , zoom_pot: stuff.homothety.2
                }
                , (stuff.resolution.0 as u32, stuff.resolution.1 as u32)
            );
            if should_retarget(&mut state, &frame_info) {
                actor.try_send(&mut to_worker, WorkerCommand::Replace { frame_info });
            }
        }
    }
    info!("Computer shutting down.");
    Ok(())
}

fn should_retarget(
    state: &mut WorkControllerState
    , frame_info: &(ObjectivePosAndZoom, (u32, u32))
) -> bool {
    let obj = &frame_info.0;
    let res = frame_info.1;
    if let Some(loc) = state.last_sampler_location.as_ref() {
        if loc == obj && res == state.worker_res {
            return false;
        }
    }
    state.worker_res = res;
    state.last_sampler_location = Some(obj.clone());
    true
}

#[cfg(test)]
mod retarget_tests {
    use super::*;

    fn frame(zoom: i32, res: (u32, u32)) -> (ObjectivePosAndZoom, (u32, u32)) {
        (
            ObjectivePosAndZoom {
                pos: (IntExp::from(0), IntExp::from(0))
                , zoom_pot: zoom
            }
            , res
        )
    }

    #[test]
    fn first_frame_retargets() {
        let mut state = WorkControllerState {
            worker_res: WORKER_INIT_RES
            , last_sampler_location: None
        };
        assert!(should_retarget(&mut state, &frame(-2, WORKER_INIT_RES)));
    }

    #[test]
    fn identical_frame_skips() {
        let mut state = WorkControllerState {
            worker_res: WORKER_INIT_RES
            , last_sampler_location: None
        };
        let f = frame(-2, WORKER_INIT_RES);
        assert!(should_retarget(&mut state, &f));
        assert!(!should_retarget(&mut state, &f));
    }

    #[test]
    fn zoom_change_retargets() {
        let mut state = WorkControllerState {
            worker_res: WORKER_INIT_RES
            , last_sampler_location: None
        };
        assert!(should_retarget(&mut state, &frame(-2, WORKER_INIT_RES)));
        assert!(should_retarget(&mut state, &frame(0, WORKER_INIT_RES)));
    }
}
