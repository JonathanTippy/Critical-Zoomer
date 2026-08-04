// read delivery.md for project context
// GPU scatter: terminal bout points → production atlas slot without host harvest.
// D-GPU-3/4: per-tile completion counter + seat-done 0→1; batch counter for scheduling only.
// Per-inflight seats/uniforms/batch counters so depth>1 does not clobber in-flight work.
// r[impl cz.pub.gpu-native-work+1]

use std::sync::{mpsc, Mutex, OnceLock};

use bytemuck::{Pod, Zeroable};

use crate::assemblies::tile_sheet;
use crate::assemblies::workgroup::production_atlas::ProductionAtlas;
use crate::constants::GPU_POINT_RING_DEPTH;
use crate::gpu_context::GpuContext;

/// Mapped-scatter staging depth (nomap waves do not consume these).
const SCATTER_PIPELINE_DEPTH: usize = GPU_POINT_RING_DEPTH;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ScatterParams {
    slot_origin_x: u32,
    slot_origin_y: u32,
    point_count: u32,
    slot_index: u32,
}

struct PendingScatter {
    staging_idx: usize,
    slot: u32,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

struct ScatterPipeline {
    counter_stagings: Vec<wgpu::Buffer>,
    batch_counters: Vec<wgpu::Buffer>,
    seat_buffers: Vec<wgpu::Buffer>,
    uniform_buffers: Vec<wgpu::Buffer>,
    free_stagings: Vec<usize>,
    pending: Vec<PendingScatter>,
}

impl ScatterPipeline {
    fn new(device: &wgpu::Device, max_points: u32) -> Self {
        let seat_bytes = u64::from(max_points) * 4;
        let uniform_size = std::mem::size_of::<ScatterParams>() as u64;
        let counter_stagings = (0..SCATTER_PIPELINE_DEPTH)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("bout_scatter_batch_staging_{i}")),
                    // 8 bytes: batch_terminals + tile_completion (D-GPU-3)
                    size: 8,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let batch_counters = (0..SCATTER_PIPELINE_DEPTH)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("bout_scatter_batch_terminals_{i}")),
                    size: 4,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let seat_buffers = (0..SCATTER_PIPELINE_DEPTH)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("bout_scatter_seats_{i}")),
                    size: seat_bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let uniform_buffers = (0..SCATTER_PIPELINE_DEPTH)
            .map(|i| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("bout_scatter_uniforms_{i}")),
                    size: uniform_size,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let free_stagings = (0..SCATTER_PIPELINE_DEPTH).collect();
        ScatterPipeline {
            counter_stagings,
            batch_counters,
            seat_buffers,
            uniform_buffers,
            free_stagings,
            pending: Vec::new(),
        }
    }

    fn acquire_staging(&mut self) -> Option<usize> {
        self.free_stagings.pop()
    }

    fn release_staging(&mut self, idx: usize) {
        if idx < SCATTER_PIPELINE_DEPTH {
            self.free_stagings.push(idx);
        }
    }
}

pub struct BoutScatter {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    max_points: u32,
    scatter_pipeline: Mutex<ScatterPipeline>,
    /// Cached nomap bind groups keyed by (point_key, ring_idx, identity_seats).
    nomap_bind_groups: Mutex<Vec<(usize, usize, bool, wgpu::BindGroup)>>,
    /// Immutable identity seats 0..N-1 — never rewritten by partial tiles.
    identity_seat_buffer: wgpu::Buffer,
}

