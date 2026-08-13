use std::time::{Duration, Instant};
use std::collections::*;

/// Rolling 1-second rate of discrete events (completions or iterations).
// r[impl cz.depth.gear-hud+2]
#[derive(Debug, Default, Clone)]
pub struct RateCounter {
    events: VecDeque<(Instant, u64)>,
}

impl RateCounter {
    pub fn record(&mut self, count: u64, now: Instant) {
        if count == 0 {
            return;
        }
        self.events.push_back((now, count));
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        while let Some(front) = self.events.front() {
            if now.duration_since(front.0) > Duration::from_secs(10) {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    fn sum_in(&self, now: Instant, window: Duration) -> u64 {
        self.events
            .iter()
            .filter(|(t, _)| now.duration_since(*t) <= window)
            .map(|e| e.1)
            .sum()
    }

    /// Events per second over the trailing 1s window.
    pub fn rate(&mut self, now: Instant) -> f64 {
        self.prune(now);
        self.sum_in(now, Duration::from_secs(1)) as f64
    }

    /// Average events per second over the trailing 10s window.
    pub fn rate_10s(&mut self, now: Instant) -> f64 {
        self.prune(now);
        self.sum_in(now, Duration::from_secs(10)) as f64 / 10.0
    }
}

pub fn rolling_frame_calc(
    rolling_frame_info: &mut (
        VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , Option<Instant>
    )
    , timinginfo:Option<(Instant, u64, Duration, Duration)>
) -> (
    Option<((u64, Duration, Duration), (Duration, Duration))>
    , Option<((u64, Duration, Duration), (Duration, Duration))>
    , Option<((u64, Duration, Duration), (Duration, Duration))>
) {

    let start_instant = Instant::now();

    let rolling_frame_info_10s = &mut rolling_frame_info.0;
    let rolling_frame_info_1s = &mut rolling_frame_info.1;
    let rolling_frame_info_100ms = &mut rolling_frame_info.2;

    match timinginfo {
        Some(t) => {
            rolling_frame_info_10s.push_front(t);
            rolling_frame_info_1s.push_front(t);
            rolling_frame_info_100ms.push_front(t);
        }
        None => {}
    }

    // Fail closed if the HUD never latched a window start (was unwrap-panic).
    let Some(window_start) = rolling_frame_info.3 else {
        return (None, None, None);
    };

    let can_calculate_10s = window_start.elapsed() > Duration::from_secs(10);
    let can_calculate_1s = window_start.elapsed() > Duration::from_secs(1);
    let can_calculate_100ms = window_start.elapsed() > Duration::from_millis(100);

    loop {
        let length = rolling_frame_info_10s.len();
        if length == 0 {break;}
        if start_instant - rolling_frame_info_10s[length-1].0 > Duration::from_secs(10) {
            if rolling_frame_info_10s.len() > 1 {
                rolling_frame_info_10s.pop_back();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    loop {
        let length = rolling_frame_info_1s.len();
        if length == 0 {break;}
        if start_instant - rolling_frame_info_1s[length-1].0 > Duration::from_secs(1) {
            if rolling_frame_info_1s.len() > 1 {
                rolling_frame_info_1s.pop_back();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    loop {
        let length = rolling_frame_info_100ms.len();
        if length == 0 {break;}
        if start_instant - rolling_frame_info_100ms[length-1].0 > Duration::from_millis(100) {
            if rolling_frame_info_100ms.len() > 1 {
                rolling_frame_info_100ms.pop_back();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    (
        if can_calculate_10s { window_stats(rolling_frame_info_10s) } else { None },
        if can_calculate_1s { window_stats(rolling_frame_info_1s) } else { None },
        if can_calculate_100ms { window_stats(rolling_frame_info_100ms) } else { None },
    )
}

/// Average + worst over a non-empty window. Empty → None (avoid ÷0).
fn window_stats(
    q: &VecDeque<(Instant, u64, Duration, Duration)>,
) -> Option<((u64, Duration, Duration), (Duration, Duration))> {
    let length = q.len();
    if length == 0 {
        return None;
    }
    let length_u = length as u64;
    let length_n = length as u128;
    let total: (u64, Duration, Duration) = (
        q.iter().map(|f| f.1).sum(),
        q.iter().map(|f| f.2).sum(),
        q.iter().map(|f| f.3).sum(),
    );
    let average = (
        total.0 / length_u,
        Duration::from_nanos((total.1.as_nanos() / length_n) as u64),
        Duration::from_nanos((total.2.as_nanos() / length_n) as u64),
    );
    let mut worst = (Duration::ZERO, Duration::ZERO);
    for f in q.iter() {
        if f.2 > worst.0 {
            worst = (f.2, f.3);
        }
    }
    Some((average, worst))
}

/// Overlay: each fps line is 1s+10s; compute gear+type(+ref if pert); color/escape; PPS.
// r[impl cz.depth.gear-hud+2]
pub fn format_hud_overlay(
    fps_1s: f64,
    fps_10s: Option<f64>,
    pub_1s: f64,
    pub_10s: f64,
    esc_1s: f64,
    esc_10s: f64,
    col_1s: f64,
    col_10s: f64,
    ctrl_1s: f64,
    ctrl_10s: f64,
    gear: &str,
    type_label: &str,
    ref_label: Option<&str>,
    color: &str,
    escape: &str,
    pps: f64,
    ips: f64,
    ipp_txt: &str,
) -> String {
    let fps10 = match fps_10s {
        Some(v) => format!("  10s:{v:.1}"),
        None => String::new(),
    };
    let ref_bit = match ref_label {
        Some(r) => format!("  ref:{r}"),
        None => String::new(),
    };
    format!(
        "fps  1s:{fps_1s:.1}{fps10}\npub  1s:{pub_1s:.1}  10s:{pub_10s:.1}\nesc  1s:{esc_1s:.1}  10s:{esc_10s:.1}\ncol  1s:{col_1s:.1}  10s:{col_10s:.1}\nctrl  1s:{ctrl_1s:.1}  10s:{ctrl_10s:.1}\ngear:{gear}  type:{type_label}{ref_bit}\ncolor gear:{color}  escape gear:{escape}\npps:{pps:.0}  ips:{ips:.0}  {ipp_txt}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.depth.gear-hud+2]
    #[test]
    fn pps_counter_counts_completions_not_wip() {
        let mut c = RateCounter::default();
        let t0 = Instant::now();
        c.record(10, t0);
        c.record(0, t0); // WIP / empty must not inflate
        assert!((c.rate(t0) - 10.0).abs() < 1e-9);
    }

    // r[verify cz.depth.gear-hud+2]
    #[test]
    fn hud_telemetry_carries_gear_and_rates() {
        use crate::assemblies::structs::{HostStack, KernelMode, ReferenceStatus, ViewHud};
        use crate::delta_gear::ComputeGear;
        let hud = ViewHud {
            stack: HostStack::F64,
            mode: KernelMode::Pert,
            reference: ReferenceStatus::Complete,
            gear: ComputeGear::ScaledF64,
            points_delta: 3,
            iterations_delta: 1000,
            packages_dropped: 0,
            ..Default::default()
        };
        assert_eq!(hud.stack.hud_label(), "f64");
        assert_eq!(hud.mode.hud_label(), "pert");
        assert_eq!(hud.ref_hud_label(), "complete");
        assert_eq!(hud.gear.hud_label(), "S-F64");
        assert_eq!(hud.ipp, 0);
        assert!(!hud.ipp_final);
        let mut pps = RateCounter::default();
        let mut ips = RateCounter::default();
        let now = Instant::now();
        pps.record(hud.points_delta, now);
        ips.record(hud.iterations_delta, now);
        assert!((pps.rate(now) - 3.0).abs() < 1e-9);
        assert!((ips.rate(now) - 1000.0).abs() < 1e-9);
        let overlay = format_hud_overlay(
            36.7,
            Some(13.0),
            21.0,
            18.0,
            25.0,
            20.0,
            56.0,
            40.0,
            0.0,
            0.0,
            "naive",
            "i64",
            None,
            "GPU",
            "OG",
            28784.0,
            546896.0,
            "ipp:19",
        );
        let lines: Vec<&str> = overlay.lines().collect();
        assert_eq!(lines.len(), 8);
        assert!(lines[0].starts_with("fps  1s:36.7"));
        assert!(lines[0].contains("10s:13.0"));
        assert!(lines[1].starts_with("pub  1s:"));
        assert!(lines[2].starts_with("esc  1s:"));
        assert!(lines[3].starts_with("col  1s:"));
        assert!(lines[4].starts_with("ctrl  1s:"));
        assert_eq!(lines[5], "gear:naive  type:i64");
        assert_eq!(lines[6], "color gear:GPU  escape gear:OG");
        assert!(lines[7].contains("pps:"));
        assert!(lines[7].contains("ips:"));
        assert!(lines[7].contains("ipp:19"));
        let pert = format_hud_overlay(
            36.7,
            None,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            "pert",
            "f64",
            Some("complete"),
            "OG",
            "OG",
            1.0,
            1.0,
            "ipp:~0",
        );
        assert!(pert.contains("gear:pert  type:f64  ref:complete"));
        assert!(
            !pert.lines().next().unwrap().contains("10s:"),
            "paint fps omits 10s until that window exists"
        );
        assert!(!overlay.contains("mode:"));
        assert!(!overlay.contains("stack:"));
        assert!(!overlay.contains("drop:"));
        assert!(!overlay.contains("gaze:"));
        assert!(!overlay.contains("gear:F64"));
    }

        /// Thought-killed pins for HUD RateCounter (zero skip, 1s rate / 10s keep).
    #[test]
    fn mutant_kill_rate_counter_window() {
        let mut c = RateCounter::default();
        let t0 = Instant::now();
        c.record(0, t0);
        assert_eq!(c.rate(t0), 0.0);
        c.record(5, t0);
        c.record(7, t0);
        assert!((c.rate(t0) - 12.0).abs() < 1e-9);
        assert_ne!(c.rate(t0), 5.0); // must sum, not last-only
        // Events older than 1s must drop (Duration::from_secs(1) threshold).
        let t_old = t0 + Duration::from_millis(1001);
        c.record(1, t_old);
        assert!((c.rate(t_old) - 1.0).abs() < 1e-9);
        assert_ne!(c.rate(t_old), 13.0);
        // Exactly at 1s boundary: keep (prune uses > 1s, not >=).
        let mut c2 = RateCounter::default();
        let t1 = Instant::now();
        c2.record(3, t1);
        let t_edge = t1 + Duration::from_secs(1);
        assert!((c2.rate(t_edge) - 3.0).abs() < 1e-9);

        // Emission Instant in the past + query at now → silence ages to zero.
        let mut c3 = RateCounter::default();
        let emitted = Instant::now();
        c3.record(1, emitted);
        assert!((c3.rate(emitted) - 1.0).abs() < 1e-9);
        assert_eq!(c3.rate(emitted + Duration::from_millis(1001)), 0.0);
    }

    type Rolling = (
        VecDeque<(Instant, u64, Duration, Duration)>,
        VecDeque<(Instant, u64, Duration, Duration)>,
        VecDeque<(Instant, u64, Duration, Duration)>,
        Option<Instant>,
    );

    fn empty_rolling(start: Option<Instant>) -> Rolling {
        (VecDeque::new(), VecDeque::new(), VecDeque::new(), start)
    }

    #[test]
    fn mutant_kill_rolling_frame_gates_push_avg_worst() {
        // Missing window_start must fail closed (was unwrap panic).
        let mut none_start = empty_rolling(None);
        let r = rolling_frame_calc(&mut none_start, None);
        assert!(r.0.is_none() && r.1.is_none() && r.2.is_none());

        // Gates closed: elapsed < 100ms → all None.
        let mut early = empty_rolling(Some(Instant::now() - Duration::from_millis(50)));
        let r = rolling_frame_calc(&mut early, None);
        assert!(r.0.is_none() && r.1.is_none() && r.2.is_none());

        // 100ms open, 1s/10s closed; empty queues → None (÷0 fail-closed).
        let mut only_100 = empty_rolling(Some(Instant::now() - Duration::from_millis(150)));
        let r = rolling_frame_calc(&mut only_100, None);
        assert!(r.0.is_none() && r.1.is_none() && r.2.is_none());

        // Tri-queue push: one sample lands in all three deques.
        let now = Instant::now();
        let sample = (now, 10u64, Duration::from_millis(4), Duration::from_millis(1));
        let mut pushed = empty_rolling(Some(now - Duration::from_millis(150)));
        let r = rolling_frame_calc(&mut pushed, Some(sample));
        assert_eq!(pushed.0.len(), 1);
        assert_eq!(pushed.1.len(), 1);
        assert_eq!(pushed.2.len(), 1);
        // Only 100ms arm reports; avg count 10, worst d2=4ms.
        assert!(r.0.is_none() && r.1.is_none());
        let (avg, worst) = r.2.expect("100ms arm");
        assert_eq!(avg.0, 10);
        assert_eq!(avg.1, Duration::from_millis(4));
        assert_eq!(worst, (Duration::from_millis(4), Duration::from_millis(1)));

        // 1s gate: average / not last-only; worst is max of field .2.
        let t = Instant::now();
        let a = (t, 10u64, Duration::from_millis(4), Duration::from_millis(1));
        let b = (t, 20u64, Duration::from_millis(8), Duration::from_millis(3));
        let mut one_s = empty_rolling(Some(t - Duration::from_millis(1100)));
        one_s.1.push_front(a);
        one_s.1.push_front(b);
        one_s.2.push_front(a);
        one_s.2.push_front(b);
        let r = rolling_frame_calc(&mut one_s, None);
        let (avg, worst) = r.1.expect("1s arm");
        assert_eq!(avg.0, 15); // (10+20)/2, not 20 last-only, not 10*20
        assert_eq!(avg.1, Duration::from_millis(6));
        assert_eq!(worst, (Duration::from_millis(8), Duration::from_millis(3)));
        assert_ne!(worst.0, Duration::from_millis(4)); // not min

        // Prune keeps at least one stale sample (len>1 gate).
        let old = Instant::now() - Duration::from_secs(11);
        let mut prune = empty_rolling(Some(Instant::now() - Duration::from_secs(11)));
        prune.0.push_front((old, 1, Duration::from_millis(1), Duration::ZERO));
        let _ = rolling_frame_calc(&mut prune, None);
        assert_eq!(prune.0.len(), 1);
        assert!(rolling_frame_calc(&mut prune, None).0.is_some());
    }
}