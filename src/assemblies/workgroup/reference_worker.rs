//! Background owner of the single arbitrary-precision reference-orbit job.
//!
//! Requests coalesce to newest; work is sliced into wall-clock bouts; completed
//! references move across the channel as whole immutable snapshots.

use std::error::Error;
use std::time::Duration;

use steady_state::*;

use crate::assemblies::workgroup::screen_worker::workshift::{WorkContext, MAX_BOUT};
use crate::assemblies::workgroup::c_generator::{CGenerator, Mandelbrotable};
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::reference::ReferenceOrbit;
use crate::utils::{IntExp, ObjectivePosAndZoom};

#[derive(Clone)]
pub struct ReferenceRequest {
    pub c: (IntExp, IntExp),
    pub precision_bits: u32,
    /// Requested orbit length, not an application effort cap. A later request
    /// for the same c may extend the target.
    pub max_iterations: u32,
}

pub struct PublishedReference {
    pub orbit: ReferenceOrbit,
    pub c: (IntExp, IntExp),
    pub generation: u64,
}

impl std::fmt::Debug for PublishedReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishedReference")
            .field("generation", &self.generation)
            .field("c", &self.c)
            .field("orbit_len", &self.orbit.iterates.len())
            .field("period", &self.orbit.period)
            .finish()
    }
}

struct ReferenceJob {
    request: ReferenceRequest,
    orbit: ReferenceOrbit,
}

pub struct ReferenceWorkerState {
    job: Option<ReferenceJob>,
    generation: u64,
}

impl ReferenceWorkerState {
    pub fn new() -> Self {
        Self {
            job: None,
            generation: 0,
        }
    }

    // r[impl cz.depth.reference-latest-wins+1]
    pub fn replace(&mut self, request: ReferenceRequest) {
        let orbit = ReferenceOrbit::start(&request.c, request.precision_bits);
        self.job = Some(ReferenceJob { request, orbit });
    }

    /// Runs one bounded bout and returns a whole snapshot only when this
    /// request's target length (or an honest terminal state) is reached.
    // r[impl cz.depth.reference-bout-law+1]
    // r[impl cz.depth.reference-whole-snapshot+1]
    pub fn work_for(&mut self, budget: Duration) -> Option<PublishedReference> {
        let job = self.job.as_mut()?;
        let completed = job.orbit.iterates.len().saturating_sub(1) as u32;
        let remaining = job.request.max_iterations.saturating_sub(completed);
        if remaining > 0 && job.orbit.period.is_none() && !job.orbit.escaped {
            job.orbit.extend_for(remaining, budget);
        }

        let completed = job.orbit.iterates.len().saturating_sub(1) as u32;
        let done = completed >= job.request.max_iterations
            || job.orbit.period.is_some()
            || job.orbit.escaped;
        if !done {
            return None;
        }

        let job = self.job.take().expect("completed job exists");
        self.generation = self.generation.wrapping_add(1);
        Some(PublishedReference {
            orbit: job.orbit,
            c: job.request.c,
            generation: self.generation,
        })
    }
}

impl Default for ReferenceWorkerState {
    fn default() -> Self {
        Self::new()
    }
}

