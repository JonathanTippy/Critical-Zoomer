//! Session debug helpers.
//!
//! Hot-path NDJSON logging stays **hard off**. Optional CPU busy probes activate
//! only when `CZ_PROFILE_CPU=1` (writes `/tmp/cz_cpu_profile.log` or
//! `CZ_PROFILE_CPU_OUT`).
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

static PROFILE_CPU: AtomicBool = AtomicBool::new(false);

static ESCAPER_NS: AtomicU64 = AtomicU64::new(0);
static COLORER_NS: AtomicU64 = AtomicU64::new(0);
static COLLECTOR_NS: AtomicU64 = AtomicU64::new(0);
static WORKER_SHIFT_NS: AtomicU64 = AtomicU64::new(0);
static WORKER_LOOP_NS: AtomicU64 = AtomicU64::new(0);
static WINDOW_NS: AtomicU64 = AtomicU64::new(0);

static ESCAPER_CALLS: AtomicU64 = AtomicU64::new(0);
static COLORER_CALLS: AtomicU64 = AtomicU64::new(0);
static COLLECTOR_CALLS: AtomicU64 = AtomicU64::new(0);
static WORKER_SHIFT_CALLS: AtomicU64 = AtomicU64::new(0);
static WORKER_LOOP_CALLS: AtomicU64 = AtomicU64::new(0);
static WINDOW_CALLS: AtomicU64 = AtomicU64::new(0);
static WORKER_WORKING_WAKES: AtomicU64 = AtomicU64::new(0);
static WORKER_PARK_WAKES: AtomicU64 = AtomicU64::new(0);

/// Escaper cadence RCA counters (enabled with `CZ_ESCAPE_RCA=1`).
static ESC_RCA: AtomicBool = AtomicBool::new(false);
static ESC_RCA_WAKES: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_BODY_NS: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_CONVERT_NS: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_SEND_OK: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_SEND_FULL: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_SEND_BLOCKED: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_NO_VALUES: AtomicU64 = AtomicU64::new(0);
static ESC_RCA_PACKAGES_TAKEN: AtomicU64 = AtomicU64::new(0);

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

/// Enable in-process busy-time profiling when `CZ_PROFILE_CPU` is set.
pub fn init_cpu_profile_from_env() {
    let on = std::env::var_os("CZ_PROFILE_CPU").is_some();
    PROFILE_CPU.store(on, Ordering::Relaxed);
    if !on {
        return;
    }
    let out = std::env::var("CZ_PROFILE_CPU_OUT")
        .unwrap_or_else(|_| "/tmp/cz_cpu_profile.log".to_string());
    let _ = std::fs::write(
        &out,
        format!(
            "# cz cpu profile start {:?}\n# columns: t_ms escaper_ms colorer_ms collector_ms worker_shift_ms worker_loop_ms window_ms calls… working_wakes park_wakes\n",
            Instant::now()
        ),
    );
    std::thread::Builder::new()
        .name("cz-cpu-profile".into())
        .spawn(move || {
            let t0 = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let line = format!(
                    "t_ms={} escaper_ms={:.3} colorer_ms={:.3} collector_ms={:.3} worker_shift_ms={:.3} worker_loop_ms={:.3} window_ms={:.3} escaper_n={} colorer_n={} collector_n={} worker_shift_n={} worker_loop_n={} window_n={} working_wakes={} park_wakes={}\n",
                    t0.elapsed().as_millis(),
                    take_ms(&ESCAPER_NS),
                    take_ms(&COLORER_NS),
                    take_ms(&COLLECTOR_NS),
                    take_ms(&WORKER_SHIFT_NS),
                    take_ms(&WORKER_LOOP_NS),
                    take_ms(&WINDOW_NS),
                    ESCAPER_CALLS.swap(0, Ordering::Relaxed),
                    COLORER_CALLS.swap(0, Ordering::Relaxed),
                    COLLECTOR_CALLS.swap(0, Ordering::Relaxed),
                    WORKER_SHIFT_CALLS.swap(0, Ordering::Relaxed),
                    WORKER_LOOP_CALLS.swap(0, Ordering::Relaxed),
                    WINDOW_CALLS.swap(0, Ordering::Relaxed),
                    WORKER_WORKING_WAKES.swap(0, Ordering::Relaxed),
                    WORKER_PARK_WAKES.swap(0, Ordering::Relaxed),
                );
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&out)
                    .and_then(|mut f| {
                        use std::io::Write;
                        f.write_all(line.as_bytes())
                    });
            }
        })
        .ok();
}

fn take_ms(counter: &AtomicU64) -> f64 {
    counter.swap(0, Ordering::Relaxed) as f64 / 1_000_000.0
}

