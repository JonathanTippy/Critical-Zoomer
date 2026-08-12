use eframe::epaint::Color32;
use steady_state::*;
use std::time::{Duration, Instant};

use crate::assemblies::headgroup::window::*;
use crate::assemblies::workgroup::screen_worker::workshift::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::workgroup::screen_worker::*;
use crate::constants::*;
use crate::constants::*;

use crate::assemblies::structs::*;
use crate::settings::Settings;

use rand::prelude::SliceRandom;
use crate::utils::*;


#[derive(Clone, Debug)]

pub struct ResultsPackage<T> {
    pub results: Vec<CompletedPoint<T>>
    , pub screen_res: (u32, u32)
    , pub location: ObjectivePosAndZoom
    // r[impl cz.depth.gear-hud+2]
    , pub hud: crate::assemblies::structs::ViewHud
}

pub struct WorkCollectorState<T> {
    completed_work: Option<ResultsPackage<T>>
    , surrounding_work: Option<ResultsPackage<T>>
    // Controller emission Instant received on a WorkUpdate, held until the next publish put.
    , pending_controller_emitted_at: Option<Instant>
}


pub const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub const WORKER_INIT_LOC:(f64, f64) = (0.0, 0.0);
pub const WORKER_INIT_ZOOM_POT: i64 = -2;
pub const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub const PIXELS_PER_UNIT_POT:i32 = 9;
pub const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

fn answer_from_completed(x: CompletedPoint<f64>) -> Answer {
    match x {
        CompletedPoint::Escapes {
            escape_time,
            escape_location,
            escape_derivative,
            smallness,
            small_time,
            ..
        } => Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: escape_time as u64,
                escape_z: (escape_location.0 as f32, escape_location.1 as f32),
                escape_dc: (escape_derivative.0 as f32, escape_derivative.1 as f32),
            },
            min_magnitude_time: small_time as u64,
            min_magnitude: smallness,
        },
        CompletedPoint::Repeats {
            period,
            smallness,
            small_time,
        } => Answer {
            result: MandelbrotResult::Inside {
                period: period as u64,
            },
            min_magnitude_time: small_time as u64,
            min_magnitude: smallness,
        },
        CompletedPoint::Dummy {} => Answer {
            result: MandelbrotResult::Inside { period: 0 },
            min_magnitude_time: 0,
            min_magnitude: 0.0,
        },
    }
}

fn view_from_package(package: &ResultsPackage<f64>, ctrl_emit: Option<Instant>) -> View<Answer> {
    let mut hud = package.hud;
    hud.controller_emitted_at = ctrl_emit;
    hud.publisher_emitted_at = Some(Instant::now());
    View {
        stencil: PointStencil {
            location: (
                package.location.pos.0.clone(),
                IntExp::ZERO - package.location.pos.1.clone(),
                package.location.zoom_pot,
            ),
            resolution: (
                package.screen_res.0 as usize,
                package.screen_res.1 as usize,
            ),
            serial_number: 0,
        },
        data: package
            .results
            .iter()
            .cloned()
            .map(answer_from_completed)
            .collect(),
        bitmap: vec![0; package.results.len()],
        hud,
    }
}

/// Content beat is due when the shared period has elapsed — independent of
/// whether new work arrived. Shade always gets the resident package.
// r[impl cz.craft.content-beat-publish+1]
pub(crate) fn content_beat_due(last_publish: Instant, period: Duration, now: Instant) -> bool {
    now.duration_since(last_publish) >= period
}

