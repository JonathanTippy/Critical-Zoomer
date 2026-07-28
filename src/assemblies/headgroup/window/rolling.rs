use std::time::{Duration, Instant};
use std::collections::*;
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
/// Rolling 1-second counter of completed tiles ingested into the headgroup hoard.
/// Standards HUD TPS — not workgroup emit rate.
#[derive(Default, Debug, Clone)]
pub struct TpsCounter {
    events: VecDeque<Instant>,
}

impl TpsCounter {
    pub fn new() -> Self {
        Self { events: VecDeque::new() }
    }

    /// Record `n` newly completed tiles ingested this turn.
    pub fn record(&mut self, n: usize, now: Instant) {
        for _ in 0..n {
            self.events.push_front(now);
        }
        self.prune(now);
    }

    fn prune(&mut self, now: Instant) {
        while let Some(back) = self.events.back() {
            if now.duration_since(*back) > Duration::from_secs(1) {
                self.events.pop_back();
            } else {
                break;
            }
        }
    }

    /// Completed tiles in the last 1s window.
    pub fn tps(&mut self, now: Instant) -> f64 {
        self.prune(now);
        self.events.len() as f64
    }
}

#[cfg(test)]
mod tps_tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        let mut c = TpsCounter::new();
        assert_eq!(c.tps(Instant::now()), 0.0);
    }

    #[test]
    fn records_count_within_window() {
        let mut c = TpsCounter::new();
        let t0 = Instant::now();
        c.record(5, t0);
        assert_eq!(c.tps(t0), 5.0);
    }

    #[test]
    fn prunes_events_older_than_one_second() {
        let mut c = TpsCounter::new();
        let t0 = Instant::now();
        c.events.push_front(t0 - Duration::from_millis(1500));
        c.record(2, t0);
        assert_eq!(c.tps(t0), 2.0);
    }
}
