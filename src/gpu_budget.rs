//! How much GPU time a compute submission may take before it starts costing frames.
//!
//! One shared device means one queue, so compute dispatches and the window's
//! present serialize against each other. The headgroup is capped at 60fps
//! specifically "to avoid wasting GPU working time" (headgroup.md), which makes
//! the leftover time the workgroup's to use — but only the leftover. A dispatch
//! that overruns delays the next present, and the user must see movement within
//! 17ms (requirements: Fast).
//!
//! So the worker asks the budget how many iterations it may dispatch, and the
//! budget answers from what previous dispatches of that shape actually cost.
// r[impl cz.fast.natural-zoom-2x+1]

use std::time::Duration;

/// One frame at 60Hz: the deadline the user's movement must be visible within.
pub const FRAME_BUDGET: Duration = Duration::from_micros(16_667);

/// Share of the frame the compute side may occupy in one submission. The rest is
/// reserved for the present itself plus the egui pass that precedes it.
const COMPUTE_SHARE: f64 = 0.5;

/// Never dispatch less than this; below it, submission overhead dominates the
/// work and scheduling stops being "insignificant compared with time spent
/// working" (requirements: Fast).
const MIN_ITERATIONS: u32 = 64;

/// Ceiling on a single submission regardless of how fast the device looks, so a
/// mispredicted cost cannot strand the queue for multiple frames.
const MAX_ITERATIONS: u32 = 1_000_000;

/// Tracks the observed cost of compute submissions and sizes the next one.
///
/// Cost is tracked per point-iteration (one point advanced one iteration) so the
/// estimate transfers across batch sizes, which vary with the scheduler's work
/// front.
#[derive(Clone, Copy, Debug)]
pub struct SubmissionBudget {
    /// Seconds per point-iteration, smoothed across submissions.
    seconds_per_point_iteration: f64,
    /// How much of a frame a compute submission may claim.
    compute_share: f64,
    observations: u32,
}

impl Default for SubmissionBudget {
    fn default() -> Self {
        SubmissionBudget::new()
    }
}

impl SubmissionBudget {
    pub fn new() -> Self {
        SubmissionBudget {
            // Deliberately pessimistic until measured: better to under-dispatch
            // for a few submissions than to blow the first frames of a zoom.
            seconds_per_point_iteration: 1.0e-8,
            compute_share: COMPUTE_SHARE,
            observations: 0,
        }
    }

    /// How many iterations `point_count` points may be advanced in one submission.
    pub fn iterations_for(&self, point_count: u32) -> u32 {
        if point_count == 0 {
            return 0;
        }
        let allowance = FRAME_BUDGET.as_secs_f64() * self.compute_share;
        let per_iteration = self.seconds_per_point_iteration * f64::from(point_count);
        if per_iteration <= 0.0 {
            return MAX_ITERATIONS;
        }
        let iterations = allowance / per_iteration;
        if !iterations.is_finite() {
            return MAX_ITERATIONS;
        }
        (iterations as u32).clamp(MIN_ITERATIONS, MAX_ITERATIONS)
    }

    /// Fold in what a completed submission actually cost.
    ///
    /// Smoothed rather than replaced, because a single submission can be delayed
    /// by unrelated queue traffic and should not whipsaw the next dispatch.
    pub fn observe(&mut self, point_count: u32, iterations: u32, elapsed: Duration) {
        let work = f64::from(point_count) * f64::from(iterations);
        if work <= 0.0 || elapsed.is_zero() {
            return;
        }
        let observed = elapsed.as_secs_f64() / work;
        if !observed.is_finite() || observed <= 0.0 {
            return;
        }
        self.observations = self.observations.saturating_add(1);
        // First real measurement replaces the pessimistic seed outright; after
        // that, an exponential average with a short memory.
        self.seconds_per_point_iteration = if self.observations == 1 {
            observed
        } else {
            self.seconds_per_point_iteration * 0.75 + observed * 0.25
        };
    }

    /// True once the budget is working from measurement rather than its seed.
    pub fn is_calibrated(&self) -> bool {
        self.observations > 0
    }

    pub fn seconds_per_point_iteration(&self) -> f64 {
        self.seconds_per_point_iteration
    }

    /// Observed iterations per second (point-iterations), once calibrated.
    pub fn estimated_ips(&self) -> f64 {
        let s = self.seconds_per_point_iteration;
        if s <= 0.0 || !s.is_finite() {
            return 0.0;
        }
        1.0 / s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batch_gets_no_dispatch() {
        assert_eq!(SubmissionBudget::new().iterations_for(0), 0);
    }

    #[test]
    fn a_slower_device_is_given_fewer_iterations() {
        let mut fast = SubmissionBudget::new();
        let mut slow = SubmissionBudget::new();
        fast.observe(1024, 1000, Duration::from_micros(500));
        slow.observe(1024, 1000, Duration::from_millis(50));
        assert!(
            fast.iterations_for(1024) > slow.iterations_for(1024)
            , "budget must shrink the dispatch on a device observed to be slower"
        );
    }

    #[test]
    fn more_points_means_fewer_iterations_each() {
        let mut budget = SubmissionBudget::new();
        budget.observe(1024, 1000, Duration::from_micros(500));
        assert!(
            budget.iterations_for(4096) <= budget.iterations_for(1024)
            , "a wider batch must not also get a longer bout"
        );
    }

    #[test]
    fn dispatch_stays_within_the_frame_allowance() {
        let mut budget = SubmissionBudget::new();
        budget.observe(1024, 1000, Duration::from_micros(800));
        let points = 4096u32;
        let iterations = budget.iterations_for(points);
        let predicted = budget.seconds_per_point_iteration()
            * f64::from(points)
            * f64::from(iterations);
        // MIN_ITERATIONS can force an overrun on a very slow device; that floor is
        // deliberate, so only assert the allowance when the floor is not binding.
        if iterations > MIN_ITERATIONS {
            assert!(
                predicted <= FRAME_BUDGET.as_secs_f64() * COMPUTE_SHARE + f64::EPSILON
                , "predicted {predicted}s exceeds the compute share of a frame"
            );
        }
    }

    #[test]
    fn nonsense_observations_do_not_corrupt_the_estimate() {
        let mut budget = SubmissionBudget::new();
        let seed = budget.seconds_per_point_iteration();
        budget.observe(0, 1000, Duration::from_millis(5));
        budget.observe(1024, 0, Duration::from_millis(5));
        budget.observe(1024, 1000, Duration::ZERO);
        assert_eq!(budget.seconds_per_point_iteration(), seed);
        assert!(!budget.is_calibrated());
    }
}
