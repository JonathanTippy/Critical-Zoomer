// read delivery.md for project context
//! Reference worker SteadyState actor (auth workgroup sub-actor).
//!
//! Receives **whole-screen** `PointStencil`s from the headgroup (never tiles).
//! Chooses the UL corner, builds on mag change, delivers bound orbit ids to the
//! tile worker. Glitch handling stays in the tile worker: fallback to the const
//! zero orbit (same per-point code; Z=0 every iteration → no glitch trigger).
//!
//! r[impl cz.ref.zero-orbit-same-path+1]

use steady_state::*;

use crate::assemblies::structs::PointStencil;
use crate::assemblies::workgroup::actor_messages::ReferenceDelivery;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::reference_worker::ReferenceWorker;
use crate::assemblies::workgroup::workcore::mandelbrot::ReferenceCollection;

pub struct ReferenceActorState {
    worker: ReferenceWorker,
    collection: ReferenceCollection,
    last_mag: Option<i32>,
}

pub async fn run(
    actor: SteadyActorShadow,
    stencil_in: SteadyRx<PointStencil>,
    to_worker: SteadyTx<ReferenceDelivery>,
    state: SteadyState<ReferenceActorState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&stencil_in], [&to_worker]),
        stencil_in,
        to_worker,
        state,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    stencil_in: SteadyRx<PointStencil>,
    to_worker: SteadyTx<ReferenceDelivery>,
    state: SteadyState<ReferenceActorState>,
) -> Result<(), Box<dyn Error>> {
    let mut stencil_in = stencil_in.lock().await;
    let mut to_worker = to_worker.lock().await;
    let mut state = state
        .lock(|| ReferenceActorState {
            worker: ReferenceWorker::empty(),
            collection: ReferenceCollection::new(),
            last_mag: None,
        })
        .await;

    // Always re-check inputs at a quick pace; fully drain + latest-wins below.
    let max_sleep = Duration::from_millis(1);

    while actor.is_running(|| i!(to_worker.mark_closed())) {
        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut stencil_in, 1),
        );

        let mut deliver = false;
        if actor.avail_units(&mut stencil_in) > 0 {
            // Latest-wins: reference considers the current screen only.
            while actor.avail_units(&mut stencil_in) > 1 {
                let _ = actor.try_take(&mut stencil_in);
            }
            if let Some(stencil) = actor.try_take(&mut stencil_in) {
                // UL corner of the screen = stencil homothety location (auth).
                let c = (stencil.homothety.0.clone(), stencil.homothety.1.clone());
                let mag = stencil.homothety.2;
                if state.last_mag.is_none() {
                    state.worker =
                        ReferenceWorker::seed_into(&mut state.collection, c, mag);
                    state.last_mag = Some(mag);
                    deliver = true;
                } else if state.last_mag != Some(mag) {
                    // Mag change only; pan is insignificant (auth).
                    state.worker.notify_mag_change(c, mag);
                    state.last_mag = Some(mag);
                    let changed = {
                        let ReferenceActorState {
                            worker,
                            collection,
                            ..
                        } = &mut *state;
                        worker.poll(collection)
                    };
                    if changed {
                        deliver = true;
                    }
                }
            }
        }

        // Keep polling pending construction even without new stencils.
        let pending_changed = {
            let ReferenceActorState {
                worker,
                collection,
                ..
            } = &mut *state;
            worker.has_pending() && worker.poll(collection)
        };
        if pending_changed {
            deliver = true;
        }

        if deliver {
            if let Some(mag) = state.worker.bound_mag() {
                let _ = actor.try_send(
                    &mut to_worker,
                    ReferenceDelivery {
                        bound_orbit_id: state.worker.bound_orbit_id(),
                        bound_mag: mag,
                    },
                );
            }
        }
    }
    info!("Reference worker shutting down.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::workgroup::workcore::mandelbrot::{ReferenceOrbit, ZERO_ORBIT_ID};
    use crate::intexp::IntExp;

    // r[verify cz.ref.zero-orbit-same-path+1]
    #[test]
    fn zero_orbit_is_const_z_zero() {
        let z = ReferenceOrbit::zero();
        assert_eq!(z.period, 1);
        assert_eq!(z.f64.big_z_orbit, vec![(0.0, 0.0)]);
        assert_eq!(z.big_c, (IntExp::ZERO, IntExp::ZERO));
        assert_eq!(ZERO_ORBIT_ID, 0);
    }

    // r[verify cz.ref.zero-orbit-same-path+1]
    #[test]
    fn seed_uses_stencil_ul_not_tile_origin() {
        // Period-2 nucleus at UL so seed succeeds (non-nucleus → zero orbit).
        let mut collection = ReferenceCollection::new();
        let c = (IntExp::from(-1), IntExp::from(0));
        let worker = ReferenceWorker::seed_into(&mut collection, c.clone(), -2);
        assert_ne!(worker.bound_orbit_id(), ZERO_ORBIT_ID);
        let orbit = collection.get(worker.bound_orbit_id()).expect("seeded");
        assert_eq!(orbit.big_c.0, c.0);
        assert_eq!(orbit.big_c.1, c.1);
    }
}