impl BoutScatter {
    pub fn shared() -> Option<&'static Self> {
        static SCATTER: OnceLock<Option<BoutScatter>> = OnceLock::new();
        SCATTER.get_or_init(BoutScatter::new).as_ref()
    }

    fn new() -> Option<Self> {
        let shared = GpuContext::shared()?;
        let device = &shared.device;
        let max_points = crate::constants::GPU_WORKER_BATCH_N as u32;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bout_scatter"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bout_scatter.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bout_scatter_bgl"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                uniform_entry(2),
                storage_texture_entry(3),
                storage_texture_entry(4),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bout_scatter_pll"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bout_scatter_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("scatter"),
            compilation_options: Default::default(),
            cache: None,
        });
        let scatter_pipeline = ScatterPipeline::new(device, max_points);
        // Dedicated identity seats: never overwritten by partial-tile seat lists.
        let identity: Vec<u32> = (0..max_points).collect();
        let identity_seat_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bout_scatter_identity_seats"),
            size: u64::from(max_points) * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        shared
            .queue
            .write_buffer(&identity_seat_buffer, 0, bytemuck::cast_slice(&identity));
        Some(BoutScatter {
            pipeline,
            bind_group_layout,
            max_points,
            scatter_pipeline: Mutex::new(scatter_pipeline),
            nomap_bind_groups: Mutex::new(Vec::new()),
            identity_seat_buffer,
        })
    }

    /// Write scatter params/seats/counter for `ring_idx` (no encode). Call before
    /// `encode_scatter_nomap_pass` when batching many tiles into one submit.
    pub fn write_scatter_nomap(
        &self,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
        ring_idx: usize,
    ) -> bool {
        let shared = match GpuContext::shared() {
            Some(s) => s,
            None => return false,
        };
        let queue = &shared.queue;
        let count = local_seats.len().min(self.max_points as usize) as u32;
        if count == 0 {
            return true;
        }
        let seats_are_identity = local_seats.len() == self.max_points as usize
            && local_seats
                .iter()
                .enumerate()
                .all(|(i, &s)| s == i as u32);
        let origin = tile_sheet::slot_origin(slot);
        let params = ScatterParams {
            slot_origin_x: origin[0],
            slot_origin_y: origin[1],
            point_count: count,
            slot_index: slot,
        };
        let pipe = match self.scatter_pipeline.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let staging = ring_idx.min(SCATTER_PIPELINE_DEPTH.saturating_sub(1));
        let uniform = &pipe.uniform_buffers[staging];
        let staging_seats = &pipe.seat_buffers[staging];
        let batch_counter = &pipe.batch_counters[staging];
        queue.write_buffer(uniform, 0, bytemuck::bytes_of(&params));
        if !seats_are_identity {
            queue.write_buffer(staging_seats, 0, bytemuck::cast_slice(local_seats));
        }
        queue.write_buffer(batch_counter, 0, &[0u8; 4]);
        let _ = atlas; // slot already baked into params via tile_sheet::slot_origin
        true
    }

    /// Encode nomap scatter pass only (params must already be written for `ring_idx`).
    pub fn encode_scatter_nomap_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        point_buffer: &wgpu::Buffer,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
        ring_idx: usize,
    ) -> bool {
        let shared = match GpuContext::shared() {
            Some(s) => s,
            None => return false,
        };
        let device = &shared.device;
        let count = local_seats.len().min(self.max_points as usize) as u32;
        if count == 0 {
            return true;
        }
        let seats_are_identity = local_seats.len() == self.max_points as usize
            && local_seats
                .iter()
                .enumerate()
                .all(|(i, &s)| s == i as u32);
        let pipe = match self.scatter_pipeline.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let staging = ring_idx.min(SCATTER_PIPELINE_DEPTH.saturating_sub(1));
        let uniform = &pipe.uniform_buffers[staging];
        let staging_seats = &pipe.seat_buffers[staging];
        let batch_counter = &pipe.batch_counters[staging];
        let seats_binding = if seats_are_identity {
            self.identity_seat_buffer.as_entire_binding()
        } else {
            staging_seats.as_entire_binding()
        };
        let point_key = point_buffer as *const wgpu::Buffer as usize;
        let bind_group = {
            let mut cache = match self.nomap_bind_groups.lock() {
                Ok(c) => c,
                Err(_) => return false,
            };
            if let Some((_, _, _, bg)) = cache
                .iter()
                .find(|(k, r, id, _)| *k == point_key && *r == staging && *id == seats_are_identity)
            {
                bg.clone()
            } else {
                let meta_view =
                    atlas.meta_texture().create_view(&wgpu::TextureViewDescriptor::default());
                let z_view = atlas.z_texture().create_view(&wgpu::TextureViewDescriptor::default());
                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bout_scatter_nomap_bg"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: point_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: seats_binding,
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&meta_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&z_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: atlas.completion_counters().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: atlas.seat_done_bits().as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: batch_counter.as_entire_binding(),
                        },
                    ],
                });
                cache.push((point_key, staging, seats_are_identity, bg.clone()));
                bg
            }
        };
        drop(pipe);
        let _ = slot;
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("bout_scatter_nomap_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        true
    }

    /// Encode nomap scatter into `encoder` (writes seats/params; no submit).
    /// `ring_idx` selects per-inflight scatter uniforms/batch-counter (and staging seats when not identity).
    pub fn encode_scatter_nomap(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        point_buffer: &wgpu::Buffer,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
        ring_idx: usize,
    ) -> bool {
        if !self.write_scatter_nomap(atlas, slot, local_seats, ring_idx) {
            return false;
        }
        self.encode_scatter_nomap_pass(encoder, point_buffer, atlas, slot, local_seats, ring_idx)
    }

    /// Submit scatter with no counter map (D-GPU hot path). Caller syncs via tile counter.
    pub fn scatter_submit_nomap(
        &self,
        point_buffer: &wgpu::Buffer,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
    ) -> bool {
        let shared = match GpuContext::shared() {
            Some(s) => s,
            None => return false,
        };
        let device = &shared.device;
        let queue = &shared.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bout_scatter_nomap_enc"),
        });
        if !self.encode_scatter_nomap(&mut encoder, point_buffer, atlas, slot, local_seats, 0) {
            return false;
        }
        queue.submit(Some(encoder.finish()));
        true
    }

    /// Submit scatter into atlas slot. Tile completion counter is cumulative (not zeroed).
    /// Returns batch-new-terminals via later `poll_scatter_counter` (scheduling only).
    pub fn scatter_submit(
        &self,
        point_buffer: &wgpu::Buffer,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
    ) -> bool {
        let shared = match GpuContext::shared() {
            Some(s) => s,
            None => return false,
        };
        let device = &shared.device;
        let queue = &shared.queue;
        let count = local_seats.len().min(self.max_points as usize) as u32;
        if count == 0 {
            return true;
        }
        let mut pipe = match self.scatter_pipeline.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let staging_idx = match pipe.acquire_staging() {
            Some(i) => i,
            None => return false,
        };
        let origin = tile_sheet::slot_origin(slot);
        let params = ScatterParams {
            slot_origin_x: origin[0],
            slot_origin_y: origin[1],
            point_count: count,
            slot_index: slot,
        };
        let uniform = pipe.uniform_buffers[staging_idx].clone();
        let seats = pipe.seat_buffers[staging_idx].clone();
        let batch_counter = pipe.batch_counters[staging_idx].clone();
        queue.write_buffer(&uniform, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(&seats, 0, bytemuck::cast_slice(local_seats));
        queue.write_buffer(&batch_counter, 0, &[0u8; 4]);

        let meta_view = atlas.meta_texture().create_view(&wgpu::TextureViewDescriptor::default());
        let z_view = atlas.z_texture().create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bout_scatter_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: point_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: seats.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&meta_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&z_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: atlas.completion_counters().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: atlas.seat_done_bits().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: batch_counter.as_entire_binding(),
                },
            ],
        });
        let counter_staging = pipe.counter_stagings[staging_idx].clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bout_scatter_enc"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("bout_scatter_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&batch_counter, 0, &counter_staging, 0, 4);
        encoder.copy_buffer_to_buffer(
            atlas.completion_counters(),
            u64::from(slot) * 4,
            &counter_staging,
            4,
            4,
        );
        queue.submit(Some(encoder.finish()));

        let slice = counter_staging.slice(..8);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        pipe.pending.push(PendingScatter {
            staging_idx,
            slot,
            receiver,
        });
        true
    }

    /// Poll oldest pending scatter: `(batch_terminals, tile_completion_count)`.
    pub fn poll_scatter_counter(&self) -> Option<Option<(u32, u32)>> {
        let shared = GpuContext::shared()?;
        let device = &shared.device;
        let _ = device.poll(wgpu::PollType::Poll);
        let mut pipe = self.scatter_pipeline.lock().ok()?;
        if pipe.pending.is_empty() {
            return Some(None);
        }
        let pending = &pipe.pending[0];
        match pending.receiver.try_recv() {
            Ok(Ok(())) => {
                let staging_idx = pipe.pending.remove(0).staging_idx;
                let staging = &pipe.counter_stagings[staging_idx];
                let slice = staging.slice(..8);
                let (batch_terminals, tile_completion) = {
                    let data = slice.get_mapped_range();
                    (
                        u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
                        u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
                    )
                };
                staging.unmap();
                pipe.release_staging(staging_idx);
                Some(Some((batch_terminals, tile_completion)))
            }
            Ok(Err(_)) => {
                let staging_idx = pipe.pending.remove(0).staging_idx;
                pipe.release_staging(staging_idx);
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                let staging_idx = pipe.pending.remove(0).staging_idx;
                pipe.release_staging(staging_idx);
                None
            }
            Err(mpsc::TryRecvError::Empty) => Some(None),
        }
    }

    /// True while a deferred scatter map is still outstanding.
    pub fn has_pending_maps(&self) -> bool {
        self.scatter_pipeline
            .lock()
            .map(|p| !p.pending.is_empty())
            .unwrap_or(false)
    }

    /// Free staging slots remaining (capacity for more submits without flush).
    pub fn free_staging_count(&self) -> usize {
        self.scatter_pipeline
            .lock()
            .map(|p| p.free_stagings.len())
            .unwrap_or(0)
    }

    /// Block until oldest pending counters are ready (8-byte map only).
    pub fn flush_scatter_counter(&self) -> Option<(u32, u32)> {
        let shared = GpuContext::shared()?;
        let device = &shared.device;
        let started = std::time::Instant::now();
        loop {
            match self.poll_scatter_counter()? {
                Some(counts) => return Some(counts),
                None => {
                    if started.elapsed().as_millis() > 2_000 {
                        return None;
                    }
                    std::thread::yield_now();
                }
            }
        }
    }

    /// Drain every pending scatter map; returns the last `(batch_terminals, tile_completion)`.
    pub fn flush_all_scatter_counters(&self) -> Option<(u32, u32)> {
        let mut last = None;
        while self.has_pending_maps() {
            match self.flush_scatter_counter() {
                Some(counts) => last = Some(counts),
                None => break,
            }
        }
        last
    }

    /// Scatter terminals into atlas; returns `(batch_terminals, tile_completion)`.
    pub fn scatter_to_slot(
        &self,
        point_buffer: &wgpu::Buffer,
        atlas: &ProductionAtlas,
        slot: u32,
        local_seats: &[u32],
    ) -> Option<(u32, u32)> {
        if !self.scatter_submit(point_buffer, atlas, slot, local_seats) {
            return None;
        }
        self.flush_scatter_counter()
    }
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba32Float,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}