pub struct BusyGuard {
    start: Instant,
    ns: &'static AtomicU64,
    calls: &'static AtomicU64,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        let dt = self.start.elapsed().as_nanos();
        self.ns.fetch_add(dt as u64, Ordering::Relaxed);
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(always)]
fn guard(ns: &'static AtomicU64, calls: &'static AtomicU64) -> Option<BusyGuard> {
    if !PROFILE_CPU.load(Ordering::Relaxed) {
        return None;
    }
    Some(BusyGuard {
        start: Instant::now(),
        ns,
        calls,
    })
}

#[inline(always)]
pub fn busy_escaper() -> Option<BusyGuard> {
    guard(&ESCAPER_NS, &ESCAPER_CALLS)
}
#[inline(always)]
pub fn busy_colorer() -> Option<BusyGuard> {
    guard(&COLORER_NS, &COLORER_CALLS)
}
#[inline(always)]
pub fn busy_collector() -> Option<BusyGuard> {
    guard(&COLLECTOR_NS, &COLLECTOR_CALLS)
}
#[inline(always)]
pub fn busy_worker_shift() -> Option<BusyGuard> {
    guard(&WORKER_SHIFT_NS, &WORKER_SHIFT_CALLS)
}
#[inline(always)]
pub fn busy_worker_loop() -> Option<BusyGuard> {
    guard(&WORKER_LOOP_NS, &WORKER_LOOP_CALLS)
}
#[inline(always)]
pub fn busy_window() -> Option<BusyGuard> {
    guard(&WINDOW_NS, &WINDOW_CALLS)
}

#[inline(always)]
pub fn note_worker_working() {
    if PROFILE_CPU.load(Ordering::Relaxed) {
        WORKER_WORKING_WAKES.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn note_worker_park_wake() {
    if PROFILE_CPU.load(Ordering::Relaxed) {
        WORKER_PARK_WAKES.fetch_add(1, Ordering::Relaxed);
    }
}

/// Enable escaper cadence RCA counters (`CZ_ESCAPE_RCA=1` or explicit call).
pub fn init_escape_rca_from_env() {
    if std::env::var_os("CZ_ESCAPE_RCA").is_some() {
        enable_escape_rca();
    }
}

pub fn enable_escape_rca() {
    ESC_RCA.store(true, Ordering::Relaxed);
    reset_escape_rca();
}

pub fn reset_escape_rca() {
    for c in [
        &ESC_RCA_WAKES,
        &ESC_RCA_BODY_NS,
        &ESC_RCA_CONVERT_NS,
        &ESC_RCA_SEND_OK,
        &ESC_RCA_SEND_FULL,
        &ESC_RCA_SEND_BLOCKED,
        &ESC_RCA_NO_VALUES,
        &ESC_RCA_PACKAGES_TAKEN,
    ] {
        c.store(0, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, Default)]
pub struct EscapeRcaSnapshot {
    pub wakes: u64,
    pub body_ms: f64,
    pub convert_ms: f64,
    pub send_ok: u64,
    pub send_full: u64,
    pub send_blocked: u64,
    pub no_values: u64,
    pub packages_taken: u64,
}

pub fn snapshot_escape_rca() -> EscapeRcaSnapshot {
    EscapeRcaSnapshot {
        wakes: ESC_RCA_WAKES.load(Ordering::Relaxed),
        body_ms: ESC_RCA_BODY_NS.load(Ordering::Relaxed) as f64 / 1e6,
        convert_ms: ESC_RCA_CONVERT_NS.load(Ordering::Relaxed) as f64 / 1e6,
        send_ok: ESC_RCA_SEND_OK.load(Ordering::Relaxed),
        send_full: ESC_RCA_SEND_FULL.load(Ordering::Relaxed),
        send_blocked: ESC_RCA_SEND_BLOCKED.load(Ordering::Relaxed),
        no_values: ESC_RCA_NO_VALUES.load(Ordering::Relaxed),
        packages_taken: ESC_RCA_PACKAGES_TAKEN.load(Ordering::Relaxed),
    }
}

#[inline(always)]
pub fn esc_rca_wake() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_WAKES.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_no_values() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_NO_VALUES.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_package_taken() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_PACKAGES_TAKEN.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_send_ok() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_SEND_OK.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_send_full() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_SEND_FULL.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_send_blocked() {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_SEND_BLOCKED.fetch_add(1, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_add_body_ns(ns: u64) {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_BODY_NS.fetch_add(ns, Ordering::Relaxed);
    }
}
#[inline(always)]
pub fn esc_rca_add_convert_ns(ns: u64) {
    if ESC_RCA.load(Ordering::Relaxed) {
        ESC_RCA_CONVERT_NS.fetch_add(ns, Ordering::Relaxed);
    }
}

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
