//! Tile publisher: calibrated → Answer with proximate bias / NORES.
//! Design: docs/design/tile_publisher.md
// r[impl cz.int.publisher-nores-bias+1]
// r[impl cz.int.publish-cadence+1]

use std::time::{Duration, Instant};

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::constants::NORES_ANSWER;
use crate::constants::TILE_EDGE_LENGTH;
use crate::range::Range;

/// Convert one calibrated seat using optional proximate bias.
/// Finished Outside/Inside collapse honestly (bias when provided for continuity).
/// Agnostic with proximate → biased guess; without proximate → NORES (never invent Inside).
///
/// Progressive publish note: flood/hint paths must not finish Agnostic seats into the
/// headgroup as sole content — see TileSession (Agnostic hints go to GPU work).
pub fn publish_seat(calibrated: CalibratedAnswer, proximate: Option<Answer>) -> Answer {
    match &calibrated.result {
        CalibratedMandelbrotResult::Agnostic { .. } => match proximate {
            Some(bias) => calibrated.guess_biased(bias),
            None => NORES_ANSWER,
        },
        CalibratedMandelbrotResult::Outside { .. }
        | CalibratedMandelbrotResult::Inside { .. } => match proximate {
            Some(bias) => calibrated.guess_biased(bias),
            None => collapse_exact(calibrated),
        },
    }
}

/// Publish a full tile: each present calibrated seat uses matching proximate if any.
pub fn publish_tile(
    calibrated: &Tile<CalibratedAnswer>,
    proximate: Option<&Tile<Answer>>,
) -> Tile<Answer> {
    let mut out = Tile::new(calibrated.origin_seat, calibrated.magnification_pot);
    for y in 0..TILE_EDGE_LENGTH {
        for x in 0..TILE_EDGE_LENGTH {
            let local = (x, y);
            let Some(cal) = calibrated.get(local) else {
                continue;
            };
            let bias = proximate.and_then(|p| p.get(local));
            out.set(local, publish_seat(cal, bias));
        }
    }
    out
}

fn collapse_exact(answer: CalibratedAnswer) -> Answer {
    match answer.result {
        CalibratedMandelbrotResult::Outside {
            escape_time_r2,
            escape_z,
        } => Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: escape_time_r2.lower_bound,
                escape_z: (escape_z.0.lower_bound, escape_z.1.lower_bound),
            },
            min_magnitude_time: answer.min_magnitude_time.lower_bound,
            min_magnitude: answer.min_magnitude.lower_bound,
        },
        CalibratedMandelbrotResult::Inside { period } => Answer {
            result: MandelbrotResult::Inside {
                period: period.lower_bound,
            },
            min_magnitude_time: answer.min_magnitude_time.lower_bound,
            min_magnitude: answer.min_magnitude.lower_bound,
        },
        CalibratedMandelbrotResult::Agnostic { period, .. } => {
            // Should not reach here; Agnostic handled by publish_seat.
            Answer {
                result: MandelbrotResult::Inside {
                    period: period.lower_bound,
                },
                min_magnitude_time: answer.min_magnitude_time.lower_bound,
                min_magnitude: answer.min_magnitude.lower_bound,
            }
        }
    }
}

/// Design band: at least 30/s, at most 1000/s while incomplete; idle when complete.
pub const PUBLISH_MIN_HZ: f64 = 30.0;
pub const PUBLISH_MAX_HZ: f64 = 1000.0;

/// Memory limit bump request from workgroup publisher → headgroup (raises slider floor).
// r[impl cz.int.memory-bump+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryBump {
    pub needed_bytes: usize,
}

/// Thin live-path owner of publish cadence (+ optional bump send helper).
/// Full publisher actor extract is follow-up; screen_worker hosts this for now.
#[derive(Debug)]
pub struct LivePublisher {
    pub cadence: PublishCadence,
    pub memory_limit_bytes: usize,
}

impl LivePublisher {
    pub fn new(incomplete: bool) -> Self {
        LivePublisher {
            cadence: PublishCadence::new(incomplete),
            memory_limit_bytes: 1_000_000_000,
        }
    }

