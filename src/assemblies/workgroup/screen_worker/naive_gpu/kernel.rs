use super::buffers::*;
use super::device::{GpuPrecision, NaiveGpuContext, MAX_WAVE};
use crate::assemblies::workgroup::screen_worker::workshift::{BoutCap, Point, Step};
use bytemuck::{bytes_of, cast_slice, pod_read_unaligned};
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct WipMeta {
    pub index: usize,
    pub pos: (i32, i32),
    pub step: Step,
}

#[derive(Clone, Debug)]
pub struct HarvestedFinish {
    pub seat_index: u32,
    pub flags: u32,
    pub iterations: u32,
    pub small_time: u32,
    pub smallness: f64,
    pub iter_delta: u32,
    pub z_x: f64,
    pub z_y: f64,
    pub dc_x: f64,
    pub dc_y: f64,
    pub c_x: f64,
    pub c_y: f64,
}

impl NaiveGpuContext {
    pub fn dispatch_wave(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
    ) -> Result<(), String> {
        let wip_count = seats.len() as u32;
        if wip_count == 0 {
            return Ok(());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }

        // Reset counters.
        self.queue
            .write_buffer(&self.finish_count_buf, 0, &0u32.to_ne_bytes());
        self.queue
            .write_buffer(&self.iter_total_buf, 0, &0u32.to_ne_bytes());

        match self.precision {
            GpuPrecision::F32 => {
                let packed: Vec<SeatF32> = seats
                    .iter()
                    .map(|(i, p)| SeatF32::from_point(*i, p))
                    .collect();
                self.queue
                    .write_buffer(&self.seats_buf, 0, cast_slice(&packed));
                let params = ParamsF32 {
                    r_squared: r_squared as f32,
                    epsilon: epsilon as f32,
                    cap: cap.get(),
                    wip_count,
                    generation: self.generation,
                    _p0: 0,
                    _p1: 0,
                    _p2: 0,
                };
                self.queue
                    .write_buffer(&self.params_buf, 0, bytes_of(&params));
            }
            GpuPrecision::F64 => {
                let packed: Vec<SeatF64> = seats
                    .iter()
                    .map(|(i, p)| SeatF64::from_point(*i, p))
                    .collect();
                self.queue
                    .write_buffer(&self.seats_buf, 0, cast_slice(&packed));
                let params = ParamsF64 {
                    r_squared,
                    epsilon,
                    cap: cap.get(),
                    wip_count,
                    generation: self.generation,
                    _p0: 0,
                };
                self.queue
                    .write_buffer(&self.params_buf, 0, bytes_of(&params));
            }
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("naive_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.seats_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.finishes_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.finish_count_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.iter_total_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_enc"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("naive_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = (wip_count + 63) / 64;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        let finish_bytes = self.finish_stride * wip_count as u64;
        encoder.copy_buffer_to_buffer(&self.finishes_buf, 0, &self.finish_staging, 0, finish_bytes);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.count_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.iter_staging, 0, 4);
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    pub fn harvest_finishes(&self) -> Result<(Vec<HarvestedFinish>, u32), String> {
        let count = map_u32(&self.device, &self.count_staging)?;
        let iter_delta = map_u32(&self.device, &self.iter_staging)?;
        let n = count.min(MAX_WAVE) as usize;
        let mut out = Vec::with_capacity(n);
        match self.precision {
            GpuPrecision::F32 => {
                let bytes = map_bytes(
                    &self.device,
                    &self.finish_staging,
                    self.finish_stride * n as u64,
                )?;
                for i in 0..n {
                    let off = i * self.finish_stride as usize;
                    let fin: FinishF32 = pod_read_unaligned(&bytes[off..off + 48]);
                    out.push(HarvestedFinish {
                        seat_index: fin.seat_index,
                        flags: fin.flags,
                        iterations: fin.iterations,
                        small_time: fin.small_time,
                        smallness: fin.smallness as f64,
                        iter_delta: fin.iter_delta,
                        z_x: fin.z_x as f64,
                        z_y: fin.z_y as f64,
                        dc_x: fin.dc_x as f64,
                        dc_y: fin.dc_y as f64,
                        c_x: fin.c_x as f64,
                        c_y: fin.c_y as f64,
                    });
                }
            }
            GpuPrecision::F64 => {
                let bytes = map_bytes(
                    &self.device,
                    &self.finish_staging,
                    self.finish_stride * n as u64,
                )?;
                for i in 0..n {
                    let off = i * self.finish_stride as usize;
                    let fin: FinishF64 = pod_read_unaligned(&bytes[off..off + 80]);
                    out.push(HarvestedFinish {
                        seat_index: fin.seat_index,
                        flags: fin.flags,
                        iterations: fin.iterations,
                        small_time: fin.small_time,
                        smallness: fin.smallness,
                        iter_delta: fin.iter_delta,
                        z_x: fin.z_x,
                        z_y: fin.z_y,
                        dc_x: fin.dc_x,
                        dc_y: fin.dc_y,
                        c_x: fin.c_x,
                        c_y: fin.c_y,
                    });
                }
            }
        }
        Ok((out, iter_delta))
    }
}

fn map_u32(device: &wgpu::Device, buf: &wgpu::Buffer) -> Result<u32, String> {
    let bytes = map_bytes(device, buf, 4)?;
    Ok(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn map_bytes(device: &wgpu::Device, buf: &wgpu::Buffer, size: u64) -> Result<Vec<u8>, String> {
    let slice = buf.slice(0..size);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::Wait).map_err(|e| format!("poll: {e}"))?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("map failed: {e}"))?;
    let data = slice.get_mapped_range().to_vec();
    buf.unmap();
    Ok(data)
}
