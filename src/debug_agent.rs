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
