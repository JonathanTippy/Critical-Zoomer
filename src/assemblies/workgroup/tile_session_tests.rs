// read delivery.md for project context
// Extracted from tile_session.rs — unit/integration tests for TileSession.
use super::*;

mod perturbation_always_on_tests {
    use super::*;
    use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::naive_cpu_worker::{
        iterate_point_bout
        , point_to_answer as naive_point_to_answer
    };
    use proptest::prelude::*;

    fn naive_finish(c: (f64, f64), max_iters: u32) -> Answer {
        let z = (0.0, 0.0);
        let derivative = (1.0, 0.0);
        let mut point = ActivePoint {
            c
            , z
            , derivative
            , real_squared: 0.0
            , imag_squared: 0.0
            , real_imag: 0.0
            , iteration_count: 0
            , min_magnitude: f64::MAX
            , min_magnitude_time: 0
            , periodicity_detector: CpuPeriodicityDetector::init(0, z, derivative)
            , escaped: false
            , finished: false
            , orbit_id: ZERO_ORBIT_ID
            , seat_linear: 0
        };
        let epsilon = 1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6);
        let mut left = max_iters;
        while !point.finished && left > 0 {
            let bout = left.min(1000);
            iterate_point_bout(&mut point, 4.0, epsilon, bout);
            left = left.saturating_sub(bout);
        }
        naive_point_to_answer(&point)
    }

    fn same_membership(a: &Answer, b: &Answer) -> bool {
        match (&a.result, &b.result) {
            (
                MandelbrotResult::Outside { escape_time_r2: ea, .. }
                , MandelbrotResult::Outside { escape_time_r2: eb, .. }
            ) => ea == eb
            , (MandelbrotResult::Inside { .. }, MandelbrotResult::Inside { .. }) => true
            , _ => false
        }
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn new_session_seeds_nonzero_reference_orbit_at_period_two_corner() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO)
            , zoom_pot: 4
        };
        let session = TileSession::new(location, (8, 8));
        assert!(
            session.worker_state.references.len() > 1
            , "expected a nonzero reference orbit seeded from the period-2 nucleus at the stencil corner"
        );
        let orbit_id = session.worker_state.seat_orbit_ids[0];
        assert_ne!(orbit_id, ZERO_ORBIT_ID);
        assert!(
            session.worker_state.seat_orbit_ids.iter().all(|&id| id == orbit_id)
            , "every seat must start bound to the same seeded reference orbit: \
               perturbation is always-on for the whole screen, not opt-in per seat"
        );
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn new_session_falls_back_to_zero_orbit_when_corner_has_no_nucleus() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(3), IntExp::ZERO)
            , zoom_pot: 0
        };
        let session = TileSession::new(location, (4, 4));
        assert_eq!(session.worker_state.references.len(), 1);
        assert!(
            session.worker_state.seat_orbit_ids.iter().all(|&id| id == ZERO_ORBIT_ID)
            , "with no nucleus at the corner, every seat still goes through the \
               perturbation worker, just bound to the immortal zero orbit"
        );
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn live_workshift_perturbation_matches_naive_near_period_two_nucleus() {
        // TileSession embeds a full GPU_WORKER_BATCH_N-sized PointBatch directly
        // (not boxed) whenever a tile or scredge batch is open, matching the
        // real actor's stack budget (main.rs sizes actor stacks at 100MiB via
        // with_default_actor_stack_size). Run this on a thread with a comparable
        // stack instead of the default ~2MiB test-thread stack.
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(live_workshift_matches_naive_body)
            .expect("spawn test thread");
        handle.join().expect("live workshift parity test thread panicked");
    }

    fn live_workshift_matches_naive_body() {
        // Corner chosen with a nonzero (but tiny) imaginary part on purpose:
        // PointStencil's own f64-availability check (type_contains_all_points)
        // degenerates whenever a homothety coordinate is exactly zero (adding
        // then subtracting zero trivially "loses no precision"), which is a
        // pre-existing quirk of docs/design geometry unrelated to perturbation.
        // Keeping the offset tiny (but at/above this stencil's own pixel
        // precision, so it survives PointStencil::correct_precision instead
        // of rounding back to zero) still lands essentially on the period-2
        // nucleus at c=-1, so period detection converges in a handful of
        // iterations instead of the slow brute-force search that happens
        // well inside (but off-center of) a hyperbolic component.
        let im = IntExp::from(1).shift(-8);
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO - im)
            , zoom_pot: 2
        };
        let res: (usize, usize) = (6, 6);
        let mut session = TileSession::new(location, (res.0 as u32, res.1 as u32));
        // Periodicity for this fixture is CPU-side; skip GPU bout round-trips so
        // the parity check stays interactive. Preference still selected at new().
        session.worker_state.use_cpu_bouts_only();
        assert_ne!(
            session.worker_state.seat_orbit_ids[0]
            , ZERO_ORBIT_ID
            , "fixture must exercise a real (nonzero) reference orbit, not the zero-orbit fallback"
        );
        let mut guard = 0;
        while session.percent_completed() < 100.0 {
            session.workshift();
            guard += 1;
            assert!(guard < 500, "live session did not complete work in time");
        }
        let generator = session.stencil.get_c_generator::<f64>().expect("f64 c generator");
        for y in 0..res.1 {
            for x in 0..res.0 {
                let idx = y * res.0 + x;
                let calibrated = session.screen_answer[idx]
                    .expect("every seat must be finished once the session reports 100%");
                let live_answer = calibrated_to_answer(calibrated);
                let c = generator.get_c((x as u16, y as u16));
                let naive_answer = naive_finish(c, 100_000);
                assert!(
                    same_membership(&live_answer, &naive_answer)
                    , "live perturbation path disagrees with naive iteration at seat ({x},{y}) c={c:?}: \
                       live={live_answer:?} naive={naive_answer:?}"
                );
            }
        }
    }

    /// Home-sized screen must fill most seats under stationary mag_velocity (no headed UI).
    #[test]
    fn home_screen_session_fills_majority_of_seats() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                    , zoom_pot: HOME_POSITION.2
                };
                let mut session = TileSession::new(location, (128, 96));
                session.worker_state.use_cpu_bouts_only();
                session.set_mag_velocity(0);
                let mut guard = 0u32;
                let mut last = -1.0;
                let mut stall = 0u32;
                while session.percent_completed() < 95.0 && guard < 5_000 {
                    session.workshift();
                    guard += 1;
                    let p = session.percent_completed();
                    if (p - last).abs() < 1e-9 {
                        stall += 1;
                        if stall > 200 {
                            panic!(
                                "home session stalled at {p}% after {guard} workshifts \
                                 (active={} lookahead={})"
                                , session.active_tile.is_some()
                                , session.lookahead_work.is_some()
                            );
                        }
                    } else {
                        stall = 0;
                        last = p;
                    }
                }
                assert!(
                    session.percent_completed() >= 95.0
                    , "home fill only reached {}% in {guard} workshifts"
                    , session.percent_completed()
                );
            })
            .expect("spawn");
        handle.join().expect("home fill thread panicked");
    }

    /// Full window home fill must reach 95% within the product &lt;5s bar (CPU bouts).
    /// Release-only: the bar is a product timing gate (debug is too slow).
    // r[verify cz.perf.home-100tps+1]
    // r[verify cz.e2e.perf-home-fill+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn home_800x480_fills_within_five_seconds_cpu() {
        let handle = std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                    , zoom_pot: HOME_POSITION.2
                };
                let mut session = TileSession::new(location, (800, 480));
                session.worker_state.use_cpu_bouts_only();
                session.set_mag_velocity(0);
                let t0 = Instant::now();
                let mut guard = 0u32;
                while session.percent_completed() < 95.0 && t0.elapsed().as_millis() < 8_000 {
                    session.workshift();
                    guard += 1;
                }
                let ms = t0.elapsed().as_millis();
                eprintln!(
                    "home_800x480 fill {}% in {ms}ms ({guard} workshifts)"
                    , session.percent_completed()
                );
                assert!(
                    session.percent_completed() >= 95.0
                    , "only reached {}% in {ms}ms"
                    , session.percent_completed()
                );
                assert!(
                    ms <= 5000
                    , "home 800x480 fill took {ms}ms (>5000); need faster CPU fill path"
                );
                // Standards TPS addendum: completed *whole* tiles / s ≥ 100 on CPU.
                let whole = session
                    .answer_tiles
                    .values()
                    .filter(|t| {
                        t.data.iter().filter(|c| c.is_some()).count() == TILE_SEAT_COUNT
                    })
                    .count() as f64;
                let secs = (ms as f64 / 1000.0).max(1e-3);
                let tps = whole / secs;
                eprintln!("home_fill_completed_whole_tps≈{tps:.1} whole={whole} ms={ms}");
                assert!(
                    tps >= 100.0
                    , "home CPU completed-whole TPS {tps:.1} < 100 (whole={whole}, ms={ms})"
                );
                // Continue to completion and report answer-tile quality by x-band.
                while session.percent_completed() < 100.0 && t0.elapsed().as_millis() < 20_000 {
                    session.workshift();
                }
                let mut nores_by_band = [0u32; 8];
                let mut out_by_band = [0u32; 8];
                let mut none_by_band = [0u32; 8];
                let mut inside_by_band = [0u32; 8];
                for y in 0..480usize {
                    for x in 0..800usize {
                        let band = (x * 8 / 800).min(7);
                        let origin = tile_origin_for_seat((x, y), (800, 480));
                        let local = (x - origin.0, y - origin.1);
                        let Some(tile) = session.answer_tiles.get(&origin) else {
                            none_by_band[band] += 1;
                            continue;
                        };
                        let Some(a) = tile.get(local) else {
                            none_by_band[band] += 1;
                            continue;
                        };
                        match a.result {
                            MandelbrotResult::Inside { .. } => inside_by_band[band] += 1,
                            MandelbrotResult::Outside { escape_time_r2, .. } => {
                                if escape_time_r2 == 1 && a.min_magnitude.is_infinite() {
                                    nores_by_band[band] += 1;
                                } else {
                                    out_by_band[band] += 1;
                                }
                            }
                        }
                    }
                }
                eprintln!(
                    "answer_tiles={} seats={}%"
                    , session.answer_tiles.len()
                    , session.percent_completed()
                );
                for b in 0..8 {
                    eprintln!(
                        "band{b}: nores={} out={} inside={} none={}"
                        , nores_by_band[b]
                        , out_by_band[b]
                        , inside_by_band[b]
                        , none_by_band[b]
                    );
                }
            })
            .expect("spawn");
        handle.join().expect("800x480 home fill panicked");
    }

    /// Debug probe: preferred GPU path must stay near CPU fill pace (not ~10× slower).
    #[cfg_attr(coverage, ignore = "llvm-cov overhead; run without coverage")]
    #[cfg_attr(debug_assertions, ignore = "product fill bar requires --release")]
    #[test]
    fn home_800x480_fills_gpu_path_probe() {
        let handle = std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                    , zoom_pot: HOME_POSITION.2
                };
                let mut session = TileSession::new(location, (800, 480));
                // Prefer live GPU path (do not force CPU).
                session.set_mag_velocity(0);
                let t0 = Instant::now();
                let mut guard = 0u32;
                while session.gpu_resident_fill_percent() < 95.0 && t0.elapsed().as_millis() < 30_000 {
                    session.workshift();
                    guard += 1;
                }
                let ms = t0.elapsed().as_millis();
                eprintln!(
                    "home_800x480 GPU-path fill {}% in {ms}ms ({guard} workshifts) gpu_preferred={} gpu_held={}"
                    , session.gpu_resident_fill_percent()
                    , session.worker_state.is_gpu_preferred()
                    , session.worker_state.gpu_device_held()
                );
                assert!(
                    session.gpu_resident_fill_percent() >= 95.0
                    , "GPU path only reached {}% in {ms}ms"
                    , session.gpu_resident_fill_percent()
                );
                // Standards TPS addendum: headgroup-shaped GPU-resident whole tiles / s ≥ 3000.
                let whole = session.headgroup_completed_whole_tiles() as f64;
                let fill_pct = session.gpu_resident_fill_percent();
                let secs = (ms as f64 / 1000.0).max(1e-3);
                let tps = whole / secs;
                eprintln!(
                    "home_gpu_headgroup_whole_tps≈{tps:.1} whole={whole} ms={ms} gpu_held={} fill={fill_pct:.1}% host_fill={:.1}%"
                    , session.worker_state.gpu_device_held()
                    , session.percent_completed()
                );
                assert!(
                    session.worker_state.gpu_device_held()
                    , "GPU path probe must hold a device (lazy-init); preferred={} held={}"
                    , session.worker_state.is_gpu_preferred()
                    , session.worker_state.gpu_device_held()
                );
                assert!(
                    fill_pct >= 95.0
                    , "GPU-resident fill only reached {fill_pct:.1}% in {ms}ms (host {}%)"
                    , session.percent_completed()
                );
                assert!(
                    tps >= 3_000.0
                    , "home GPU headgroup whole-TPS {tps:.1} < 3000 (whole={whole}, ms={ms})"
                );
            })
            .expect("spawn");
        handle.join().expect("800x480 GPU home fill panicked");
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn workshift_starts_lookahead_at_deeper_mag() {
        // LookaheadWork embeds large outfill state; use a fat stack like the
        // live workshift parity test.
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 2
                };
                let mut session = TileSession::new(location, (64, 64));
                session.force_cpu_bouts_for_test();
                session.set_mag_velocity(1);
                for _ in 0..128 {
                    session.workshift_budget_ms(32);
                    if session.lookahead_work.is_some() {
                        break;
                    }
                }
                assert!(
                    session.lookahead_work.is_some()
                    , "zoom-in should begin the DFS lookahead column under attention"
                );
                let zoom = session.lookahead_work.as_ref().unwrap().publish_location.zoom_pot;
                assert_eq!(zoom, 3, "first column bump is base_mag+1");
            })
            .expect("spawn");
        handle.join().expect("lookahead start test panicked");
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn attention_tile_location_contains_attention_seat() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO)
            , zoom_pot: 3
        };
        let attention = (40, 20);
        let deeper = attention_tile_location(&location, attention, 5).expect("loc");
        assert_eq!(deeper.zoom_pot, 5);
        let att = (
            location.pos.0.clone()
                + IntExp::from(attention.0).shift(-(location.zoom_pot + PIXELS_PER_UNIT_POT))
            , location.pos.1.clone()
                + IntExp::from(attention.1).shift(-(location.zoom_pot + PIXELS_PER_UNIT_POT))
        );
        let wx = intexp_seat_i32(att.0.shift(5).shift(PIXELS_PER_UNIT_POT)).unwrap();
        let wy = intexp_seat_i32(att.1.shift(5).shift(PIXELS_PER_UNIT_POT)).unwrap();
        let edge = TILE_EDGE_LENGTH as i32;
        let ox = floor_div_tile_i32(wx) * edge;
        let oy = floor_div_tile_i32(wy) * edge;
        let ulx = intexp_seat_i32(
            deeper.pos.0.clone().shift(5).shift(PIXELS_PER_UNIT_POT)
        ).unwrap();
        let uly = intexp_seat_i32(
            deeper.pos.1.clone().shift(5).shift(PIXELS_PER_UNIT_POT)
        ).unwrap();
        assert_eq!((ulx, uly), (ox, oy));
        assert!(wx >= ox && wx < ox + edge);
        assert!(wy >= oy && wy < oy + edge);
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]
        #[test]
        fn lookahead_publish_location_zoom_matches_requested_mag(
            base_zoom in -4i32..8,
            bump in 1i32..8,
            att_x in 0i32..64,
            att_y in 0i32..64,
        ) {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO)
                , zoom_pot: base_zoom
            };
            let target = base_zoom + bump;
            let Some(pub_loc) = attention_tile_location(&location, (att_x, att_y), target) else {
                return Ok(());
            };
            prop_assert_eq!(pub_loc.zoom_pot, target);
        }
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn retarget_pan_keeps_bound_reference_orbit() {
        let location = ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO)
            , zoom_pot: 4
        };
        let mut session = TileSession::new(location.clone(), (8, 8));
        let orbit_before = session.worker_state.seat_orbit_ids[0];
        let refs_before = session.worker_state.references.len();
        assert_ne!(orbit_before, ZERO_ORBIT_ID);
        let panned = ObjectivePosAndZoom {
            pos: (
                location.pos.0.clone() + IntExp::from(1).shift(-(4 + PIXELS_PER_UNIT_POT))
                , location.pos.1.clone()
            )
            , zoom_pot: 4
        };
        session.retarget(panned, (8, 8));
        assert_eq!(session.worker_state.seat_orbit_ids[0], orbit_before);
        assert_eq!(session.worker_state.references.len(), refs_before);
        assert_eq!(session.seats_done, 0);
    }

    // r[verify cz.seamless.reference-background+1]
    #[test]
    fn retarget_zoom_rebuilds_session() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 4
                };
                let mut session = TileSession::new(location, (8, 8));
                session.seats_done = 3;
                let refs_before = session.worker_state.references.len();
                session.retarget(
                    ObjectivePosAndZoom {
                        pos: (IntExp::from(-1), IntExp::ZERO)
                        , zoom_pot: 5
                    }
                    , (8, 8)
                );
                session.set_mag_velocity(1);
                assert_eq!(session.location.zoom_pot, 5);
                assert_eq!(session.seats_done, 0);
                assert_eq!(session.tile_scheduler.mag_velocity, 1);
                assert_eq!(session.reference_worker.bound_mag(), Some(5));
                // Mag-change path keeps the prior collection (old orbit retained).
                assert!(session.worker_state.references.len() >= refs_before);
            })
            .expect("spawn");
        handle.join().expect("retarget zoom test panicked");
    }

    // r[verify cz.int.stencil-retarget+1]
    #[test]
    fn retarget_same_mag_pan_keeps_progress() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                    , zoom_pot: 2
                };
                let mut session = TileSession::new(location.clone(), (64, 64));
                for i in 0..128 {
                    session.screen_done[i] = true;
                    session.screen_kind[i] = Some(SeatKind::Outside);
                }
                session.seats_done = 128;
                let before_pct = session.percent_completed();
                let panned = ObjectivePosAndZoom {
                    pos: (
                        location.pos.0.clone()
                            + IntExp::from(8).shift(-(2 + PIXELS_PER_UNIT_POT))
                        , location.pos.1.clone()
                    )
                    , zoom_pot: 2
                };
                session.retarget(panned, (64, 64));
                session.set_mag_velocity(0);
                assert!(
                    session.percent_completed() > 1.0
                    , "same-mag pan must keep progress (before={} after={})"
                    , before_pct
                    , session.percent_completed()
                );
            })
            .expect("spawn");
        handle.join().expect("same-mag pan test panicked");
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    #[test]
    fn lookahead_wip_enqueues_before_bump_completes() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 2
                };
                let mut session = TileSession::new(location, (64, 64));
                session.force_cpu_bouts_for_test();
                session.set_mag_velocity(1);
                let mut guard = 0u32;
                let mut saw_partial_wip = false;
                while guard < 800 {
                    session.workshift_budget_ms(32);
                    if session.has_open_lookahead() {
                        let wip = session.drain_lookahead_publishes();
                        if !wip.is_empty() {
                            assert!(
                                session.has_open_lookahead()
                                , "WIP publish must not wait for full 64² bump"
                            );
                            saw_partial_wip = true;
                            break;
                        }
                    }
                    guard += 1;
                }
                assert!(
                    saw_partial_wip
                    , "partial lookahead column must reach lookahead_unsent before bump completes"
                );
            })
            .expect("spawn");
        handle.join().expect("lookahead wip test panicked");
    }

    // r[verify cz.seamless.foveated-mag-velocity+1]
    // r[verify cz.int.stencil-retarget+1]
    #[test]
    fn mag_retarget_flushes_partial_lookahead_column() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO)
                    , zoom_pot: 2
                };
                let mut session = TileSession::new(location.clone(), (64, 64));
                session.force_cpu_bouts_for_test();
                session.set_mag_velocity(1);
                let mut guard = 0u32;
                while guard < 800 {
                    session.workshift_budget_ms(32);
                    if session.has_open_lookahead() {
                        let wip = session.drain_lookahead_publishes();
                        if !wip.is_empty() {
                            break;
                        }
                    }
                    guard += 1;
                }
                assert!(
                    session.has_open_lookahead()
                    , "precondition: need in-flight lookahead before mag retarget"
                );
                session.retarget(
                    ObjectivePosAndZoom {
                        pos: location.pos.clone()
                        , zoom_pot: 3
                    }
                    , (64, 64)
                );
                let flushed = session.drain_lookahead_publishes();
                assert!(
                    !flushed.is_empty()
                    , "mag retarget must flush partial column to lookahead_unsent"
                );
            })
            .expect("spawn");
        handle.join().expect("lookahead retarget flush test panicked");
    }

    // r[verify cz.perf.play-minimize+1]
    #[test]
    fn touch_play_visible_ignores_stale_lookahead() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO),
                    zoom_pot: 2,
                };
                let mut session = TileSession::new(location.clone(), (64, 64));
                session.force_cpu_bouts_for_test();
                session.set_mag_velocity(1);
                let mut guard = 0u32;
                while guard < 800 {
                    session.workshift_budget_ms(32);
                    if !session.drain_lookahead_publishes().is_empty() {
                        break;
                    }
                    guard += 1;
                }
                session.retarget(
                    ObjectivePosAndZoom {
                        pos: location.pos.clone(),
                        zoom_pot: 3,
                    },
                    (64, 64),
                );
                assert!(
                    session.has_unsent_publish(),
                    "precondition: lookahead preserved across mag retarget"
                );
                assert!(
                    !session.has_unsent_screen_publish_for_test(),
                    "precondition: zoom wiped current-stencil unsent"
                );
                session.set_mag_velocity(1);
                assert!(
                    session.play_need_visible_for_test(),
                    "stale lookahead must not clear play_need_visible"
                );
            })
            .expect("spawn");
        handle.join().expect("touch_play_visible lookahead test panicked");
    }
}

