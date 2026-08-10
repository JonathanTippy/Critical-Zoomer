//! Session debug NDJSON logger — **disabled on the hot path**.
//!
//! Former file appends in `workshift` / HUD telemetry were a silent quality
//! regression (Criterion first-publish / full-frame). Keep the symbols so call
//! sites compile; do not reopen disk I/O without an explicit debug flag.
#![allow(dead_code)]

/// No-op. Was NDJSON append under `.cursor/debug-*.log`.
#[inline(always)]
pub fn log(_hypothesis_id: &str, _location: &str, _message: &str, _data_json: &str) {}

/// Always false — never sample into a disabled logger.
#[inline(always)]
pub fn should_sample(_every: u64) -> bool {
    false
}

/// No-op HUD debug logger.
#[inline(always)]
pub fn log_hud(_hypothesis_id: &str, _location: &str, _message: &str, _data_json: &str) {}

#[cfg(test)]
mod mutant_kill {
    use super::*;

    /// Thought-killed pin: `should_sample -> true` would re-enable hot-path sampling.
    #[test]
    fn should_sample_stays_hard_off() {
        assert!(!should_sample(0));
        assert!(!should_sample(1));
        assert!(!should_sample(u64::MAX));
        assert_ne!(should_sample(1), true);
    }
}