/// Fold one WorkUpdate into collector state. Collector never drain-to-newest.
// r[impl cz.craft.collector-absorbs-all+1]
pub(crate) fn absorb_work_update(state: &mut WorkCollectorState<f64>, u: WorkUpdate<f64>) {
    if let Some(at) = u.controller_emitted_at {
        state.pending_controller_emitted_at = Some(at);
    }

    if let Some(surrounding_work) = &mut state.surrounding_work {
        if let Some(mut f) = u.frame_info.clone() {
            f.0.zoom_pot -= 1;
            *surrounding_work = sample_old_values(surrounding_work, f.0, f.1);
        }
    }

    if let Some(completed_work) = &mut state.completed_work {
        if let Some(f) = u.frame_info {
            *completed_work = sample_old_values(completed_work, f.0, f.1);
            completed_work.hud = crate::assemblies::structs::ViewHud {
                stack: u.host_stack,
                mode: u.kernel_mode,
                reference: u.reference_status,
                gear: u.active_gear,
                points_delta: 0,
                iterations_delta: u.iterations_delta,
                packages_dropped: 0,
                color: crate::assemblies::structs::ColorerHud::Og,
                ..Default::default()
            };
        } else {
            let l = u.completed_points.len();
            let vs = u.completed_points;
            for i in 0..l {
                let w = vs[i].clone();
                completed_work.results[w.1] = w.0;
            }
            completed_work.hud = crate::assemblies::structs::ViewHud {
                stack: u.host_stack,
                mode: u.kernel_mode,
                reference: u.reference_status,
                gear: u.active_gear,
                points_delta: l as u64,
                iterations_delta: u.iterations_delta,
                packages_dropped: 0,
                color: crate::assemblies::structs::ColorerHud::Og,
                ..Default::default()
            };
        }
    } else {
        let f = u
            .frame_info
            .expect("work collector recieved an initial work update without any info");
        let mut package = ResultsPackage {
            results: vec![CompletedPoint::Dummy {}; (f.1 .0 * f.1 .1) as usize],
            screen_res: f.1,
            location: f.0,
            hud: crate::assemblies::structs::ViewHud {
                stack: u.host_stack,
                mode: u.kernel_mode,
                reference: u.reference_status,
                gear: u.active_gear,
                points_delta: u.completed_points.len() as u64,
                iterations_delta: u.iterations_delta,
                packages_dropped: 0,
                color: crate::assemblies::structs::ColorerHud::Og,
                ..Default::default()
            },
        };
        let l = u.completed_points.len();
        let vs = u.completed_points;
        for i in 0..l {
            let w = vs[i].clone();
            package.results[w.1] = w.0;
        }
        state.completed_work = Some(package);
    }
}

pub async fn run(
    actor: SteadyActorShadow,
    from_worker: SteadyRx<WorkUpdate<f64>>,
    answers_out: SteadyTx<View<Answer>>,
    settings_in: SteadyRx<Settings>,
    state: SteadyState<WorkCollectorState<f64>>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&from_worker, &settings_in], [&answers_out]),
        from_worker,
        answers_out,
        settings_in,
        state,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    from_worker: SteadyRx<WorkUpdate<f64>>,
    answers_out: SteadyTx<View<Answer>>,
    settings_in: SteadyRx<Settings>,
    state: SteadyState<WorkCollectorState<f64>>,
) -> Result<(), Box<dyn Error>> {

    let mut values_out = answers_out.lock().await;
    let mut from_worker = from_worker.lock().await;
    let mut settings_in = settings_in.lock().await;

    let mut state = state.lock(|| WorkCollectorState {
        completed_work: None
        , surrounding_work: None
        , pending_controller_emitted_at: None
    }).await;

    let mut content_period = Settings::DEFAULT.resolved_content_period();
    let mut last_publish = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);

    while actor.is_running(
        || i!(values_out.mark_closed())
    ) {

        await_for_any!(
            actor.wait_periodic(content_period),
            actor.wait_avail(&mut from_worker, 1),
            actor.wait_avail(&mut settings_in, 1),
        );

        while actor.avail_units(&mut settings_in) > 0 {
            if let Some(s) = actor.try_take(&mut settings_in) {
                content_period = s.resolved_content_period();
            }
        }

        while actor.avail_units(&mut from_worker) > 0 {
            let u = actor.try_take(&mut from_worker).expect("work update seemed available but wasn't...");
            absorb_work_update(&mut state, u);
        }

        // Content beat: always publish resident work-so-far (shade always runs).
        // r[impl cz.craft.content-beat-publish+1]
        if content_beat_due(last_publish, content_period, Instant::now()) {
            if let Some(package) = state.completed_work.clone() {
                if !actor.is_full(&mut values_out) {
                    let mut ctrl_emit = state.pending_controller_emitted_at.take();
                    let view = view_from_package(&package, ctrl_emit.take());
                    match actor.try_send(&mut values_out, view) {
                        SendOutcome::Success => {
                            last_publish = Instant::now();
                        }
                        SendOutcome::Blocked(v)
                        | SendOutcome::Timeout(v)
                        | SendOutcome::Closed(v) => {
                            state.pending_controller_emitted_at = v.hud.controller_emitted_at;
                        }
                    }
                }
            }
        }
    }
    info!("Computer shutting down.");
    Ok(())
}



