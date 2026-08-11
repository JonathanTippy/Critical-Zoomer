use steady_state::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::c_generator::admit_generator;
use crate::assemblies::workgroup::screen_worker::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::constants::*;
use crate::utils::*;

/// Wakes of the work controller since last drain into a `WorkUpdate`.
pub static CONTROLLER_WAKE_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn take_controller_frames_delta() -> u64 {
    CONTROLLER_WAKE_COUNT.swap(0, Ordering::Relaxed)
}

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
        CONTROLLER_WAKE_COUNT.fetch_add(1, Ordering::Relaxed);

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
                delta_c: point.clone(),
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
                bound_zero_generation: 0,
            })
        }
    }
    out
}

/// Stencil admission gate: unchanged views are suppressed; views whose f64
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
    let view_center = view_center_compute(&compute_loc, obj.zoom_pot, res);
    if admit_generator::<f64>(
        &compute_loc,
        obj.zoom_pot as i64,
        res,
        None,
        &view_center,
    )
    .is_none()
    {
        return false;
    }

    state.worker_res = res;
    state.last_sampler_location = Some(obj.clone());
    true
}

#[cfg(test)]
mod mutant_kill {
    use super::*;
    use crate::constants::TEST_SCREEN_RES;

    fn frame(zoom: i32) -> (ObjectivePosAndZoom, (u32, u32)) {
        (
            ObjectivePosAndZoom {
                pos: (
                    IntExp::from(HOME_POSITION.0),
                    IntExp::from(HOME_POSITION.1),
                ),
                zoom_pot: zoom,
            },
            TEST_SCREEN_RES,
        )
    }

    /// Thought-killed pins for stencil-only replace gate (unchanged suppress, collapse fail-closed).
    #[test]
    fn mutant_kill_should_send_replace_gate() {
        let mut state = WorkControllerState {
            worker_res: TEST_SCREEN_RES,
            last_sampler_location: None,
        };
        let f0 = frame(-2);
        assert!(should_send_replace(&mut state, &f0));
        // Identical stencil must not re-send (kill `!=`→`==` / `||`→`&&` flips).
        assert!(!should_send_replace(&mut state, &f0));
        let mut f1 = f0.clone();
        f1.0.zoom_pot = -1;
        assert!(should_send_replace(&mut state, &f1));
        assert!(!should_send_replace(&mut state, &f1));

        // Resolution change alone must re-admit (not location-only compare).
        let mut fres = f1.clone();
        fres.1 = (TEST_SCREEN_RES.0 + 2, TEST_SCREEN_RES.1);
        assert!(should_send_replace(&mut state, &fres));
        assert_eq!(state.worker_res, fres.1);
    }

    /// Screen→plane: +seat → +real, +row → −imag, zoom_pot polarity, init zoom.
    #[test]
    fn mutant_kill_get_points_axes_and_zoom_pot() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let pts = get_points::<f64>((4, 3), loc.clone(), 0);
        let i = |seat: u32, row: u32| (row * 4 + seat) as usize;

        // +seat increases real; same row keeps imag.
        assert!(pts[i(1, 0)].c.0 > pts[i(0, 0)].c.0);
        assert_eq!(pts[i(1, 0)].c.1, pts[i(0, 0)].c.1);

        // +row decreases imag (screen y-down → plane +imag up); same seat keeps real.
        assert!(pts[i(0, 1)].c.1 < pts[i(0, 0)].c.1);
        assert_eq!(pts[i(0, 1)].c.0, pts[i(0, 0)].c.0);

        // At zoom 0, pitch = 1/PIXELS_PER_UNIT (zoom_factor = 1).
        let space = 1.0 / (PIXELS_PER_UNIT as f64);
        assert!((pts[i(1, 0)].c.0 - pts[i(0, 0)].c.0 - space).abs() < 1e-12);
        assert!((pts[i(0, 0)].c.1 - pts[i(0, 1)].c.1 - space).abs() < 1e-12);

        // zoom>0 shrinks pitch (>>); zoom<0 expands (<<).
        let wide = get_points::<f64>((2, 1), loc.clone(), -2);
        let deep = get_points::<f64>((2, 1), loc, 2);
        let pitch = |p: &[Point<f64>]| (p[1].c.0 - p[0].c.0).abs();
        assert!(pitch(&wide) > pitch(&deep));
        assert!((pitch(&wide) / pitch(&deep) - 16.0).abs() < 1e-9); // 2^2 / 2^-2 = 16

        assert_eq!(WORKER_INIT_ZOOM_POT, -2);
        assert_eq!(WORKER_INIT_ZOOM, 0.25);
        assert_ne!(WORKER_INIT_ZOOM, 4.0);
        assert_ne!(WORKER_INIT_ZOOM, -0.25);
    }
}
