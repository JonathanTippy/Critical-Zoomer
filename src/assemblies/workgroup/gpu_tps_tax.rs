// read delivery.md for project context
//! Cheap sync-tax tallies for the GPU ≥3000 TPS ladder (experiment Steps 1–2).
//! Prefer env `CZ_GPU_TPS_TAX=1` or test cfg; atomics so probe can print without API churn.

use std::sync::atomic::{AtomicU64, Ordering};

static MAPS: AtomicU64 = AtomicU64::new(0);
static WAITS: AtomicU64 = AtomicU64::new(0);
static HARVESTS: AtomicU64 = AtomicU64::new(0);
static CPU_BRIDGE: AtomicU64 = AtomicU64::new(0);
static NOMAP_SCATTERS: AtomicU64 = AtomicU64::new(0);
static COUNTER_POLLS: AtomicU64 = AtomicU64::new(0);
static MAPPED_SCATTERS: AtomicU64 = AtomicU64::new(0);

fn enabled() -> bool {
    cfg!(test)
        || std::env::var_os("CZ_GPU_TPS_TAX").is_some()
}

pub fn reset() {
    MAPS.store(0, Ordering::Relaxed);
    WAITS.store(0, Ordering::Relaxed);
    HARVESTS.store(0, Ordering::Relaxed);
    CPU_BRIDGE.store(0, Ordering::Relaxed);
    NOMAP_SCATTERS.store(0, Ordering::Relaxed);
    COUNTER_POLLS.store(0, Ordering::Relaxed);
    MAPPED_SCATTERS.store(0, Ordering::Relaxed);
}

pub fn bump_map() {
    if enabled() {
        MAPS.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_wait() {
    if enabled() {
        WAITS.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_harvest() {
    if enabled() {
        HARVESTS.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_cpu_bridge() {
    if enabled() {
        CPU_BRIDGE.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_nomap_scatter() {
    if enabled() {
        NOMAP_SCATTERS.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_mapped_scatter() {
    if enabled() {
        MAPPED_SCATTERS.fetch_add(1, Ordering::Relaxed);
    }
}
pub fn bump_counter_poll() {
    if enabled() {
        COUNTER_POLLS.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub maps: u64,
    pub waits: u64,
    pub harvests: u64,
    pub cpu_bridge_publishes: u64,
    pub nomap_scatters: u64,
    pub mapped_scatters: u64,
    pub counter_polls: u64,
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        maps: MAPS.load(Ordering::Relaxed),
        waits: WAITS.load(Ordering::Relaxed),
        harvests: HARVESTS.load(Ordering::Relaxed),
        cpu_bridge_publishes: CPU_BRIDGE.load(Ordering::Relaxed),
        nomap_scatters: NOMAP_SCATTERS.load(Ordering::Relaxed),
        mapped_scatters: MAPPED_SCATTERS.load(Ordering::Relaxed),
        counter_polls: COUNTER_POLLS.load(Ordering::Relaxed),
    }
}

impl Snapshot {
    pub fn format_line(&self) -> String {
        format!(
            "maps={} waits={} harvests={} cpu_bridge={} nomap={} mapped_scatter={} counter_polls={}",
            self.maps,
            self.waits,
            self.harvests,
            self.cpu_bridge_publishes,
            self.nomap_scatters,
            self.mapped_scatters,
            self.counter_polls
        )
    }
}