fn f64_to_intexp(value: f64) -> IntExp {
    assert!(value.is_finite());
    if value == 0.0 {
        return IntExp::ZERO;
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (significand, exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        ((1u64 << 52) | fraction, biased - 1023 - 52)
    };
    let mut val = rug::Integer::from(significand);
    if negative {
        val = -val;
    }
    IntExp { val, exp: exponent }
}

fn objective_c(
    frame: &(ObjectivePosAndZoom, (u32, u32)),
    seat: u32,
    row: u32,
) -> (IntExp, IntExp) {
    let compute_loc = (
        frame.0.pos.0.clone(),
        IntExp::ZERO - frame.0.pos.1.clone(),
    );
    // Bit-identical to `CGenerator::get_c` on f64-valid grids (the live path).
    if let Some(generator) = CGenerator::<f64>::new(
        &compute_loc,
        frame.0.zoom_pot as i64,
        frame.1,
    ) {
        let (re, im) = generator.get_c((seat, row));
        return (f64_to_intexp(re), f64_to_intexp(im));
    }
    let exponent = frame.0.zoom_pot.saturating_add(PIXELS_PER_UNIT_POT);
    let pitch = IntExp::from(1).shift(exponent.saturating_neg());
    (
        compute_loc.0.clone() + pitch.clone() * IntExp::from(seat as usize),
        compute_loc.1.clone() - pitch * IntExp::from(row as usize),
    )
}

/// Choose once at pivot: deepest delivered interior seat from the previous
/// live view, otherwise the new view's center. Progress within a view never
/// calls this function, so selection is sticky.
// r[impl cz.depth.reference-sticky-selection+1]
pub fn select_reference_request<T: Mandelbrotable>(
    previous: Option<(&WorkContext<T>, &(ObjectivePosAndZoom, (u32, u32)))>,
    new_frame: &(ObjectivePosAndZoom, (u32, u32)),
) -> ReferenceRequest {
    let selected = previous.and_then(|(context, frame)| {
        context
            .points
            .iter()
            .enumerate()
            .filter(|(_, point)| point.delivered && point.repeats && !point.escapes)
            .max_by_key(|(_, point)| point.iterations)
            .map(|(index, point)| {
                let seat = index as u32 % context.res.0;
                let row = index as u32 / context.res.0;
                (
                    objective_c(frame, seat, row),
                    point.iterations.max(MAX_BOUT),
                )
            })
    });

    let (c, max_iterations) = selected.unwrap_or_else(|| {
        let center = (new_frame.1.0 / 2, new_frame.1.1 / 2);
        (objective_c(new_frame, center.0, center.1), MAX_BOUT)
    });
    ReferenceRequest {
        c,
        precision_bits: crate::reference::bits_for_zoom(
            new_frame.0.zoom_pot as i64,
            PIXELS_PER_UNIT_POT,
        ),
        max_iterations,
    }
}

pub async fn run(
    actor: SteadyActorShadow,
    requests_in: SteadyRx<ReferenceRequest>,
    references_out: SteadyTx<PublishedReference>,
    state: SteadyState<ReferenceWorkerState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&requests_in], [&references_out]),
        requests_in,
        references_out,
        state,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    requests_in: SteadyRx<ReferenceRequest>,
    references_out: SteadyTx<PublishedReference>,
    state: SteadyState<ReferenceWorkerState>,
) -> Result<(), Box<dyn Error>> {
    let mut requests_in = requests_in.lock().await;
    let mut references_out = references_out.lock().await;
    let mut state = state.lock(ReferenceWorkerState::new).await;

    while actor.is_running(|| i!(references_out.mark_closed())) {
        await_for_any!(
            actor.wait_periodic(Duration::from_millis(10)),
            actor.wait_avail(&mut requests_in, 1),
        );

        if actor.avail_units(&mut requests_in) > 0 {
            // r[impl cz.depth.reference-latest-wins+1]
            while actor.avail_units(&mut requests_in) > 1 {
                drop(actor.try_take(&mut requests_in).expect("reference request"));
            }
            state.replace(
                actor
                    .try_take(&mut requests_in)
                    .expect("newest reference request"),
            );
        }

        if let Some(reference) = state.work_for(Duration::from_millis(10)) {
            actor.try_send(&mut references_out, reference);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::utils::ObjectivePosAndZoom;

    fn request(c: i32, iterations: u32) -> ReferenceRequest {
        ReferenceRequest {
            c: (IntExp::from(c), IntExp::ZERO),
            precision_bits: 128,
            max_iterations: iterations,
        }
    }

    #[test]
    // r[verify cz.depth.reference-latest-wins+1]
    fn newer_request_replaces_in_progress_job() {
        let mut state = ReferenceWorkerState::new();
        state.replace(request(0, 100));
        assert!(state.work_for(Duration::ZERO).is_none());
        state.replace(request(-1, 8));
        let published = state.work_for(Duration::from_secs(1)).unwrap();
        assert_eq!(published.c.0, IntExp::from(-1));
        assert_eq!(published.generation, 1);
    }

    #[test]
    // r[verify cz.depth.reference-whole-snapshot+1]
    fn publication_moves_one_complete_snapshot_and_increments_generation() {
        let mut state = ReferenceWorkerState::new();
        state.replace(request(2, 20));
        let first = state.work_for(Duration::from_secs(1)).unwrap();
        assert!(first.orbit.escaped);
        assert!(state.work_for(Duration::from_secs(1)).is_none());

        state.replace(request(-1, 8));
        let second = state.work_for(Duration::from_secs(1)).unwrap();
        assert_eq!(second.generation, first.generation + 1);
        assert!(second.orbit.period.is_some());
    }

    fn frame() -> (ObjectivePosAndZoom, (u32, u32)) {
        (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(1)),
                zoom_pot: -2,
            },
            (4, 2),
        )
    }

    #[test]
    fn objective_c_matches_c_generator_on_grid() {
        let f = frame();
        let ctx = from_stencil::<f64>(f.clone(), None).unwrap();
        for row in 0..f.1.1 {
            for seat in 0..f.1.0 {
                let oc = objective_c(&f, seat, row);
                let gc = ctx.c_generator.get_c((seat, row));
                let oc_f = (f64::from(oc.0), f64::from(oc.1));
                assert_eq!(oc_f, gc, "seat {seat} row {row}");
            }
        }
    }

    #[test]
    // r[verify cz.depth.reference-sticky-selection+1]
    fn selection_uses_deepest_completed_interior_then_center_fallback() {
        let f = frame();
        let mut context = from_stencil::<f64>(f.clone(), None).unwrap();
        context.points[1].delivered = true;
        context.points[1].repeats = true;
        context.points[1].iterations = 12;
        context.points[7].delivered = true;
        context.points[7].repeats = true;
        context.points[7].iterations = 80;

        let selected = select_reference_request(Some((&context, &f)), &f);
        assert_eq!(selected.c, objective_c(&f, 3, 1));

        let fallback = select_reference_request::<f64>(None, &f);
        assert_eq!(fallback.c, objective_c(&f, 2, 1));
    }

    #[test]
    fn precision_is_chosen_once_from_new_view_depth() {
        let mut f = frame();
        f.0.zoom_pot = 1500;
        let selected = select_reference_request::<f64>(None, &f);
        assert_eq!(
            selected.precision_bits,
            crate::reference::bits_for_zoom(1500, PIXELS_PER_UNIT_POT)
        );
    }
}