mod mag_depth_tests {
    use super::*;
    use crate::assemblies::headgroup::window::sampling::{SamplingContext, ZoomerCommand};
    use crate::assemblies::headgroup::window::transforms::transform;
    use crate::constants::HOME_POSITION;
    use crate::gear::Gear;

    fn zoom_from_home(center: (i32, i32), steps: usize) -> ObjectivePosAndZoom {
        let mut ctx = SamplingContext {
            tiles: Default::default(),
            tile_gpu_ids: Default::default(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 1,
            reset_gpu_tile_slots: false,
            proximate_answers: false,
            unsent_answers: false,
            screen_size: (800, 480),
            location: ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1)),
                zoom_pot: HOME_POSITION.2,
            },
            updated: false,
            mouse_drag_start: None,
            memory_limit_bytes: 256 * 1024 * 1024,
            last_memory_bump: None,
            handle_filled: Default::default(),
        };
        for _ in 0..steps {
            transform(
                vec![ZoomerCommand::Zoom {
                    pot: 1,
                    center_screenspace_pos: center,
                }],
                &mut ctx,
            );
        }
        ctx.location
    }

    #[test]
    fn reference_orbit_seeds_through_mag_twenty() {
        for mag in [10i32, 15, 18, 19, 20, 21] {
            let location = ObjectivePosAndZoom {
                pos: (IntExp::from(-1), IntExp::ZERO),
                zoom_pot: mag,
            };
            let session = TileSession::new(location, (4, 4));
            assert_ne!(
                session.bound_orbit_id_for_test(),
                ZERO_ORBIT_ID,
                "mag {mag}: corner nucleus must bind a reference orbit"
            );
        }
    }

    #[test]
    fn gear_selects_beyond_f64_when_discrimination_exceeds_mantissa() {
        let mag = 20i32;
        let stencil = PointStencil {
            homothety: (IntExp::from(-1), IntExp::ZERO, mag),
            resolution: (64, 64),
            serial_number: 0,
            focus: None,
            hover: None,
            mag_velocity: 0.0,
        }
        .correct_precision();
        let corner = (stencil.homothety.0.clone(), stencil.homothety.1.clone());
        assert_eq!(
            stencil.select_gear(Some(&corner), true),
            Gear::StackedI32 { limbs: 1 }
        );
        assert_eq!(
            stencil.select_gear(Some(&corner), false),
            Gear::F64
        );
    }

    #[test]
    fn workshift_completes_at_mag_twenty_on_period_two_nucleus() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let location = ObjectivePosAndZoom {
                    pos: (IntExp::from(-1), IntExp::ZERO),
                    zoom_pot: 20,
                };
                let mut session = TileSession::new(location, (6, 6));
                session.force_cpu_bouts_for_test();
                let mut guard = 0;
                while session.percent_completed() < 100.0 {
                    session.workshift();
                    guard += 1;
                    assert!(
                        guard < 800,
                        "mag 20: session stalled at {:.1}%",
                        session.percent_completed()
                    );
                }
            })
            .expect("spawn mag-20 workshift test");
        handle.join().expect("mag-20 workshift test panicked");
    }

    #[test]
    fn workshift_completes_after_twenty_zooms_from_home() {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let center = (400, 240);
                let location = zoom_from_home(center, 20);
                let mag = location.zoom_pot;
                let mut session = TileSession::new(location, (64, 64));
                session.force_cpu_bouts_for_test();
                let mut guard = 0;
                while session.percent_completed() < 100.0 {
                    session.workshift();
                    guard += 1;
                    assert!(
                        guard < 400,
                        "mag {mag}: stalled at {:.1}% after zoom-from-home",
                        session.percent_completed()
                    );
                }
            })
            .expect("spawn zoom-from-home test");
        handle.join().expect("zoom-from-home test panicked");
    }
}