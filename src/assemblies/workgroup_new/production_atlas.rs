//! Where the workgroup builds tiles, in GPU memory, before publishing them.
//!
//! The workgroup and headgroup keep separate hoards and do not synchronise them
//! (architecture.md, tile_manager.md). This is the workgroup's side: slots for
//! tiles currently being produced, which the bout shader writes in place and
//! the publisher reads. A tile leaves here by being copied into a headgroup
//! slot, never by being read back to the CPU.
//!
//! It is deliberately small. The workgroup only ever holds the tiles in flight
//! for the current stencil plus its lookahead, so this is a working set, not a
//! hoard; the hoard the workgroup accounts for stays CPU-side per tile_manager.
// r[impl cz.seamless.gpu-preferred+1]

use std::sync::{Arc, Mutex, OnceLock};

use crate::assemblies::tile_sheet;
use crate::gpu_context::GpuContext;

/// Slots the production atlas starts with, and grows past on demand.
///
/// A 1080p stencil is 30x17 tiles, so this covers a screen's worth of in-flight
/// work without a reallocation on the first fill.
const INITIAL_SLOTS: u32 = 512;

/// GPU-resident slots for tiles the workgroup is still working on.
pub struct ProductionAtlas {
    shared: Arc<GpuContext>
    , meta_texture: wgpu::Texture
    , meta_view: wgpu::TextureView
    , z_texture: wgpu::Texture
    , z_view: wgpu::TextureView
    , slot_capacity: u32
    , slot_ceiling: u32
    , free_slots: Vec<u32>
    , next_slot: u32
}

/// Process-wide production atlas. The uploader writes finished tiles here; the
/// headgroup copies them out. One atlas means one place for in-flight VRAM.
pub type SharedProductionAtlas = Option<Arc<Mutex<ProductionAtlas>>>;

impl ProductionAtlas {
    /// Build the atlas on the app's shared device, or `None` without a GPU.
    pub fn new() -> Option<Self> {
        let shared = GpuContext::shared()?;
        Some(Self::with_context(shared))
    }

    /// The workgroup's one production atlas, built on first use.
    pub fn shared() -> SharedProductionAtlas {
        static SHARED: OnceLock<SharedProductionAtlas> = OnceLock::new();
        SHARED
            .get_or_init(|| ProductionAtlas::new().map(|atlas| Arc::new(Mutex::new(atlas))))
            .clone()
    }

    pub fn with_context(shared: Arc<GpuContext>) -> Self {
        let slot_ceiling = tile_sheet::max_slots_for(&shared.device);
        let slot_capacity = INITIAL_SLOTS.min(slot_ceiling);
        let (meta_texture, meta_view) =
            tile_sheet::create_sheet(&shared.device, slot_capacity, "workgroup_meta_sheet");
        let (z_texture, z_view) =
            tile_sheet::create_sheet(&shared.device, slot_capacity, "workgroup_z_sheet");
        ProductionAtlas {
            shared
            , meta_texture
            , meta_view
            , z_texture
            , z_view
            , slot_capacity
            , slot_ceiling
            , free_slots: Vec::new()
            , next_slot: 0
        }
    }

    pub fn meta_view(&self) -> &wgpu::TextureView {
        &self.meta_view
    }

    pub fn z_view(&self) -> &wgpu::TextureView {
        &self.z_view
    }

    pub fn meta_texture(&self) -> &wgpu::Texture {
        &self.meta_texture
    }

    pub fn z_texture(&self) -> &wgpu::Texture {
        &self.z_texture
    }

    pub fn slot_capacity(&self) -> u32 {
        self.slot_capacity
    }

    /// Slots currently held by in-flight tiles.
    pub fn slots_in_use(&self) -> u32 {
        self.next_slot - self.free_slots.len() as u32
    }

    /// VRAM this atlas occupies. Counted against the workgroup's own budget,
    /// separately from the headgroup's hoard.
    pub fn bytes(&self) -> u64 {
        u64::from(self.slot_capacity) * tile_sheet::SLOT_BYTES
    }

    /// Take a slot for a tile about to be produced.
    ///
    /// Grows the sheets rather than refusing while the device allows it: a
    /// refused slot would mean a tile the worker cannot finish.
    pub fn acquire(&mut self) -> Option<u32> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        if self.next_slot >= self.slot_capacity && !self.grow(self.next_slot + 1) {
            return None;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        Some(slot)
    }

    /// Give a slot back once its tile has been published.
    pub fn release(&mut self, slot: u32) {
        if slot < self.next_slot && !self.free_slots.contains(&slot) {
            self.free_slots.push(slot);
        }
    }

    /// Write a CPU-computed tile into a slot.
    ///
    /// Only the CPU-fallback worker reaches this; GPU-computed tiles are already
    /// in their slot when the bout finishes.
    pub fn write_slot(&self, slot: u32, meta: &[[f32; 4]], z: &[[f32; 4]]) {
        tile_sheet::write_slot(&self.shared.queue, &self.meta_texture, slot, meta);
        tile_sheet::write_slot(&self.shared.queue, &self.z_texture, slot, z);
    }

