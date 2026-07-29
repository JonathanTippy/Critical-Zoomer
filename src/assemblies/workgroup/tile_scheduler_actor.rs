//! Tile scheduler SteadyState actor (auth workgroup sub-actor).
//! Absorbs former work_controller coalesce / should_retarget, plus attention.
//! Does not feed the reference worker — that receives whole-screen stencils
//! directly from the headgroup (architecture actor-wiring addendum).

use steady_state::*;

use crate::assemblies::headgroup::window::inputs::mag_velocity_mode;
use crate::assemblies::structs::PointStencil;
use crate::assemblies::workgroup::actor_messages::SchedulerToWorker;
use crate::constants::*;
use crate::intexp::*;
use crate::utils::*;

pub struct TileSchedulerActorState {
    worker_res: (u32, u32),
    last_sampler_location: Option<ObjectivePosAndZoom>,
    last_mag_mode: i32,
}

pub const WORKER_INIT_RES: (u32, u32) = DEFAULT_WINDOW_RES;

pub async fn run(
    actor: SteadyActorShadow,
    stencil_in: SteadyRx<PointStencil>,
    attention_in: SteadyRx<(i32, i32)>,
    to_worker: SteadyTx<SchedulerToWorker>,
    state: SteadyState<TileSchedulerActorState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&stencil_in, &attention_in], [&to_worker]),
        stencil_in,
        attention_in,
        to_worker,
        state,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    stencil_in: SteadyRx<PointStencil>,
    attention_in: SteadyRx<(i32, i32)>,
    to_worker: SteadyTx<SchedulerToWorker>,
    state: SteadyState<TileSchedulerActorState>,
) -> Result<(), Box<dyn Error>> {
    let mut stencil_in = stencil_in.lock().await;
    let mut attention_in = attention_in.lock().await;
    let mut to_worker = to_worker.lock().await;

    let mut state = state
        .lock(|| TileSchedulerActorState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
            last_mag_mode: 0,
        })
        .await;

    // Always re-check inputs at a quick pace; fully drain + latest-wins below.
    let max_sleep = Duration::from_millis(1);

    while actor.is_running(|| i!(to_worker.mark_closed())) {
        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut stencil_in, 1),
            actor.wait_avail(&mut attention_in, 1),
        );

        while actor.avail_units(&mut attention_in) > 0 {
            while actor.avail_units(&mut attention_in) > 1 {
                let _ = actor.try_take(&mut attention_in);
            }
            if let Some((x, y)) = actor.try_take(&mut attention_in) {
                let _ = actor.try_send(&mut to_worker, SchedulerToWorker::SetAttention(x, y));
            }
        }

        if actor.avail_units(&mut stencil_in) > 0 {
            while actor.avail_units(&mut stencil_in) > 1 {
                let _ = actor.try_take(&mut stencil_in);
            }

            let Some(stuff) = actor.try_take(&mut stencil_in) else {
                continue;
            };

            let frame_info = (
                ObjectivePosAndZoom {
                    pos: (
                        stuff.homothety.0.clone(),
                        IntExp::ZERO - stuff.homothety.1.clone(),
                    ),
                    zoom_pot: stuff.homothety.2,
                },
                (stuff.resolution.0 as u32, stuff.resolution.1 as u32),
                stuff.mag_velocity,
            );

            if should_retarget(&mut state, &frame_info) {
                let _ = actor.try_send(
                    &mut to_worker,
                    SchedulerToWorker::Retarget { frame_info },
                );
            }
        }
    }
    info!("Tile scheduler shutting down.");
    Ok(())
}

fn should_retarget(
    state: &mut TileSchedulerActorState,
    frame_info: &(ObjectivePosAndZoom, (u32, u32), f64),
) -> bool {
    let obj = &frame_info.0;
    let res = frame_info.1;
    let mode = mag_velocity_mode(frame_info.2);
    let loc_same =
        state.last_sampler_location.as_ref() == Some(obj) && res == state.worker_res;
    let mode_same = mode == state.last_mag_mode;
    if loc_same && mode_same {
        return false;
    }
    state.worker_res = res;
    state.last_sampler_location = Some(obj.clone());
    state.last_mag_mode = mode;
    true
}

#[cfg(test)]
mod retarget_tests {
    use super::*;

    fn frame(zoom: i32, res: (u32, u32), mag_vel: f64) -> (ObjectivePosAndZoom, (u32, u32), f64) {
        (
            ObjectivePosAndZoom {
                pos: (IntExp::from(0), IntExp::from(0)),
                zoom_pot: zoom,
            },
            res,
            mag_vel,
        )
    }

    #[test]
    fn first_frame_retargets() {
        let mut state = TileSchedulerActorState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
            last_mag_mode: 0,
        };
        assert!(should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 0.0)
        ));
    }

    #[test]
    fn identical_frame_skips() {
        let mut state = TileSchedulerActorState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
            last_mag_mode: 0,
        };
        let f = frame(-2, WORKER_INIT_RES, 0.0);
        assert!(should_retarget(&mut state, &f));
        assert!(!should_retarget(&mut state, &f));
    }

    #[test]
    fn zoom_change_retargets() {
        let mut state = TileSchedulerActorState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
            last_mag_mode: 0,
        };
        assert!(should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 0.0)
        ));
        assert!(should_retarget(
            &mut state,
            &frame(0, WORKER_INIT_RES, 0.0)
        ));
    }

    #[test]
    fn mag_velocity_mode_change_retargets_same_location() {
        let mut state = TileSchedulerActorState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
            last_mag_mode: 0,
        };
        assert!(should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 0.0)
        ));
        assert!(should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 5.0)
        ));
        assert!(!should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 4.0)
        ));
        assert!(should_retarget(
            &mut state,
            &frame(-2, WORKER_INIT_RES, 0.0)
        ));
    }
}
