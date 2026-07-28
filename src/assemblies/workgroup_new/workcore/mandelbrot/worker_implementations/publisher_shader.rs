//! GPU publisher compute: bounds disproof, biased-nearest clamp, NORES.
//! docs/design/tile_publisher.md
// r[impl cz.int.publisher-nores-bias+1]
// r[impl cz.range.guess-biased-nearest+1]

use bytemuck::{Pod, Zeroable};
use crate::assemblies::structs::gpu_tile::GPUCalibratedAnswer;
use crate::constants::TILE_EDGE_LENGTH;
use crate::gpu_context::GpuContext;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuPackedAnswer {
    pub kind: u32,
    pub escape_or_period: u32,
    pub min_mag_time: u32,
    pub min_mag: f32,
    pub zx: f32,
    pub zy: f32,
    pub _pad0: u32,
    pub _pad1: u32,
}

pub struct PublisherGpu {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    cal_buf: wgpu::Buffer,
    bias_buf: wgpu::Buffer,
    valid_buf: wgpu::Buffer,
    out_buf: wgpu::Buffer,
    staging: wgpu::Buffer,
    seat_count: u32,
}

impl PublisherGpu {
    pub fn new() -> Option<Self> {
        let shared = GpuContext::shared()?;
        let device = &shared.device;
        let seats = (TILE_EDGE_LENGTH * TILE_EDGE_LENGTH) as u32;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("publisher"),
            source: wgpu::ShaderSource::Wgsl(include_str!("publisher.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("publisher_bgl"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("publisher_pll"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("publisher_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("publish"),
            compilation_options: Default::default(),
            cache: None,
        });
        let cal_bytes = seats as u64 * std::mem::size_of::<GPUCalibratedAnswer>() as u64;
        let ans_bytes = seats as u64 * std::mem::size_of::<GpuPackedAnswer>() as u64;
        let valid_bytes = seats as u64 * 4;
        let cal_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("publisher_cal"),
            size: cal_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bias_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("publisher_bias"),
            size: ans_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let valid_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("publisher_valid"),
            size: valid_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("publisher_out"),
            size: ans_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("publisher_staging"),
            size: ans_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            pipeline,
            bind_group_layout,
            cal_buf,
            bias_buf,
            valid_buf,
            out_buf,
            staging,
            seat_count: seats,
        })
    }

    /// Dispatch publish for one tile; returns packed answers (CPU readback for now).
    pub fn publish_tile(
        &self
        , calibrated: &[GPUCalibratedAnswer]
        , bias: &[GpuPackedAnswer]
        , bias_valid: &[u32]
    ) -> Option<Vec<GpuPackedAnswer>> {
        let shared = GpuContext::shared()?;
        let n = self.seat_count as usize;
        if calibrated.len() != n || bias.len() != n || bias_valid.len() != n {
            return None;
        }
        shared.queue.write_buffer(&self.cal_buf, 0, bytemuck::cast_slice(calibrated));
        shared.queue.write_buffer(&self.bias_buf, 0, bytemuck::cast_slice(bias));
        shared.queue.write_buffer(&self.valid_buf, 0, bytemuck::cast_slice(bias_valid));
        let bind_group = shared.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("publisher_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.cal_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.bias_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.valid_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.out_buf.as_entire_binding() },
            ],
        });
        let mut enc = shared.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("publisher_enc"),
        });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("publisher_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(8, 8, 1);
        }
        let ans_bytes = n as u64 * std::mem::size_of::<GpuPackedAnswer>() as u64;
        enc.copy_buffer_to_buffer(&self.out_buf, 0, &self.staging, 0, ans_bytes);
        shared.queue.submit(Some(enc.finish()));
        let slice = self.staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });
        let _ = shared.device.poll(wgpu::PollType::Wait);
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range();
        let out: Vec<GpuPackedAnswer> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        self.staging.unmap();
        Some(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::structs::gpu_tile::{
        GPU_CAL_KIND_AGNOSTIC, GPU_CAL_KIND_OUTSIDE, GPUCalibratedAnswer,
    };

    #[test]
    fn publisher_shader_source_loads() {
        let src = include_str!("publisher.wgsl");
        assert!(src.contains("fn publish"));
        assert!(src.contains("nores"));
        assert!(src.contains("clamp_biased"));
    }

    #[test]
    fn publisher_gpu_nores_when_agnostic_without_bias() {
        let Some(gpu) = PublisherGpu::new() else {
            return;
        };
        let n = (TILE_EDGE_LENGTH * TILE_EDGE_LENGTH) as usize;
        let mut cal = vec![GPUCalibratedAnswer::EMPTY; n];
        cal[0].kind = GPU_CAL_KIND_AGNOSTIC;
        let bias = vec![GpuPackedAnswer::zeroed(); n];
        let valid = vec![0u32; n];
        let out = gpu.publish_tile(&cal, &bias, &valid).expect("dispatch");
        assert_eq!(out[0].kind, 1, "NORES is Outside");
        assert_eq!(out[0].escape_or_period, 1);
    }

    #[test]
    fn publisher_gpu_clamps_outside_toward_bias() {
        let Some(gpu) = PublisherGpu::new() else {
            return;
        };
        let n = (TILE_EDGE_LENGTH * TILE_EDGE_LENGTH) as usize;
        let mut cal = vec![GPUCalibratedAnswer::EMPTY; n];
        cal[0] = GPUCalibratedAnswer {
            kind: GPU_CAL_KIND_OUTSIDE,
            period_lo: 0,
            period_hi: 0,
            escape_lo: 10,
            escape_hi: 20,
            escape_z_re_lo: 1.0,
            escape_z_re_hi: 3.0,
            escape_z_im_lo: -1.0,
            escape_z_im_hi: 1.0,
            min_mag_time_lo: 0,
            min_mag_time_hi: 5,
            min_mag_lo: 0.1,
            min_mag_hi: 0.9,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        let mut bias = vec![GpuPackedAnswer::zeroed(); n];
        bias[0] = GpuPackedAnswer {
            kind: 1,
            escape_or_period: 100, // above hi → clamp to 20
            min_mag_time: 2,
            min_mag: 0.5,
            zx: 2.0,
            zy: 0.0,
            _pad0: 0,
            _pad1: 0,
        };
        let mut valid = vec![0u32; n];
        valid[0] = 1;
        let out = gpu.publish_tile(&cal, &bias, &valid).expect("dispatch");
        assert_eq!(out[0].escape_or_period, 20);
        assert_eq!(out[0].zx, 2.0);
    }
}