    /// Hand a finished tile to the headgroup's sheets, GPU to GPU.
    pub fn copy_slot_to(
        &self
        , encoder: &mut wgpu::CommandEncoder
        , slot: u32
        , dst_meta: &wgpu::Texture
        , dst_z: &wgpu::Texture
        , dst_slot: u32
    ) {
        tile_sheet::copy_slot(encoder, &self.meta_texture, slot, dst_meta, dst_slot);
        tile_sheet::copy_slot(encoder, &self.z_texture, slot, dst_z, dst_slot);
    }

    fn grow(&mut self, wanted: u32) -> bool {
        if wanted <= self.slot_capacity {
            return true;
        }
        if self.slot_capacity >= self.slot_ceiling {
            return false;
        }
        let capacity = wanted
            .next_power_of_two()
            .max(tile_sheet::SHEET_COLS)
            .min(self.slot_ceiling);
        let device = &self.shared.device;
        let (meta_texture, meta_view) =
            tile_sheet::create_sheet(device, capacity, "workgroup_meta_sheet");
        let (z_texture, z_view) =
            tile_sheet::create_sheet(device, capacity, "workgroup_z_sheet");

        // Slots keep their origins across growth, so the live prefix copies as
        // one block and in-flight tiles keep the slot they were handed.
        let old = tile_sheet::sheet_size_for(self.slot_capacity);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("workgroup_atlas_grow")
        });
        for (src, dst) in [
            (&self.meta_texture, &meta_texture)
            , (&self.z_texture, &z_texture)
        ] {
            encoder.copy_texture_to_texture(
                src.as_image_copy()
                , dst.as_image_copy()
                , wgpu::Extent3d {
                    width: old.0
                    , height: old.1
                    , depth_or_array_layers: 1
                }
            );
        }
        self.shared.queue.submit(Some(encoder.finish()));

        self.meta_texture = meta_texture;
        self.meta_view = meta_view;
        self.z_texture = z_texture;
        self.z_view = z_view;
        self.slot_capacity = capacity;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas() -> Option<ProductionAtlas> {
        ProductionAtlas::new()
    }

    #[test]
    fn a_released_slot_is_handed_out_again_before_a_fresh_one() {
        let Some(mut atlas) = atlas() else { return };
        let first = atlas.acquire().expect("first slot");
        let second = atlas.acquire().expect("second slot");
        atlas.release(first);
        assert_eq!(
            atlas.acquire()
            , Some(first)
            , "in-flight work is short lived, so slots must be reused rather than grown past"
        );
        assert_ne!(first, second);
    }

    #[test]
    fn the_atlas_grows_rather_than_refusing_in_flight_work() {
        let Some(mut atlas) = atlas() else { return };
        let start = atlas.slot_capacity();
        for _ in 0..=start {
            assert!(
                atlas.acquire().is_some()
                , "a refused slot is a tile the worker can never finish"
            );
        }
        assert!(atlas.slot_capacity() > start);
    }

    #[test]
    fn slots_in_use_tracks_acquire_and_release() {
        let Some(mut atlas) = atlas() else { return };
        assert_eq!(atlas.slots_in_use(), 0);
        let a = atlas.acquire().unwrap();
        let b = atlas.acquire().unwrap();
        assert_eq!(atlas.slots_in_use(), 2);
        atlas.release(a);
        assert_eq!(atlas.slots_in_use(), 1);
        atlas.release(b);
        assert_eq!(atlas.slots_in_use(), 0);
    }

    #[test]
    fn releasing_a_slot_twice_does_not_hand_it_out_twice() {
        let Some(mut atlas) = atlas() else { return };
        let slot = atlas.acquire().unwrap();
        atlas.release(slot);
        atlas.release(slot);
        let first = atlas.acquire();
        let second = atlas.acquire();
        assert_eq!(first, Some(slot));
        assert_ne!(
            second, Some(slot)
            , "two tiles sharing a slot would overwrite each other's answers"
        );
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn a_tile_written_here_can_be_copied_into_a_headgroup_sheet() {
        let Some(atlas) = atlas() else { return };
        let shared = GpuContext::shared().expect("atlas implies a context");
        let (dst_meta, _dst_meta_view) =
            tile_sheet::create_sheet(&shared.device, 64, "test_dst_meta");
        let (dst_z, _dst_z_view) = tile_sheet::create_sheet(&shared.device, 64, "test_dst_z");

        let seats = (tile_sheet::TILE_EDGE * tile_sheet::TILE_EDGE) as usize;
        atlas.write_slot(0, &vec![[1.0, 2.0, 3.0, 4.0]; seats], &vec![[5.0; 4]; seats]);

        let mut encoder = shared
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        atlas.copy_slot_to(&mut encoder, 0, &dst_meta, &dst_z, 3);
        shared.queue.submit(Some(encoder.finish()));
        shared
            .device
            .poll(wgpu::PollType::Wait)
            .expect("handoff copy must complete on the shared device");
    }
}
