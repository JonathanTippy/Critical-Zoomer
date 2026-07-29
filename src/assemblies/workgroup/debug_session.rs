//! Temporary debug-session NDJSON logger (session 4dca53). Do not ship.
// #region agent log
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_PATH: &str = "/home/jonathan/git/therealcriticalzoomer/.cursor/debug-4dca53.log";
const SESSION: &str = "4dca53";

static RPC_N: AtomicU64 = AtomicU64::new(0);
static GPU_N: AtomicU64 = AtomicU64::new(0);
static PUB_N: AtomicU64 = AtomicU64::new(0);
static ITS_WAKE_N: AtomicU64 = AtomicU64::new(0);

pub fn rpc_tick() -> u64 {
    RPC_N.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn gpu_tick() -> u64 {
    GPU_N.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn pub_tick() -> u64 {
    PUB_N.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn its_wake_tick() -> u64 {
    ITS_WAKE_N.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn should_sample(n: u64) -> bool {
    n <= 8 || n % 64 == 0
}

pub fn log(hypothesis_id: &str, location: &str, message: &str, data_json: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(
            f,
            "{{\"sessionId\":\"{SESSION}\",\"hypothesisId\":\"{hypothesis_id}\",\"location\":\"{location}\",\"message\":\"{message}\",\"data\":{data_json},\"timestamp\":{ts}}}"
        );
    }
}
// #endregion