// r[impl cz.craft.clamped-remap-smear+1]
// r[impl cz.craft.shared-remap-transform+1]
pub(crate) fn sample_old_values<T:Clone>(old_package: &ResultsPackage<T>, new_location: ObjectivePosAndZoom, new_res: (u32, u32)) -> ResultsPackage<T> {
    let mut returned = ResultsPackage{
        results: vec!()
        , screen_res: new_res
        , location: new_location.clone()
        , hud: old_package.hud
    };

    let old_size = old_package.screen_res.0 * old_package.screen_res.1;

    //let old_package_pixel_width = old_package.location.zoom_pot

    let relative_pos = (
        old_package.location.pos.0.clone()-new_location.pos.0.clone()
        , old_package.location.pos.1.clone()-new_location.pos.1.clone()
    );

    let relative_pos_in_pixels:(i32, i32) = (
        relative_pos.0.clone().shift(new_location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
        , relative_pos.1.clone().shift(new_location.zoom_pot).shift(PIXELS_PER_UNIT_POT).into()
    );

    let relative_zoom = new_location.zoom_pot - old_package.location.zoom_pot;

    /*let relative_pos_in_pixels = (
        relative_pos_in_pixels.0 - shift(1, relative_zoom-1)
        , relative_pos_in_pixels.1 - shift(1, relative_zoom-1)
    );*/

    for row in 0..new_res.1 as usize {
        for seat in 0..new_res.0 as usize {
            returned.results.push(
                sample_value(
                    &old_package.results
                    , old_package.screen_res
                    , old_size as usize
                    , row
                    , seat
                    , relative_pos_in_pixels
                    , relative_zoom as i64
                )
            );
            //i+=1;
        }
    }
    returned
}


/*fn get_values_from_points(ps: Vec<(CompletedPoint, usize)>) -> Vec<(CompletedPoint, usize)> {
    let mut returned = vec!();
    for p in ps {
        returned.push(((p.0), p.1));
    }
    returned
}*/





fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();

    let mut indices: Vec<usize> = (0..size).collect();

    // Shuffle indices randomly
    indices.shuffle(&mut rng);
    indices
}




#[inline]
fn sample_value<T: Clone>(
    pixels: &Vec<CompletedPoint<T>>
    , data_res: (u32, u32)
    , data_len: usize
    , row: usize
    , seat: usize
    , relative_pos: (i32, i32)
    , relative_zoom_pot: i64
) -> CompletedPoint<T> {
    let color =
        pixels[
            index_from_relative_location(
                transform_relative_location_i32(
                    relative_location_i32_row_and_seat(seat, row)
                    , (relative_pos.0, relative_pos.1)
                    , relative_zoom_pot
                )
                , data_res
                , data_len
            )
            ].clone();
    color
}

#[cfg(test)]
mod mutant_kill {
    use super::*;

