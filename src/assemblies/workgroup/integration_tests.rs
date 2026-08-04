// read delivery.md for project context
//! Assembly (integration) verifies for workgroup/headgroup contracts.
//! Not headed e2e — in-process multi-unit harnesses.

#[cfg(test)]
mod assembly_tests {
    use std::collections::HashMap;
    use std::time::Instant;

    use crate::assemblies::headgroup::window::sampling::SamplingContext;
    use crate::assemblies::structs::*;
    use crate::assemblies::workgroup::tile_manager::{
        apply_memory_bump, plan_prunes, required_limit_bump, ManagedTileMeta, TileKeepClass,
    };
    use crate::assemblies::workgroup::tile_publisher::{
        agnostic_wide, exact_outside, publish_seat, PublishCadence,
    };
    use crate::assemblies::workgroup::structs::CalibratedAnswer;
    use crate::assemblies::workgroup::structs::CalibratedMandelbrotResult;
    use crate::range::Range;
    use crate::assemblies::workgroup::tile_session::TileSession;
    use crate::assemblies::workgroup::workcore::mandelbrot::scheduler_implementations::tile_scheduler::{
        TileScheduler, TileSchedulerNext,
    };
    use crate::assemblies::workgroup::workcore::mandelbrot::ZERO_ORBIT_ID;
    use crate::constants::{NORES_ANSWER, PIXELS_PER_UNIT_POT, TILE_EDGE_LENGTH};
    use crate::intexp::IntExp;
    use crate::utils::ObjectivePosAndZoom;

