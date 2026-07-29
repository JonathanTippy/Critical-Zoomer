use steady_state::*;
use crate::assemblies::structs::*;
use crate::assemblies::headgroup::window::gpu_display::pack_tile_upload;
use crate::assemblies::workgroup::tile_worker::AnswerTilePublish;
use crate::assemblies::workgroup::production_atlas::ProductionAtlas;

pub struct GpuUploaderState {
    unsent: Option<GpuTileHandle>
    , production: crate::assemblies::workgroup::production_atlas::SharedProductionAtlas
}

pub async fn run(
    actor: SteadyActorShadow
    , tiles_in: SteadyRx<AnswerTilePublish>
    , tiles_out: SteadyTx<GpuTileHandle>
    , state: SteadyState<GpuUploaderState>
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&tiles_in], [&tiles_out])
        , tiles_in
        , tiles_out
        , state
    )
        .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A
    , tiles_in: SteadyRx<AnswerTilePublish>
    , tiles_out: SteadyTx<GpuTileHandle>
    , state: SteadyState<GpuUploaderState>
) -> Result<(), Box<dyn Error>> {
    let mut tiles_in = tiles_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
    let mut state = state.lock(|| GpuUploaderState {
        unsent: None
        , production: ProductionAtlas::shared()
    }).await;

    // Always re-check inputs at a quick pace; fully drain below.
    let max_sleep = Duration::from_millis(1);

    while actor.is_running(
        || i!(tiles_out.mark_closed())
    ) {
        // Prefer draining publish backlog over sleeping when work is waiting.
        if actor.avail_units(&mut tiles_in) == 0 && state.unsent.is_none() {
            await_for_any!(
                actor.wait_periodic(max_sleep)
                , actor.wait_avail(&mut tiles_in, 1)
            );
        }

        if let Some(tile) = state.unsent.take() {
            match actor.try_send(&mut tiles_out, tile) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(tile)
                | SendOutcome::Timeout(tile)
                | SendOutcome::Closed(tile) => {
                    state.unsent = Some(tile);
                    continue;
                }
            }
        }

        while actor.avail_units(&mut tiles_in) > 0 {
            let Some(publish) = actor.try_take(&mut tiles_in) else {
                break;
            };
            let gpu_tile = GPUTile::from_answer_tile(
                &publish.tile
                , publish.screen_res
                , publish.location
            );
            // CPU-fallback uploader: pack answers into the production atlas so
            // the headgroup can copy them GPU-to-GPU. When there is no atlas,
            // the handle carries the CPU tile and the headgroup uploads bytes.
            let handle = place_on_production_atlas(&state.production, gpu_tile);
            match actor.try_send(&mut tiles_out, handle) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(tile)
                | SendOutcome::Timeout(tile)
                | SendOutcome::Closed(tile) => {
                    state.unsent = Some(tile);
                    break;
                }
            }
        }
    }

    info!("GPU uploader shutting down.");
    Ok(())
}

pub(crate) fn place_on_production_atlas(
    production: &crate::assemblies::workgroup::production_atlas::SharedProductionAtlas
    , gpu_tile: GPUTile
) -> GpuTileHandle {
    let Some(atlas) = production else {
        return GpuTileHandle::from_gpu_tile(gpu_tile, None);
    };
    let Ok(mut atlas) = atlas.lock() else {
        return GpuTileHandle::from_gpu_tile(gpu_tile, None);
    };
    let Some(slot) = atlas.acquire() else {
        return GpuTileHandle::from_gpu_tile(gpu_tile, None);
    };
    let packed = pack_tile_upload(&gpu_tile, 0);
    atlas.write_slot(slot, &packed.meta, &packed.z);
    GpuTileHandle::from_gpu_tile(gpu_tile, Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intexp::IntExp;
    use crate::utils::ObjectivePosAndZoom;

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn uploader_places_a_tile_in_the_production_atlas_when_gpu_exists() {
        let Some(production) = ProductionAtlas::shared() else {
            return;
        };
        let tile = GPUTile::from_answer_tile(
            &Tile::new((0, 0), 0)
            , (64, 64)
            , ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO)
                , zoom_pot: 0
            }
        );
        let before = {
            let atlas = production.lock().unwrap();
            atlas.slots_in_use()
        };
        let handle = place_on_production_atlas(&Some(production.clone()), tile);
        assert!(
            handle.production_slot.is_some()
            , "with a production atlas the uploader must hand off a slot, not CPU bytes"
        );
        assert!(handle.cpu_fallback.is_none());
        let after = production.lock().unwrap().slots_in_use();
        assert_eq!(after, before + 1);
    }
}