    /// Gate for the live drain/flush path: under max Hz, and either work is ready
    /// or the min-30 floor is overdue.
    pub fn should_publish(&mut self, now: Instant, has_work: bool) -> bool {
        self.cadence.should_publish(now, has_work)
    }

    pub fn record_publish(&mut self, now: Instant) {
        self.cadence.record_publish(now);
    }

    pub fn set_incomplete(&mut self, incomplete: bool) {
        self.cadence.set_incomplete(incomplete);
    }
}

#[derive(Debug)]
pub struct PublishCadence {
    incomplete: bool,
    window_start: Instant,
    publishes_in_window: u32,
    last_publish: Option<Instant>,
}

impl Default for PublishCadence {
    fn default() -> Self {
        Self::new(true)
    }
}

impl PublishCadence {
    pub fn new(incomplete: bool) -> Self {
        PublishCadence {
            incomplete,
            window_start: Instant::now(),
            publishes_in_window: 0,
            last_publish: None,
        }
    }

    pub fn new_at(incomplete: bool, now: Instant) -> Self {
        PublishCadence {
            incomplete,
            window_start: now,
            publishes_in_window: 0,
            last_publish: None,
        }
    }

    pub fn set_incomplete(&mut self, incomplete: bool) {
        self.incomplete = incomplete;
        if !incomplete {
            self.publishes_in_window = 0;
        }
    }

    pub fn incomplete(&self) -> bool {
        self.incomplete
    }

    fn roll_window(&mut self, now: Instant) {
        if now.duration_since(self.window_start) > Duration::from_secs(1) {
            self.window_start = now;
            self.publishes_in_window = 0;
        }
    }

    /// True when incomplete and ≥1/30s since last publish (or never published).
    pub fn min_interval_due(&self, now: Instant) -> bool {
        if !self.incomplete {
            return false;
        }
        match self.last_publish {
            None => true,
            Some(last) => {
                now.duration_since(last) >= Duration::from_secs_f64(1.0 / PUBLISH_MIN_HZ)
            }
        }
    }

    /// Whether a publish is allowed now under the max-1000 Hz cap while incomplete.
    pub fn allow_publish(&mut self, now: Instant) -> bool {
        if !self.incomplete {
            return false;
        }
        self.roll_window(now);
        if self.publishes_in_window as f64 >= PUBLISH_MAX_HZ {
            return false;
        }
        if let Some(last) = self.last_publish {
            let min_gap = Duration::from_secs_f64(1.0 / PUBLISH_MAX_HZ);
            if now.duration_since(last) < min_gap {
                return false;
            }
        }
        true
    }

    /// Live-path gate: respect max Hz; force when min-30 is overdue; else only if work ready.
    pub fn should_publish(&mut self, now: Instant, has_work: bool) -> bool {
        if !self.allow_publish(now) {
            return false;
        }
        has_work || self.min_interval_due(now)
    }

    pub fn record_publish(&mut self, now: Instant) {
        self.last_publish = Some(now);
        self.publishes_in_window = self.publishes_in_window.saturating_add(1);
    }

    /// Minimum expected publishes in a full second while incomplete (design floor).
    pub fn min_publishes_per_second() -> u32 {
        PUBLISH_MIN_HZ as u32
    }

    pub fn max_publishes_per_second() -> u32 {
        PUBLISH_MAX_HZ as u32
    }
}

/// Helper for tests: exact Outside calibrated seat.
pub fn exact_outside(escape_time: u64) -> CalibratedAnswer {
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Outside {
            escape_time_r2: Range {
                lower_bound: escape_time,
                upper_bound: escape_time,
            },
            escape_z: (
                Range {
                    lower_bound: 2.0,
                    upper_bound: 2.0,
                },
                Range {
                    lower_bound: 0.0,
                    upper_bound: 0.0,
                },
            ),
        },
        min_magnitude_time: Range {
            lower_bound: 0,
            upper_bound: 0,
        },
        min_magnitude: Range {
            lower_bound: 4.0,
            upper_bound: 4.0,
        },
        highlights: CalibratedHighlights {
            in_filament: Range {
                lower_bound: false,
                upper_bound: false,
            },
            out_filament: Range {
                lower_bound: false,
                upper_bound: false,
            },
            small_time_edge: Range {
                lower_bound: false,
                upper_bound: false,
            },
            node: Range {
                lower_bound: false,
                upper_bound: false,
            },
        },
    }
}