    /// Thought-killed pins for shared remap: identity + pan offset sampling.
    #[test]
    fn mutant_kill_sample_old_values_identity_and_offset() {
        let res = (2u32, 2u32);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: -2,
        };
        let results = vec![
            CompletedPoint::Escapes {
                escape_time: 1,
                escape_location: (0.0f64, 0.0),
                escape_derivative: (1.0, 0.0),
                start_location: (0.0, 0.0),
                smallness: 0.0,
                small_time: 0,
            },
            CompletedPoint::Escapes {
                escape_time: 2,
                escape_location: (0.0, 0.0),
                escape_derivative: (1.0, 0.0),
                start_location: (0.0, 0.0),
                smallness: 0.0,
                small_time: 0,
            },
            CompletedPoint::Escapes {
                escape_time: 3,
                escape_location: (0.0, 0.0),
                escape_derivative: (1.0, 0.0),
                start_location: (0.0, 0.0),
                smallness: 0.0,
                small_time: 0,
            },
            CompletedPoint::Escapes {
                escape_time: 4,
                escape_location: (0.0, 0.0),
                escape_derivative: (1.0, 0.0),
                start_location: (0.0, 0.0),
                smallness: 0.0,
                small_time: 0,
            },
        ];
        let pkg = ResultsPackage {
            results,
            screen_res: res,
            location: loc.clone(),
            hud: Default::default(),
        };
        let same = sample_old_values(&pkg, loc.clone(), res);
        for (i, r) in same.results.iter().enumerate() {
            match r {
                CompletedPoint::Escapes { escape_time, .. } => {
                    assert_eq!(*escape_time, (i as u32) + 1)
                }
                other => panic!("kind changed: {other:?}"),
            }
        }
        // relative_zoom = new - old; swapped would invert zoom direction.
        assert_eq!(same.location.zoom_pot, loc.zoom_pot);
        assert_eq!(same.screen_res, res);

        // Direct sample_value: seat/row order must stay row-major via helpers.
        let v = sample_value(
            &pkg.results,
            res,
            4,
            1,
            0,
            (0, 0),
            0,
        );
        match v {
            CompletedPoint::Escapes { escape_time, .. } => assert_eq!(escape_time, 3),
            other => panic!("{other:?}"),
        }
        let v01 = sample_value(&pkg.results, res, 4, 0, 1, (0, 0), 0);
        match v01 {
            CompletedPoint::Escapes { escape_time, .. } => assert_eq!(escape_time, 2),
            other => panic!("{other:?}"),
        }
    }

    // r[verify cz.craft.content-beat-publish+1]
    #[test]
    fn content_beat_due_without_new_work() {
        let period = Duration::from_millis(16);
        let t0 = Instant::now();
        assert!(!content_beat_due(t0, period, t0));
        assert!(content_beat_due(
            t0,
            period,
            t0 + period + Duration::from_millis(1)
        ));
        // Still due even if no WorkUpdate arrived in between — shade keeps running.
        assert!(content_beat_due(t0, period, t0 + period * 2));
    }

    fn seat_escape(time: u32) -> CompletedPoint<f64> {
        CompletedPoint::Escapes {
            escape_time: time,
            escape_location: (0.0, 0.0),
            escape_derivative: (1.0, 0.0),
            start_location: (0.0, 0.0),
            smallness: 0.0,
            small_time: 0,
        }
    }

    fn bare_update(
        frame: Option<(ObjectivePosAndZoom, (u32, u32))>,
        points: Vec<(CompletedPoint<f64>, usize)>,
    ) -> WorkUpdate<f64> {
        WorkUpdate {
            frame_info: frame,
            completed_points: points,
            active_gear: crate::delta_gear::ComputeGear::F64,
            host_stack: crate::assemblies::structs::HostStack::F64,
            kernel_mode: crate::assemblies::structs::KernelMode::Naive,
            reference_status: crate::assemblies::structs::ReferenceStatus::Wip,
            iterations_delta: 0,
            controller_emitted_at: None,
        }
    }

    // r[verify cz.craft.collector-absorbs-all+1]
    #[test]
    fn collector_absorbs_all_seat_updates() {
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: -2,
        };
        let res = (2u32, 2u32);
        let mut state = WorkCollectorState {
            completed_work: None,
            surrounding_work: None,
            pending_controller_emitted_at: None,
        };
        absorb_work_update(
            &mut state,
            bare_update(Some((loc.clone(), res)), vec![(seat_escape(10), 0)]),
        );
        absorb_work_update(
            &mut state,
            bare_update(None, vec![(seat_escape(20), 1)]),
        );
        absorb_work_update(
            &mut state,
            bare_update(None, vec![(seat_escape(30), 2), (seat_escape(40), 3)]),
        );
        let pkg = state.completed_work.expect("package");
        for (i, expect) in [10u32, 20, 30, 40].into_iter().enumerate() {
            match &pkg.results[i] {
                CompletedPoint::Escapes { escape_time, .. } => assert_eq!(*escape_time, expect),
                other => panic!("seat {i}: {other:?}"),
            }
        }
    }
}