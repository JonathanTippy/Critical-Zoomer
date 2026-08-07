use steady_state::*;

use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::c_generator::CGenerator;
use crate::assemblies::workgroup::screen_worker::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::constants::*;
use crate::utils::*;

pub enum WorkerCommand {
    Replace { frame_info: (ObjectivePosAndZoom, (u32, u32)) },
}

pub struct WorkControllerState {
    worker_res: (u32, u32),
    last_sampler_location: Option<ObjectivePosAndZoom>,
}

pub const WORKER_INIT_RES: (u32, u32) = DEFAULT_WINDOW_RES;
pub const WORKER_INIT_ZOOM_POT: i64 = -2;
pub const WORKER_INIT_ZOOM: f64 = if WORKER_INIT_ZOOM_POT > 0 {
    (1 << WORKER_INIT_ZOOM_POT) as f64
} else {
    1.0 / (1 << -WORKER_INIT_ZOOM_POT) as f64
};

pub const PIXELS_PER_UNIT: u64 = 1 << (PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(PointStencil)>,
    to_worker: SteadyTx<WorkerCommand>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
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

    let mut state = state
        .lock(|| WorkControllerState {
            worker_res: WORKER_INIT_RES,
            last_sampler_location: None,
        })
        .await;

    let max_sleep = Duration::from_millis(50);

    while actor.is_running(|| i!(to_worker.mark_closed())) {
        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );

        if actor.avail_units(&mut from_sampler) > 0 {
            // r[impl cz.craft.drain-to-newest+1]
            while actor.avail_units(&mut from_sampler) > 1 {
                let stuff = actor.try_take(&mut from_sampler).expect("internal error");
                drop(stuff);
            }

            let stuff = actor.try_take(&mut from_sampler).expect("internal error");

            let frame_info = (
                ObjectivePosAndZoom {
                    pos: (
                        stuff.location.0.clone(),
                        IntExp::ZERO - stuff.location.1.clone(),
                    ),
                    zoom_pot: stuff.location.2,
                },
                (stuff.resolution.0 as u32, stuff.resolution.1 as u32),
            );

            // r[impl cz.craft.stencil-only-replace+2]
            if should_send_replace(&mut state, &frame_info) {
                actor.try_send(
                    &mut to_worker,
                    WorkerCommand::Replace { frame_info },
                );
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

use std::ops::*;

/// Oracle grid matching v0.0.9 get_points — kept for CGenerator parity tests only.
pub fn get_points<
    T: From<f32>
        + Clone
        + From<IntExp>
        + Sub<Output = T>
        + Add<Output = T>
        + Mul<Output = T>
        + PartialOrd
        + crate::assemblies::workgroup::screen_worker::workshift::Finite
        + crate::assemblies::workgroup::screen_worker::workshift::Gt
        + crate::assemblies::workgroup::screen_worker::workshift::Abs
        + From<f32>
        + Into<f64>
        + Copy,
>(
    res: (u32, u32),
    loc: (IntExp, IntExp),
    zoom: i64,
) -> Vec<Point<T>> {
    let mut out: Vec<Point<T>> = Vec::with_capacity((res.0 * res.1) as usize);

    let significant_res = PIXELS_PER_UNIT;

    let real_center: T = loc.0.into();
    let imag_center: T = loc.1.into();

    let zoom_factor: IntExp;

    if zoom > 0 {
        zoom_factor = IntExp::from(1) >> (zoom as u32);
    } else {
        zoom_factor = IntExp::from(1) << ((-zoom) as u32);
    }

    for row in 0..res.1 {
        for seat in 0..res.0 {
            let row = row as f32;
            let seat = seat as f32;

            let point: (T, T) = (
                real_center + (T::from((seat / significant_res as f32)) * zoom_factor.clone().into()),
                imag_center
                    + (T::from(-((row / significant_res as f32))) * zoom_factor.clone().into()),
            );

            out.push(Point {
                c: point.clone(),
                z: point.clone(),
                dc: (1.0.into(), 0.0.into()),
                real_squared: 0.0.into(),
                imag_squared: 0.0.into(),
                real_imag: 0.0.into(),
                iterations: 0,
                loop_detection_point: ((0.0.into(), 0.0.into()), 0),
                escapes: false,
                repeats: false,
                delivered: false,
                initialized: true,
                period: 0,
                smallness_squared: 100.0.into(),
                small_time: 0,
                delta: None,
                direct_only: false,
            })
        }
    }
    out
}

/// Fail-closed stencil gate: unchanged views are suppressed; views whose f64
/// grid would collapse are suppressed. The worker builds the world from the
/// stencil alone.
fn should_send_replace(
    state: &mut WorkControllerState,
    frame_info: &(ObjectivePosAndZoom, (u32, u32)),
) -> bool {
    let obj = &frame_info.0;
    let res = frame_info.1;

    if let Some(loc) = &state.last_sampler_location {
        if !((*obj != *loc) || res != state.worker_res) {
            return false;
        }
    }

    // Compute-grid loc matches get_points / CGenerator: frame_info imag is
    // already display-flipped once; flip again for the arithmetic origin.
    let compute_loc = (obj.pos.0.clone(), IntExp::ZERO - obj.pos.1.clone());
    if CGenerator::<f64>::new(&compute_loc, obj.zoom_pot as i64, res).is_none() {
        return false;
    }

    state.worker_res = res;
    state.last_sampler_location = Some(obj.clone());
    true
}