pub fn agnostic_wide() -> CalibratedAnswer {
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Agnostic {
            period: Range {
                lower_bound: 0,
                upper_bound: 100,
            },
            escape_time_r2: Range {
                lower_bound: 1,
                upper_bound: 1_000_000,
            },
            escape_z: (
                Range {
                    lower_bound: -100.0,
                    upper_bound: 100.0,
                },
                Range {
                    lower_bound: -100.0,
                    upper_bound: 100.0,
                },
            ),
        },
        min_magnitude_time: Range {
            lower_bound: 0,
            upper_bound: 1000,
        },
        min_magnitude: Range {
            lower_bound: 0.0,
            upper_bound: 4.0,
        },
        highlights: CalibratedHighlights {
            in_filament: Range {
                lower_bound: false,
                upper_bound: true,
            },
            out_filament: Range {
                lower_bound: false,
                upper_bound: true,
            },
            small_time_edge: Range {
                lower_bound: false,
                upper_bound: true,
            },
            node: Range {
                lower_bound: false,
                upper_bound: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn agnostic_without_proximate_is_nores() {
        let out = publish_seat(agnostic_wide(), None);
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 1);
            }
            MandelbrotResult::Inside { .. } => panic!("must not invent Inside from empty proximate"),
        }
        assert!(out.min_magnitude.is_infinite());
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn agnostic_keeps_proximate_bias_when_in_bounds() {
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 50,
                escape_z: (1.0, 0.0),
            },
            min_magnitude_time: 10,
            min_magnitude: 1.0,
        };
        let out = publish_seat(agnostic_wide(), Some(bias));
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 50);
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside bias"),
        }
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn disproven_proximate_clamps_to_nearest_bound() {
        let cal = CalibratedAnswer {
            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range {
                    lower_bound: 10,
                    upper_bound: 20,
                },
                escape_z: (
                    Range {
                        lower_bound: 2.0,
                        upper_bound: 2.0,
                    },
                    Range {
                        lower_bound: 0.0,
                        upper_bound: 0.0,
                    },
                ),
            },
            min_magnitude_time: Range {
                lower_bound: 0,
                upper_bound: 0,
            },
            min_magnitude: Range {
                lower_bound: 4.0,
                upper_bound: 4.0,
            },
            highlights: exact_outside(1).highlights,
        };
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 100,
                escape_z: (2.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 4.0,
        };
        let out = publish_seat(cal, Some(bias));
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 20, "disproven high bias clamps to upper");
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside"),
        }
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_idle_when_complete() {
        let mut c = PublishCadence::new(false);
        assert!(!c.allow_publish(Instant::now()));
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_allows_while_incomplete() {
        let mut c = PublishCadence::new(true);
        let t0 = Instant::now();
        assert!(c.allow_publish(t0));
        c.record_publish(t0);
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_caps_at_max_hz() {
        let t0 = Instant::now();
        let mut c = PublishCadence::new_at(true, t0);
        let gap = Duration::from_millis(1);
        let max = PublishCadence::max_publishes_per_second();
        for i in 0..max {
            let t = t0 + gap * i;
            assert!(c.allow_publish(t), "i={i}");
            c.record_publish(t);
        }
        assert!(!c.allow_publish(t0 + gap * max));
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_min_interval_forces_without_work() {
        let t0 = Instant::now();
        let mut c = PublishCadence::new_at(true, t0);
        assert!(c.should_publish(t0, false), "first publish is due");
        c.record_publish(t0);
        assert!(
            !c.should_publish(t0 + Duration::from_millis(1), false),
            "too soon and no work"
        );
        assert!(
            c.should_publish(t0 + Duration::from_millis(1), true),
            "work ready under max"
        );
        let overdue = t0 + Duration::from_millis(34); // > 1/30s
        assert!(
            c.should_publish(overdue, false),
            "min-30 overdue must force publish"
        );
    }
}
