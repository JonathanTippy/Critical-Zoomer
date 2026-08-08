use std::time::{Duration, Instant};
use std::collections::*;

/// Rolling 1-second rate of discrete events (completions or iterations).
// r[impl cz.depth.gear-hud+1]
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
            if now.duration_since(front.0) > Duration::from_secs(1) {
                self.events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Events per second over the trailing 1s window.
    pub fn rate(&mut self, now: Instant) -> f64 {
        self.prune(now);
        let total: u64 = self.events.iter().map(|e| e.1).sum();
        total as f64
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
    let window_start = rolling_frame_info.3;


    match timinginfo {
        Some(t) => {
            rolling_frame_info_10s.push_front(t);
            rolling_frame_info_1s.push_front(t);
            rolling_frame_info_100ms.push_front(t);
        }
        None => {}
    }



    let can_calculate_10s = window_start.unwrap().elapsed() > Duration::from_secs(10);
    let can_calculate_1s = window_start.unwrap().elapsed() > Duration::from_secs(1);
    let can_calculate_100ms = window_start.unwrap().elapsed() > Duration::from_millis(100);


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

    return (
        if can_calculate_10s {
            let length = rolling_frame_info_10s.len() as u128;

            let total: (u64, Duration, Duration) =  (
                rolling_frame_info_10s.into_iter().map(|f| { f.1 }).sum()
                , rolling_frame_info_10s.into_iter().map(|f| { f.2 }).sum()
                , rolling_frame_info_10s.into_iter().map(|f| { f.3 }).sum()
            );

            let average: (u64, Duration, Duration) = (
                total.0 / length as u64
                , Duration::from_nanos((total.1.as_nanos() / length) as u64)
                , Duration::from_nanos((total.2.as_nanos() / length) as u64)
            );

            let mut worst = (Duration::from_millis(0), Duration::from_millis(0));
            rolling_frame_info_10s
            .into_iter()
            .map(|f| {if f.2 > worst.0 {worst = (f.2, f.3);}})
            .max().unwrap();

            Some( ( average, worst ) )
        } else {None},
        if can_calculate_1s {
            let length = rolling_frame_info_1s.len() as u128;

            let total: (u64, Duration, Duration) =  (
                rolling_frame_info_1s.into_iter().map(|f| { f.1 }).sum()
                , rolling_frame_info_1s.into_iter().map(|f| { f.2 }).sum()
                , rolling_frame_info_1s.into_iter().map(|f| { f.3 }).sum()
            );

            let average: (u64, Duration, Duration) = (
                total.0 / length as u64
                , Duration::from_nanos((total.1.as_nanos() / length) as u64)
                , Duration::from_nanos((total.2.as_nanos() / length) as u64)
            );

            let mut worst = (Duration::from_millis(0), Duration::from_millis(0));
            rolling_frame_info_1s
            .into_iter()
            .map(|f| {if f.2 > worst.0 {worst = (f.2, f.3);}})
            .max().unwrap();

            Some( ( average, worst ) )
        } else {None},
        if can_calculate_100ms {
            let length = rolling_frame_info_100ms.len() as u128;

            let total: (u64, Duration, Duration) =  (
                rolling_frame_info_100ms.into_iter().map(|f| { f.1 }).sum()
                , rolling_frame_info_100ms.into_iter().map(|f| { f.2 }).sum()
                , rolling_frame_info_100ms.into_iter().map(|f| { f.3 }).sum()
            );

            let average: (u64, Duration, Duration) = (
                total.0 / length as u64
                , Duration::from_nanos((total.1.as_nanos() / length) as u64)
                , Duration::from_nanos((total.2.as_nanos() / length) as u64)
            );

            let mut worst = (Duration::from_millis(0), Duration::from_millis(0));
            rolling_frame_info_100ms
            .into_iter()
            .map(|f| {if f.2 > worst.0 {worst = (f.2, f.3);}})
            .max().unwrap();

            Some( ( average, worst ) )
        } else {None},
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.depth.gear-hud+1]
    #[test]
    fn pps_counter_counts_completions_not_wip() {
        let mut c = RateCounter::default();
        let t0 = Instant::now();
        c.record(10, t0);
        c.record(0, t0); // WIP / empty must not inflate
        assert!((c.rate(t0) - 10.0).abs() < 1e-9);
    }

    // r[verify cz.depth.gear-hud+1]
    #[test]
    fn hud_telemetry_carries_gear_and_rates() {
        use crate::assemblies::structs::ViewHud;
        use crate::delta_gear::ComputeGear;
        let hud = ViewHud {
            gear: ComputeGear::ScaledF64,
            points_delta: 3,
            iterations_delta: 1000,
        };
        assert_eq!(hud.gear.hud_label(), "S-F64");
        let mut pps = RateCounter::default();
        let mut ips = RateCounter::default();
        let now = Instant::now();
        pps.record(hud.points_delta, now);
        ips.record(hud.iterations_delta, now);
        assert!((pps.rate(now) - 3.0).abs() < 1e-9);
        assert!((ips.rate(now) - 1000.0).abs() < 1e-9);
    }
}