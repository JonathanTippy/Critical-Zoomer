// read delivery.md for project context
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

/// u32 words per tile for a 4096-seat done bitfield (D-GPU-4).
pub const SEAT_DONE_WORDS_PER_SLOT: u32 = 128;

/// GPU-resident slots for tiles the workgroup is still working on.
pub struct ProductionAtlas {
    shared: Arc<GpuContext>
    , meta_texture: wgpu::Texture
    , meta_view: wgpu::TextureView
    , z_texture: wgpu::Texture
    , z_view: wgpu::TextureView
    // Per-slot completion counts (D-GPU-3). Host may map 4 bytes only.
    , completion_counters: wgpu::Buffer
    // Per-slot seat done bits — first 0→1 only bumps the counter (D-GPU-4).
    , seat_done_bits: wgpu::Buffer
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
        let (completion_counters, seat_done_bits) =
            Self::alloc_completion_buffers(&shared.device, slot_capacity);
        ProductionAtlas {
            shared
            , meta_texture
            , meta_view
            , z_texture
            , z_view
            , completion_counters
            , seat_done_bits
            , slot_capacity
            , slot_ceiling
            , free_slots: Vec::new()
            , next_slot: 0
        }
    }

    fn alloc_completion_buffers(
        device: &wgpu::Device,
        slot_capacity: u32,
    ) -> (wgpu::Buffer, wgpu::Buffer) {
        let completion_counters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("production_atlas_completion_counters"),
            size: u64::from(slot_capacity) * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let seat_done_bits = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("production_atlas_seat_done_bits"),
            size: u64::from(slot_capacity) * u64::from(SEAT_DONE_WORDS_PER_SLOT) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (completion_counters, seat_done_bits)
    }

    pub fn completion_counters(&self) -> &wgpu::Buffer {
        &self.completion_counters
    }

    pub fn seat_done_bits(&self) -> &wgpu::Buffer {
        &self.seat_done_bits
    }

    /// Clear on-device completion state for a newly acquired slot (D-GPU-4).
    pub fn clear_slot_completion(&self, slot: u32) {
        if slot >= self.slot_capacity {
            return;
        }
        self.shared
            .queue
            .write_buffer(&self.completion_counters, u64::from(slot) * 4, &[0u8; 4]);
        let done_bytes = usize::try_from(SEAT_DONE_WORDS_PER_SLOT)
            .unwrap_or(128)
            .saturating_mul(4);
        let zeros = vec![0u8; done_bytes];
        self.shared.queue.write_buffer(
            &self.seat_done_bits,
            u64::from(slot) * u64::from(SEAT_DONE_WORDS_PER_SLOT) * 4,
            &zeros,
        );
    }

    /// Read several per-tile completion counters in one copy+map (D-GPU-2/3).
    pub fn read_completion_counts(&self, slots: &[u32]) -> Option<Vec<u32>> {
        if slots.is_empty() {
            return Some(Vec::new());
        }
        let device = &self.shared.device;
        let bytes = (slots.len() as u64) * 4;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("production_atlas_completion_staging_bulk"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("production_atlas_completion_read_bulk"),
        });
        for (i, &slot) in slots.iter().enumerate() {
            if slot >= self.slot_capacity {
                return None;
            }
            encoder.copy_buffer_to_buffer(
                &self.completion_counters,
                u64::from(slot) * 4,
                &staging,
                (i as u64) * 4,
                4,
            );
        }
        self.shared.queue.submit(Some(encoder.finish()));
        let slice = staging.slice(..bytes);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let started = std::time::Instant::now();
        loop {
            let _ = device.poll(wgpu::PollType::Poll);
            match rx.try_recv() {
                Ok(Ok(())) => {
                    let counts = {
                        let data = slice.get_mapped_range();
                        (0..slots.len())
                            .map(|i| {
                                let o = i * 4;
                                u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
                            })
                            .collect()
                    };
                    staging.unmap();
                    return Some(counts);
                }
                Ok(Err(_)) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if started.elapsed().as_millis() > 2_000 {
                        return None;
                    }
                    std::thread::yield_now();
                }
            }
        }
    }

    /// Read the per-tile completion counter (4-byte map only — D-GPU-2/3).
    pub fn read_completion_count(&self, slot: u32) -> Option<u32> {
        self.read_completion_counts(&[slot])
            .and_then(|v| v.into_iter().next())
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
        let slot = if let Some(slot) = self.free_slots.pop() {
            slot
        } else {
            if self.next_slot >= self.slot_capacity && !self.grow(self.next_slot + 1) {
                return None;
            }
            let slot = self.next_slot;
            self.next_slot += 1;
            slot
        };
        self.clear_slot_completion(slot);
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
        let (completion_counters, seat_done_bits) =
            Self::alloc_completion_buffers(device, capacity);

        // Slots keep their origins across growth, so the live prefix copies as
        // one block and in-flight tiles keep the slot they were handed.
        let old = tile_sheet::sheet_size_for(self.slot_capacity);
        let old_counter_bytes = u64::from(self.slot_capacity) * 4;
        let old_done_bytes =
            u64::from(self.slot_capacity) * u64::from(SEAT_DONE_WORDS_PER_SLOT) * 4;
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
        encoder.copy_buffer_to_buffer(
            &self.completion_counters,
            0,
            &completion_counters,
            0,
            old_counter_bytes,
        );
        encoder.copy_buffer_to_buffer(
            &self.seat_done_bits,
            0,
            &seat_done_bits,
            0,
            old_done_bytes,
        );
        self.shared.queue.submit(Some(encoder.finish()));

        self.meta_texture = meta_texture;
        self.meta_view = meta_view;
        self.z_texture = z_texture;
        self.z_view = z_view;
        self.completion_counters = completion_counters;
        self.seat_done_bits = seat_done_bits;
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
