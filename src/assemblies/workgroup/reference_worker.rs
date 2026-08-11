//! Background owner of the single arbitrary-precision reference-orbit job.
//!
//! Requests coalesce to newest; work is sliced into wall-clock bouts; completed
//! references move across the channel as whole immutable snapshots.

use std::error::Error;
use std::time::Duration;

use steady_state::*;

use crate::assemblies::workgroup::screen_worker::workshift::{view_center_compute, WorkContext};
use crate::assemblies::workgroup::c_generator::{admit_generator, GeneratorAdmission, Mandelbrotable};
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::reference::ReferenceOrbit;
use crate::utils::{IntExp, ObjectivePosAndZoom};

#[derive(Clone)]
pub struct ReferenceRequest {
    pub c: (IntExp, IntExp),
    pub precision_bits: u32,
}

pub struct PublishedReference {
    pub orbit: ReferenceOrbit,
    /// reference_c — exact objective parameter for this orbit.
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

    /// Runs one wall-clock bout. Publishes only when the orbit has an honest
    /// terminal state (period found or escaped) — never an artificial length
    /// wall. Incomplete interiors keep the zero-orbit floor until then.
    // r[impl cz.depth.reference-bout-law+1]
    // r[impl cz.depth.reference-whole-snapshot+1]
    // r[impl cz.depth.reference-until-done+1]
    pub fn work_for(&mut self, budget: Duration) -> Option<PublishedReference> {
        let job = self.job.as_mut()?;
        if job.orbit.period.is_none() && !job.orbit.escaped {
            // Same interruptibility as seats: wall-clock bout, no length cap.
            // `u32::MAX` here is only "keep going within budget," not a target.
            job.orbit.extend_for(u32::MAX, budget);
        }

        let done = job.orbit.period.is_some() || job.orbit.escaped;
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
    let view_center = view_center_compute(&compute_loc, frame.0.zoom_pot, frame.1);
    if let Some(admission) = admit_generator::<f64>(
        &compute_loc,
        frame.0.zoom_pot as i64,
        frame.1,
        None,
        &view_center,
    ) {
        let (re, im) = admission.generator().get_c((seat, row));
        return match admission {
            GeneratorAdmission::Absolute(_) => (f64_to_intexp(re), f64_to_intexp(im)),
            GeneratorAdmission::Relative { anchor, .. } => (
                anchor.0.clone() + f64_to_intexp(re),
                anchor.1.clone() + f64_to_intexp(im),
            ),
        };
    }
    let exponent = frame.0.zoom_pot.saturating_add(PIXELS_PER_UNIT_POT);
    let pitch = IntExp::from(1).shift(exponent.saturating_neg());
    (
        compute_loc.0.clone() + pitch.clone() * IntExp::from(seat as usize),
        compute_loc.1.clone() - pitch * IntExp::from(row as usize),
    )
}

/// True when `c` lies inside the viewport rectangle of `frame` (inclusive).
/// Coverage is diagnostic / fixture helper — carry/reuse no longer drops refs
/// merely because they left the view (greedy keep; see reference-reuse paraphrase).
// r[impl cz.depth.reference-coverage+1]
pub fn reference_c_covers_frame(
    c: &(IntExp, IntExp),
    frame: &(ObjectivePosAndZoom, (u32, u32)),
) -> bool {
    let w = frame.1.0.max(1);
    let h = frame.1.1.max(1);
    let a = objective_c(frame, 0, 0);
    let b = objective_c(frame, w - 1, h - 1);
    let re_lo = if a.0.clone() < b.0.clone() { a.0.clone() } else { b.0.clone() };
    let re_hi = if a.0.clone() < b.0.clone() { b.0.clone() } else { a.0.clone() };
    let im_lo = if a.1.clone() < b.1.clone() { a.1.clone() } else { b.1.clone() };
    let im_hi = if a.1.clone() < b.1.clone() { b.1.clone() } else { a.1.clone() };
    c.0.clone() >= re_lo
        && c.0.clone() <= re_hi
        && c.1.clone() >= im_lo
        && c.1.clone() <= im_hi
}

/// Choose once at pivot: deepest delivered interior seat from the previous
/// live view (even if it left the new viewport — off-screen refs stay useful),
/// otherwise the new view's center. Progress within a view never calls this.
// r[impl cz.depth.reference-sticky-selection+1]
pub fn select_reference_request<T: Mandelbrotable>(
    previous: Option<(&WorkContext<T>, &(ObjectivePosAndZoom, (u32, u32)))>,
    new_frame: &(ObjectivePosAndZoom, (u32, u32)),
) -> ReferenceRequest {
    let c = previous
        .and_then(|(context, frame)| {
            context
                .points
                .iter()
                .enumerate()
                .filter(|(_, point)| point.delivered && point.repeats && !point.escapes)
                .max_by_key(|(_, point)| point.iterations)
                .map(|(index, _point)| {
                    let seat = index as u32 % context.res.0;
                    let row = index as u32 / context.res.0;
                    objective_c(frame, seat, row)
                })
        })
        .unwrap_or_else(|| {
            let center = (new_frame.1.0 / 2, new_frame.1.1 / 2);
            objective_c(new_frame, center.0, center.1)
        });
    ReferenceRequest {
        c,
        precision_bits: crate::reference::bits_for_zoom(
            new_frame.0.zoom_pot as i64,
            PIXELS_PER_UNIT_POT,
        ),
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
    use crate::constants::TEST_SCREEN_RES;
    use crate::assemblies::workgroup::screen_worker::workshift::from_stencil;
    use crate::utils::ObjectivePosAndZoom;

    fn request(c: i32) -> ReferenceRequest {
        ReferenceRequest {
            c: (IntExp::from(c), IntExp::ZERO),
            precision_bits: 128,
        }
    }

    #[test]
    // r[verify cz.depth.reference-latest-wins+1]
    fn newer_request_replaces_in_progress_job() {
        let mut state = ReferenceWorkerState::new();
        state.replace(request(0));
        assert!(state.work_for(Duration::ZERO).is_none());
        state.replace(request(-1));
        let published = state.work_for(Duration::from_secs(1)).unwrap();
        assert_eq!(published.c.0, IntExp::from(-1));
        assert_eq!(published.generation, 1);
        assert!(published.orbit.period.is_some() || published.orbit.escaped);
    }

    #[test]
    // r[verify cz.depth.reference-whole-snapshot+1]
    // r[verify cz.depth.reference-until-done+1]
    fn publication_moves_one_complete_snapshot_and_increments_generation() {
        let mut state = ReferenceWorkerState::new();
        state.replace(request(2));
        let first = state.work_for(Duration::from_secs(1)).unwrap();
        assert!(first.orbit.escaped);
        assert!(state.work_for(Duration::from_secs(1)).is_none());

        state.replace(request(-1));
        let second = state.work_for(Duration::from_secs(1)).unwrap();
        assert_eq!(second.generation, first.generation + 1);
        assert!(second.orbit.period.is_some());
    }

    #[test]
    // r[verify cz.depth.reference-until-done+1]
    fn never_publishes_a_finite_incomplete_orbit() {
        // Period-1 center takes many rug steps; a 0-budget bout must not publish
        // a truncated orbit (the old max_iterations wall).
        let mut state = ReferenceWorkerState::new();
        state.replace(ReferenceRequest {
            c: (IntExp::ZERO, IntExp::ZERO),
            precision_bits: 128,
        });
        assert!(
            state.work_for(Duration::ZERO).is_none(),
            "zero-budget must not publish an incomplete orbit"
        );
        let job_len = state
            .job
            .as_ref()
            .map(|j| j.orbit.iterates.len())
            .unwrap_or(0);
        assert!(
            job_len < 1000 || state.job.as_ref().unwrap().orbit.period.is_some(),
            "must not treat a length wall as completion"
        );
    }

    fn frame() -> (ObjectivePosAndZoom, (u32, u32)) {
        (
            ObjectivePosAndZoom {
                pos: (IntExp::from(-2), IntExp::from(1)),
                zoom_pot: -2,
            },
            TEST_SCREEN_RES,
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
        let shallow = (1u32, 1u32);
        let deep = (30u32, 40u32);
        let shallow_i = (shallow.1 * TEST_SCREEN_RES.0 + shallow.0) as usize;
        let deep_i = (deep.1 * TEST_SCREEN_RES.0 + deep.0) as usize;
        context.points[shallow_i].delivered = true;
        context.points[shallow_i].repeats = true;
        context.points[shallow_i].iterations = 12;
        context.points[deep_i].delivered = true;
        context.points[deep_i].repeats = true;
        context.points[deep_i].iterations = 80;

        let selected = select_reference_request(Some((&context, &f)), &f);
        assert_eq!(selected.c, objective_c(&f, deep.0, deep.1));

        let fallback = select_reference_request::<f64>(None, &f);
        assert_eq!(
            fallback.c,
            objective_c(&f, TEST_SCREEN_RES.0 / 2, TEST_SCREEN_RES.1 / 2)
        );
    }

    #[test]
    // r[verify cz.depth.reference-sticky-selection+1]
    fn sticky_selection_keeps_interior_outside_new_view() {
        let old = frame();
        let mut context = from_stencil::<f64>(old.clone(), None).unwrap();
        let deep = (30u32, 40u32);
        let deep_i = (deep.1 * TEST_SCREEN_RES.0 + deep.0) as usize;
        context.points[deep_i].delivered = true;
        context.points[deep_i].repeats = true;
        context.points[deep_i].iterations = 80;
        // Zoom hard into a distant corner that does not contain the previous
        // deepest interior seat — still keep that sticky c (greedy reuse).
        let new = (
            ObjectivePosAndZoom {
                pos: (IntExp::from(1), IntExp::from(-1)),
                zoom_pot: 8,
            },
            old.1,
        );
        let old_deep = objective_c(&old, deep.0, deep.1);
        assert!(
            !reference_c_covers_frame(&old_deep, &new),
            "fixture must place old deep c outside the new view"
        );
        let selected = select_reference_request(Some((&context, &old)), &new);
        assert_eq!(
            selected.c, old_deep,
            "off-screen sticky interior must still be requested"
        );
    }

    #[test]
    // r[verify cz.depth.reference-coverage+1]
    fn coverage_accepts_center_of_same_view() {
        let f = frame();
        let c = objective_c(&f, f.1.0 / 2, f.1.1 / 2);
        assert!(reference_c_covers_frame(&c, &f));
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