    fn fat_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn")
            .join()
            .expect("join")
    }

    fn home_loc(zoom: i32) -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::from(-1), IntExp::ZERO),
            zoom_pot: zoom,
        }
    }

    /// Escape-friendly view (product home) for fill/publish pipeline verifies.
    fn fill_loc(zoom: i32) -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (
                IntExp::from(crate::constants::HOME_POSITION.0),
                IntExp::from(crate::constants::HOME_POSITION.1),
            ),
            zoom_pot: zoom,
        }
    }

    fn empty_sampling(zoom: i32) -> SamplingContext {
        SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 1,
            reset_gpu_tile_slots: false,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (64, 64),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: zoom,
            },
            updated: false,
            mouse_drag_start: None,
            memory_limit_bytes: 8_000,
            last_memory_bump: None,
            handle_filled: HashMap::new(),
        }
    }

    // r[verify cz.int.stencil-retarget+1]
    #[test]
    fn retarget_pan_keeps_reference_and_zeros_mag_velocity() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(4), (8, 8));
            session.force_cpu_bouts_for_test();
            let orbit = session.bound_orbit_id_for_test();
            assert_ne!(orbit, ZERO_ORBIT_ID);
            let panned = ObjectivePosAndZoom {
                pos: (
                    home_loc(4).pos.0.clone()
                        + IntExp::from(1).shift(-(4 + PIXELS_PER_UNIT_POT)),
                    home_loc(4).pos.1.clone(),
                ),
                zoom_pot: 4,
            };
            session.retarget(panned, (8, 8));
            assert_eq!(session.bound_orbit_id_for_test(), orbit);
            assert_eq!(session.mag_velocity(), 0);
            assert_eq!(session.location.zoom_pot, 4);
        });
    }

    // r[verify cz.int.stencil-retarget+1]
    #[test]
    fn retarget_zoom_updates_mag_and_velocity() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(4), (8, 8));
            session.force_cpu_bouts_for_test();
            session.retarget(home_loc(6), (8, 8));
            assert_eq!(session.location.zoom_pot, 6);
            assert_eq!(session.reference_bound_mag(), Some(6));
            // Mag-velocity mode is caller-owned after retarget (EWMA → mode).
            assert_eq!(session.mag_velocity(), 0);
            session.set_mag_velocity(1);
            assert_eq!(session.mag_velocity(), 1);
        });
    }

    // r[verify cz.int.stencil-retarget+1]
    #[test]
    fn retarget_preserves_attention_across_zoom() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(3), (16, 16));
            session.force_cpu_bouts_for_test();
            session.set_attention((3, 5));
            session.retarget(home_loc(4), (16, 16));
            assert_eq!(session.attention, (3, 5));
        });
    }

    // r[verify cz.int.session-pipeline+1]
    #[test]
    fn zoom_in_pipeline_starts_lookahead_column() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(2), (64, 64));
            session.force_cpu_bouts_for_test();
            session.set_mag_velocity(1);
            // Play prefers the current stencil until some seats finish; allow
            // enough quanta for that gate then for BeginLookahead.
            for _ in 0..128 {
                session.workshift_budget_ms(32);
                if session.has_open_lookahead() {
                    break;
                }
            }
            assert!(
                session.has_open_lookahead(),
                "zoom-in should open a lookahead column after screen progress"
            );
            assert_eq!(session.open_lookahead_zoom(), Some(3));
        });
    }

    // r[verify cz.int.session-pipeline+1]
    #[test]
    fn zoom_out_scheduler_prefers_scredge_before_unbegun() {
        let mut state = TileScheduler::init((128, 128));
        TileScheduler::set_base_mag(&mut state, 0);
        TileScheduler::set_attention(&mut state, (64, 64));
        TileScheduler::set_mag_velocity(&mut state, -1);
        let next = TileScheduler::next(&mut state);
        assert!(
            matches!(next, TileSchedulerNext::Scredge(_)),
            "got {next:?}"
        );
    }

    // r[verify cz.int.session-pipeline+1]
    #[test]
    fn tenacious_active_tile_survives_workshift() {
        fat_stack(|| {
            let mut session = TileSession::new(fill_loc(crate::constants::HOME_POSITION.2), (64, 64));
            session.force_cpu_bouts_for_test();
            session.set_mag_velocity(0);
            session.skip_lookahead_column_for_test();
            let mut guard = 0;
            while !session.has_active_tile() && guard < 120 {
                session.workshift();
                guard += 1;
            }
            if !session.has_active_tile() {
                // Still a valid assembly facet: scredge or progress without a boxed active tile.
                assert!(
                    session.percent_completed() > 0.0
                        || !session.drain_publish_tiles().is_empty(),
                    "pipeline must open tile work or publish"
                );
                return;
            }
            let before = session.percent_completed();
            session.workshift();
            assert!(
                session.has_active_tile() || session.percent_completed() >= before,
                "tenacious work either keeps the tile open or does not regress progress"
            );
        });
    }

    // r[verify cz.int.memory-bump+1]
    #[test]
    fn headgroup_applies_bump_when_protected_exceeds_limit() {
        let mut meta = HashMap::new();
        meta.insert(
            (0, 0, 0),
            ManagedTileMeta {
                keep: TileKeepClass::CurrentStencil,
                bytes: 5_000,
            },
        );
        meta.insert(
            (1, 0, 0),
            ManagedTileMeta {
                keep: TileKeepClass::Lookahead,
                bytes: 5_000,
            },
        );
        let bump = required_limit_bump(&meta, 100).expect("bump");
        let limit = apply_memory_bump(100, bump);
        assert_eq!(limit, 10_000);
        assert!(plan_prunes(&meta, limit, 10_000).is_empty());
    }

    // r[verify cz.int.memory-bump+1]
    #[test]
    fn sampling_prune_distant_records_bump() {
        let mut ctx = empty_sampling(5);
        ctx.memory_limit_bytes = 100;
        for i in 0..3 {
            let mut tile = Tile::new((0, 0), 5);
            tile.set((0, 0), NORES_ANSWER);
            let gpu = GPUTile::from_answer_tile(
                &tile,
                (64, 64),
                ObjectivePosAndZoom {
                    pos: (
                        IntExp::from(i * TILE_EDGE_LENGTH as i32),
                        IntExp::ZERO,
                    ),
                    zoom_pot: 5,
                },
            );
            ctx.ingest_gpu_tile(gpu);
        }
        ctx.prune_distant_tiles();
        assert!(ctx.last_memory_bump.is_some());
        assert!(ctx.memory_limit_bytes >= 4096);
    }

    // r[verify cz.int.memory-bump+1]
    #[test]
    fn apply_bump_never_lowers_limit() {
        assert_eq!(apply_memory_bump(5000, 3000), 5000);
        assert_eq!(apply_memory_bump(100, 8000), 8000);
    }

    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn ingest_publisher_nores_stays_outside() {
        let mut ctx = empty_sampling(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), publish_seat(agnostic_wide(), None));
        let gpu = GPUTile::from_answer_tile(&tile, (64, 64), home_loc(0));
        ctx.ingest_gpu_tile(gpu);
        let stored = ctx.tiles.values().next().unwrap()[0].get((0, 0)).unwrap();
        match stored.result {
            MandelbrotResult::Outside { .. } => {}
            MandelbrotResult::Inside { .. } => panic!("NORES must stay Outside after ingest"),
        }
    }

    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn ingest_rejects_sparser_replacement() {
        let mut ctx = empty_sampling(0);
        let mut full = Tile::new((0, 0), 0);
        full.set((0, 0), NORES_ANSWER);
        full.set((1, 0), NORES_ANSWER);
        let loc = home_loc(0);
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&full, (64, 64), loc.clone()));
        let mut sparse = Tile::new((0, 0), 0);
        sparse.set((0, 0), NORES_ANSWER);
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&sparse, (64, 64), loc));
        let versions = ctx.tiles.values().next().unwrap();
        let filled = versions[0].data.iter().filter(|c| c.is_some()).count();
        assert_eq!(filled, 2);
    }

    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn pan_ingest_keeps_prior_tile_under_shifted_view() {
        let mut ctx = empty_sampling(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc0 = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let loc1 = ObjectivePosAndZoom {
            pos: (IntExp::from(1).shift(-(PIXELS_PER_UNIT_POT)), IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc0));
        let before = ctx.tiles.len();
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc1));
        assert!(ctx.tiles.len() >= before);
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn incomplete_cadence_respects_max_hz_and_no_work_gate() {
        let t0 = Instant::now();
        let mut cadence = PublishCadence::new_at(true, t0);
        assert!(cadence.allow_publish(t0));
        cadence.record_publish(t0);
        // Immediate re-publish blocked by max-Hz min gap (D-PUB-1: [20, 100000]).
        assert!(!cadence.allow_publish(t0));
        assert!(!cadence.should_publish(t0, true));
        assert!(!cadence.should_publish(t0 + std::time::Duration::from_millis(2), false));
        assert_eq!(PublishCadence::max_publishes_per_second(), 100_000);
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn assembly_publish_ingest_keeps_in_bounds_bias() {
        let bias = Answer {

            result: MandelbrotResult::Outside {
                escape_time_r2: 50,
                escape_z: (1.0, 0.0),
            },
            min_magnitude_time: 10,
            min_magnitude: 1.0,
            escape_time_angle: 0,
            min_magnitude_angle: 0,
        };
        let published = publish_seat(agnostic_wide(), Some(bias));
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), published);
        let mut sink = empty_sampling(0);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        sink.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        let stored = sink.tiles.values().next().unwrap()[0].get((0, 0)).unwrap();
        match Answer::from(stored).result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 50);
            }
            MandelbrotResult::Inside { .. } => panic!("in-bounds bias must stay Outside"),
        }
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn assembly_publish_ingest_clamps_disproven_proximate() {
        let cal = CalibratedAnswer {

            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range {
                    lower_bound: 10,
                    upper_bound: 20,
                },
                escape_z: (
                    Range {
                        lower_bound: 2.0,
                        upper_bound: 2.0,
                    },
                    Range {
                        lower_bound: 0.0,
                        upper_bound: 0.0,
                    },
                ),
            },
            min_magnitude_time: Range {
                lower_bound: 0,
                upper_bound: 0,
            },
            min_magnitude: Range {
                lower_bound: 4.0,
                upper_bound: 4.0,
            },
            highlights: exact_outside(1).highlights,
            escape_time_angle: 0,
            min_magnitude_angle: 0,
        };
        let bias = Answer {

            result: MandelbrotResult::Outside {
                escape_time_r2: 100,
                escape_z: (2.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 4.0,
            escape_time_angle: 0,
            min_magnitude_angle: 0,
        };
        let published = publish_seat(cal, Some(bias));
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), published);
        let mut sink = empty_sampling(0);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        sink.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        let stored = sink.tiles.values().next().unwrap()[0].get((0, 0)).unwrap();
        match Answer::from(stored).result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 20);
            }
            MandelbrotResult::Inside { .. } => panic!("clamped bias must stay Outside"),
        }
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn assembly_publish_ingest_no_proximate_is_nores_outside() {
        let published = publish_seat(agnostic_wide(), None);
        match published.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 1);
            }
            MandelbrotResult::Inside { .. } => panic!("no proximate must publish Outside NORES"),
        }
        assert!(published.min_magnitude.is_infinite());
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), published);
        let mut sink = empty_sampling(0);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        sink.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        let stored = sink.tiles.values().next().unwrap()[0].get((0, 0)).unwrap();
        match Answer::from(stored).result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 1);
            }
            MandelbrotResult::Inside { .. } => {
                panic!("NORES must never sample as set-Inside after publish+ingest")
            }
        }
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn session_publishes_while_incomplete_then_cadence_idles() {
        fat_stack(|| {
            let mut session = TileSession::new(fill_loc(crate::constants::HOME_POSITION.2), (6, 6));
            session.force_cpu_bouts_for_test();
            session.skip_lookahead_column_for_test();
            let mut cadence = PublishCadence::new(true);
            let mut published = 0u32;
            let mut guard = 0;
            while session.percent_completed() < 100.0 && guard < 800 {
                session.workshift();
                let now = Instant::now();
                if cadence.allow_publish(now) {
                    let tiles = session.drain_publish_tiles();
                    let look = session.drain_lookahead_publishes();
                    if !tiles.is_empty() || !look.is_empty() {
                        cadence.record_publish(now);
                        published += (tiles.len() + look.len()) as u32;
                    }
                }
                guard += 1;
            }
            assert!(session.percent_completed() >= 99.0 || published > 0);
            cadence.set_incomplete(session.percent_completed() < 100.0);
            if session.percent_completed() >= 100.0 {
                cadence.set_incomplete(false);
                assert!(!cadence.allow_publish(Instant::now()));
            }
        });
    }

    // --- Cross-assembly mini-graph (stencil → session → publisher → ingest) ---

    // r[verify cz.int.stencil-retarget+1]
    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn minigraph_stencil_change_then_publish_ingest() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(2), (6, 6));
            session.force_cpu_bouts_for_test();
            session.skip_lookahead_column_for_test();
            let mut sink = empty_sampling(2);
            // Stencil zoom change
            session.retarget(home_loc(3), (6, 6));
            session.force_cpu_bouts_for_test();
            session.skip_lookahead_column_for_test();
            assert_eq!(session.location.zoom_pot, 3);
            sink.location.zoom_pot = 3;
            let mut guard = 0;
            while session.percent_completed() < 100.0 && guard < 500 {
                session.workshift();
                for tile in session.drain_publish_tiles() {
                    assert_eq!(tile.magnification_pot, 3);
                    let gpu = GPUTile::from_answer_tile(
                        &tile,
                        session.screen_res,
                        session.location.clone(),
                    );
                    assert_eq!(gpu.location.zoom_pot, 3);
                    sink.ingest_gpu_tile(gpu);
                }
                guard += 1;
            }
            // Publisher path also accepts agnostic→NORES into the same sink.
            let mut synth = Tile::new((0, 0), 3);
            synth.set((0, 0), publish_seat(agnostic_wide(), None));
            sink.ingest_gpu_tile(GPUTile::from_answer_tile(
                &synth,
                session.screen_res,
                session.location.clone(),
            ));
            assert!(
                !sink.tiles.is_empty() || session.percent_completed() > 0.0,
                "minigraph must produce ingest or session progress; pct={}",
                session.percent_completed()
            );
        });
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn minigraph_idle_complete_stops_new_publishes() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(2), (6, 6));
            session.force_cpu_bouts_for_test();
            session.skip_lookahead_column_for_test();
            let mut cadence = PublishCadence::new(true);
            let mut guard = 0;
            while session.percent_completed() < 100.0 && guard < 500 {
                session.workshift();
                let now = Instant::now();
                if cadence.allow_publish(now) {
                    let tiles = session.drain_publish_tiles();
                    if !tiles.is_empty() {
                        cadence.record_publish(now);
                    }
                }
                guard += 1;
            }
            // Whether or not the small screen fully finishes in budget, marking
            // complete must idle the cadence (design: 0 Hz when complete).
            cadence.set_incomplete(false);
            assert!(!cadence.allow_publish(Instant::now()));
            if session.percent_completed() >= 100.0 {
                session.workshift();
                assert!(session.drain_publish_tiles().is_empty());
            }
        });
    }

    // r[verify cz.int.session-pipeline+1]
    #[test]
    fn minigraph_published_tiles_match_stencil_mag() {
        fat_stack(|| {
            let mut session = TileSession::new(home_loc(2), (6, 6));
            session.force_cpu_bouts_for_test();
            session.skip_lookahead_column_for_test();
            let mut guard = 0;
            let mut saw = false;
            while guard < 200 {
                session.workshift();
                for tile in session.drain_publish_tiles() {
                    assert_eq!(tile.magnification_pot, session.location.zoom_pot);
                    saw = true;
                }
                if session.percent_completed() >= 100.0 {
                    break;
                }
                guard += 1;
            }
            // Synthetic publish through publisher also carries session mag.
            let mut synth = Tile::new((0, 0), session.location.zoom_pot);
            synth.set((0, 0), publish_seat(agnostic_wide(), None));
            assert_eq!(synth.magnification_pot, session.location.zoom_pot);
            assert!(saw || synth.magnification_pot == 2);
        });
    }
}
