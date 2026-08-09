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
        self.dispatch_wave_multi(seats, r_squared, epsilon, cap, 1)
    }

    /// Resident multi-bout; copies finishes+header. Seats staging optional via `with_seats`.
    pub fn dispatch_wave_multi(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
    ) -> Result<(), String> {
        self.dispatch_multi(seats, r_squared, epsilon, cap, bouts, true)
    }

    /// Hot path: no seats staging copy (partials stay on GPU; use harvest_sparse_finals).
    pub fn dispatch_wave_multi_sparse(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
    ) -> Result<(), String> {
        self.dispatch_multi(seats, r_squared, epsilon, cap, bouts, false)
    }

    pub fn dispatch_wave_multi_iters_only(
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
        self.upload_seats_and_params(seats, r_squared, epsilon, cap, wip_count)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_multi_iters"),
            });
        for _ in 0..bouts {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("naive_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((wip_count + 255) / 256, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        self.queue.submit(Some(encoder.finish()));
        self.last_wip_count
            .store(wip_count, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn dispatch_multi(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
        copy_seats: bool,
    ) -> Result<(), String> {
        let wip_count = seats.len() as u32;
        if wip_count == 0 || bouts == 0 {
            return Ok(());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        self.upload_seats_and_params(seats, r_squared, epsilon, cap, wip_count)?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_multi"),
            });
        for _ in 0..bouts {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("naive_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((wip_count + 255) / 256, 1, 1);
        }
        // Header + compact finish prefix in the same submit (no second GPU sync).
        // Header + finish records for the whole WIP (shallow home can finish all seats).
        let sparse_n = wip_count.min(MAX_WAVE);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.sparse_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.sparse_staging, 4, 4);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        if sparse_n > 0 {
            encoder.copy_buffer_to_buffer(
                &self.finishes_buf,
                0,
                &self.sparse_staging,
                16,
                self.finish_stride * sparse_n as u64,
            );
        }
        if copy_seats {
            encoder.copy_buffer_to_buffer(
                &self.seats_buf,
                0,
                &self.seat_staging,
                0,
                self.seat_stride * wip_count as u64,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.last_wip_count
            .store(wip_count, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn upload_seats_and_params(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        wip_count: u32,
    ) -> Result<(), String> {
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
        Ok(())
    }

    /// Continue resident seats without re-upload (finish counter accumulates).
    pub fn dispatch_continue_multi(
        &self,
        wip_count: u32,
        bouts: u32,
        copy_seats: bool,
    ) -> Result<(), String> {
        if wip_count == 0 || bouts == 0 {
            return Ok(());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_continue"),
            });
        for _ in 0..bouts {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("naive_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((wip_count + 255) / 256, 1, 1);
        }
        let sparse_n = wip_count.min(MAX_WAVE);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.sparse_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.sparse_staging, 4, 4);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        if sparse_n > 0 {
            encoder.copy_buffer_to_buffer(
                &self.finishes_buf,
                0,
                &self.sparse_staging,
                16,
                self.finish_stride * sparse_n as u64,
            );
        }
        if copy_seats {
            encoder.copy_buffer_to_buffer(
                &self.seats_buf,
                0,
                &self.seat_staging,
                0,
                self.seat_stride * wip_count as u64,
            );
        }
        self.queue.submit(Some(encoder.finish()));
        self.last_wip_count
            .store(wip_count, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    pub fn harvest_iters_only(&self) -> Result<u32, String> {
        let header = map_bytes(&self.device, &self.header_staging, 8)?;
        Ok(u32::from_ne_bytes([header[4], header[5], header[6], header[7]]))
    }

    /// Finals from the finish prefix copied with compute (up to full WIP).
    pub fn harvest_sparse_finals(&self) -> Result<(Vec<HarvestedFinish>, u32), String> {
        let header = map_bytes(&self.device, &self.header_staging, 8)?;
        let count = u32::from_ne_bytes([header[0], header[1], header[2], header[3]]);
        let iter_delta = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
        let n_fin = count.min(MAX_WAVE) as usize;
        if n_fin == 0 {
            return Ok((Vec::new(), iter_delta));
        }
        let finish_bytes = self.finish_stride * n_fin as u64;
        let bytes = map_bytes_offset(&self.device, &self.sparse_staging, 16, finish_bytes)?;
        let mut finals = Vec::with_capacity(n_fin);
        for i in 0..n_fin {
            let off = i * self.finish_stride as usize;
            match self.precision {
                GpuPrecision::F32 => {
                    finals.push(finish_from_f32(pod_read_unaligned(&bytes[off..off + 64])));
                }
                GpuPrecision::F64 => {
                    finals.push(finish_from_f64(pod_read_unaligned(&bytes[off..off + 96])));
                }
            }
        }
        Ok((finals, iter_delta))
    }

    /// Copy seats GPU→staging and map (no compute). Sync unfinished before re-upload.
    pub fn pull_seats(&self) -> Result<Vec<HarvestedFinish>, String> {
        let wip = self
            .last_wip_count
            .load(std::sync::atomic::Ordering::Relaxed)
            .min(MAX_WAVE);
        if wip == 0 {
            return Ok(Vec::new());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("naive_pull_seats"),
            });
        encoder.copy_buffer_to_buffer(
            &self.seats_buf,
            0,
            &self.seat_staging,
            0,
            self.seat_stride * wip as u64,
        );
        self.queue.submit(Some(encoder.finish()));
        let bytes = map_bytes(
            &self.device,
            &self.seat_staging,
            self.seat_stride * wip as u64,
        )?;
        Ok(parse_seats(
            self.precision,
            &bytes,
            self.seat_stride,
            wip as usize,
        ))
    }

    pub fn harvest_finishes(&self) -> Result<(Vec<HarvestedFinish>, Vec<HarvestedFinish>, u32), String> {
        let (finals, iters) = self.harvest_sparse_finals()?;
        let wip = self
            .last_wip_count
            .load(std::sync::atomic::Ordering::Relaxed)
            .min(MAX_WAVE) as usize;
        if wip == 0 {
            return Ok((finals, Vec::new(), iters));
        }
        let bytes = map_bytes(
            &self.device,
            &self.seat_staging,
            self.seat_stride * wip as u64,
        )?;
        let seats = parse_seats(self.precision, &bytes, self.seat_stride, wip);
        Ok((finals, seats, iters))
    }
}

fn finish_from_f32(fin: FinishF32) -> HarvestedFinish {
    HarvestedFinish {
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
    }
}

fn finish_from_f64(fin: FinishF64) -> HarvestedFinish {
    HarvestedFinish {
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
    }
}

fn parse_seats(
    precision: GpuPrecision,
    bytes: &[u8],
    stride: u64,
    n: usize,
) -> Vec<HarvestedFinish> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let off = i * stride as usize;
        match precision {
            GpuPrecision::F32 => {
                let s: SeatF32 = pod_read_unaligned(&bytes[off..off + 72]);
                out.push(HarvestedFinish {
                    seat_index: s.seat_index,
                    flags: s.flags,
                    iterations: s.iterations,
                    small_time: s.small_time,
                    smallness: s.smallness as f64,
                    iter_delta: 0,
                    z_x: s.z_x as f64,
                    z_y: s.z_y as f64,
                    dc_x: s.dc_x as f64,
                    dc_y: s.dc_y as f64,
                    c_x: s.c_x as f64,
                    c_y: s.c_y as f64,
                    loop_zx: s.loop_zx as f64,
                    loop_zy: s.loop_zy as f64,
                    loop_iter: s.loop_iter,
                });
            }
            GpuPrecision::F64 => {
                let s: SeatF64 = pod_read_unaligned(&bytes[off..off + 120]);
                out.push(HarvestedFinish {
                    seat_index: s.seat_index,
                    flags: s.flags,
                    iterations: s.iterations,
                    small_time: s.small_time,
                    smallness: s.smallness,
                    iter_delta: 0,
                    z_x: s.z_x,
                    z_y: s.z_y,
                    dc_x: s.dc_x,
                    dc_y: s.dc_y,
                    c_x: s.c_x,
                    c_y: s.c_y,
                    loop_zx: s.loop_zx,
                    loop_zy: s.loop_zy,
                    loop_iter: s.loop_iter,
                });
            }
        }
    }
    out
}

fn map_bytes(device: &wgpu::Device, buf: &wgpu::Buffer, size: u64) -> Result<Vec<u8>, String> {
    map_bytes_offset(device, buf, 0, size)
}

fn map_bytes_offset(
    device: &wgpu::Device,
    buf: &wgpu::Buffer,
    offset: u64,
    size: u64,
) -> Result<Vec<u8>, String> {
    let slice = buf.slice(offset..offset + size);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|e| format!("poll: {e}"))?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("map failed: {e}"))?;
    let data = slice.get_mapped_range().to_vec();
    buf.unmap();
    Ok(data)
}
