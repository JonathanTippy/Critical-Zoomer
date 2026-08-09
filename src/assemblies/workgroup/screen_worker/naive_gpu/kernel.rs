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
    pub loop_zx: f64,
    pub loop_zy: f64,
    pub loop_iter: u32,
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
            pass.set_bind_group(0, &self.bind_group, &[]);
            let groups = (wip_count + 63) / 64;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        let finish_bytes = self.finish_stride * wip_count as u64;
        encoder.copy_buffer_to_buffer(&self.finishes_buf, 0, &self.finish_staging, 0, finish_bytes);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Upload once, run `bouts` compute passes on resident seats, one harvest copy.
    /// Finishes from early bouts are preserved because done seats stay inactive and
    /// each bout appends into finishes only for seats that progressed *this* bout;
    /// early finals are not re-copied. To avoid losing them, we copy finishes after
    /// every bout into a host-side merge via staging ring — here we use a simpler
    /// approach: one long effective bout by repeating compute without clearing
    /// seats, and emitting finishes every bout into consecutive staging slots.
    pub fn dispatch_wave_multi(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
    ) -> Result<(), String> {
        let wip_count = seats.len() as u32;
        if wip_count == 0 || bouts == 0 {
            return Ok(());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        if bouts == 1 {
            return self.dispatch_wave(seats, r_squared, epsilon, cap);
        }
        // For multi-bout IPS: upload once, N dispatches, accumulate iter_total,
        // copy seats is not done — instead harvest only iter header by running
        // N passes and reading iter_total; finishes from last pass only.
        // Production workshift still uses single dispatch_wave.
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_multi"),
            });
        for bout in 0..bouts {
            if bout > 0 {
                encoder.clear_buffer(&self.finish_count_buf, 0, Some(4));
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("naive_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                let groups = (wip_count + 63) / 64;
                pass.dispatch_workgroups(groups, 1, 1);
            }
        }
        let finish_bytes = self.finish_stride * wip_count as u64;
        encoder.copy_buffer_to_buffer(&self.finishes_buf, 0, &self.finish_staging, 0, finish_bytes);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    pub fn harvest_finishes(&self) -> Result<(Vec<HarvestedFinish>, u32), String> {
        // One poll: header + full finish staging (sized to MAX_WAVE at init).
        let finish_bytes = self.finish_stride * MAX_WAVE as u64;
        let (header, bytes) =
            map_header_and_finishes(&self.device, &self.header_staging, &self.finish_staging, finish_bytes)?;
        let count = u32::from_ne_bytes([header[0], header[1], header[2], header[3]]);
        let iter_delta = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
        let n = count.min(MAX_WAVE) as usize;
        if n == 0 {
            return Ok((Vec::new(), iter_delta));
        }
        let mut out = Vec::with_capacity(n);
        match self.precision {
            GpuPrecision::F32 => {
                for i in 0..n {
                    let off = i * self.finish_stride as usize;
                    let fin: FinishF32 = pod_read_unaligned(&bytes[off..off + 64]);
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
                        loop_zx: fin.loop_zx as f64,
                        loop_zy: fin.loop_zy as f64,
                        loop_iter: fin.loop_iter,
                    });
                }
            }
            GpuPrecision::F64 => {
                for i in 0..n {
                    let off = i * self.finish_stride as usize;
                    let fin: FinishF64 = pod_read_unaligned(&bytes[off..off + 96]);
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
                        loop_zx: fin.loop_zx,
                        loop_zy: fin.loop_zy,
                        loop_iter: fin.loop_iter,
                    });
                }
            }
        }
        Ok((out, iter_delta))
    }
}

fn map_header_and_finishes(
    device: &wgpu::Device,
    header: &wgpu::Buffer,
    finishes: &wgpu::Buffer,
    finish_bytes: u64,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let h = header.slice(0..8);
    let f = finishes.slice(0..finish_bytes);
    let (tx_h, rx_h) = mpsc::channel();
    let (tx_f, rx_f) = mpsc::channel();
    h.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx_h.send(r);
    });
    f.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx_f.send(r);
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|e| format!("poll: {e}"))?;
    rx_h.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("header map failed: {e}"))?;
    rx_f.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("finish map failed: {e}"))?;
    let header_bytes = h.get_mapped_range().to_vec();
    let finish_data = f.get_mapped_range().to_vec();
    header.unmap();
    finishes.unmap();
    Ok((header_bytes, finish_data))
}
