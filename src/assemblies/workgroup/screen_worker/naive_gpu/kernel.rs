use super::buffers::*;
use super::device::{GpuPrecision, NaiveGpuContext, MAX_WAVE};

/// Finish records copied in the compute submit (iterate-heavy waves stay ≤ this).
/// A second copy runs only when `count` exceeds it.
const FINISH_EAGER: u32 = 16;
use crate::assemblies::workgroup::screen_worker::workshift::{BoutCap, Point, Step};
use bytemuck::{bytes_of, pod_read_unaligned};
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
            .map(|_| ())
    }

    /// Hot path: no seats staging copy (partials stay on GPU; use harvest_sparse_finals).
    /// Returns the sparse-staging slot written (for pipelined harvest).
    pub fn dispatch_wave_multi_sparse(
        &self,
        seats: &[(u32, &Point<f64>)],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
    ) -> Result<u8, String> {
        self.dispatch_multi(seats, r_squared, epsilon, cap, bouts, false)
    }

    /// Upload from WIP metas + point store (no intermediate ref Vec).
    pub fn dispatch_wave_wip(
        &self,
        wip: &[WipMeta],
        points: &[Point<f64>],
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
        bouts: u32,
    ) -> Result<u8, String> {
        let wip_count = wip.len() as u32;
        if wip_count == 0 || bouts == 0 {
            return Ok(self.sparse_write.get());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        self.upload_wip_and_params(wip, points, r_squared, epsilon, cap, wip_count)?;
        self.encode_sparse_dispatch(wip_count, bouts, false)
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
    ) -> Result<u8, String> {
        let wip_count = seats.len() as u32;
        if wip_count == 0 || bouts == 0 {
            return Ok(self.sparse_write.get());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        self.upload_seats_and_params(seats, r_squared, epsilon, cap, wip_count)?;
        self.encode_sparse_dispatch(wip_count, bouts, copy_seats)
    }

    fn encode_sparse_dispatch(
        &self,
        wip_count: u32,
        bouts: u32,
        copy_seats: bool,
    ) -> Result<u8, String> {
        let slot = self.sparse_write.get() & 1;
        let staging = &self.sparse_staging[slot as usize];
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
        self.copy_sparse_header_and_eager(&mut encoder, staging, wip_count);
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
        self.sparse_wip[slot as usize].set(wip_count);
        self.sparse_write.set(1 - slot);
        Ok(slot)
    }

    fn upload_wip_and_params(
        &self,
        wip: &[WipMeta],
        points: &[Point<f64>],
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
                let nbytes = (wip.len() * std::mem::size_of::<SeatF32>()) as u64;
                if let Some(size) = std::num::NonZeroU64::new(nbytes) {
                    let mut view = self
                        .queue
                        .write_buffer_with(&self.seats_buf, 0, size)
                        .ok_or_else(|| "write_buffer_with seats f32 failed".to_string())?;
                    let out: &mut [SeatF32] = bytemuck::cast_slice_mut(view.as_mut());
                    for (dst, m) in out.iter_mut().zip(wip.iter()) {
                        *dst = SeatF32::from_point(m.index as u32, &points[m.index]);
                    }
                }
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
                let nbytes = (wip.len() * std::mem::size_of::<SeatF64>()) as u64;
                if let Some(size) = std::num::NonZeroU64::new(nbytes) {
                    let mut view = self
                        .queue
                        .write_buffer_with(&self.seats_buf, 0, size)
                        .ok_or_else(|| "write_buffer_with seats f64 failed".to_string())?;
                    let out: &mut [SeatF64] = bytemuck::cast_slice_mut(view.as_mut());
                    for (dst, m) in out.iter_mut().zip(wip.iter()) {
                        *dst = SeatF64::from_point(m.index as u32, &points[m.index]);
                    }
                }
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
                let nbytes = (seats.len() * std::mem::size_of::<SeatF32>()) as u64;
                if let Some(size) = std::num::NonZeroU64::new(nbytes) {
                    let mut view = self
                        .queue
                        .write_buffer_with(&self.seats_buf, 0, size)
                        .ok_or_else(|| "write_buffer_with seats f32 failed".to_string())?;
                    let out: &mut [SeatF32] = bytemuck::cast_slice_mut(view.as_mut());
                    for (dst, (i, p)) in out.iter_mut().zip(seats.iter()) {
                        *dst = SeatF32::from_point(*i, p);
                    }
                }
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
                let nbytes = (seats.len() * std::mem::size_of::<SeatF64>()) as u64;
                if let Some(size) = std::num::NonZeroU64::new(nbytes) {
                    let mut view = self
                        .queue
                        .write_buffer_with(&self.seats_buf, 0, size)
                        .ok_or_else(|| "write_buffer_with seats f64 failed".to_string())?;
                    let out: &mut [SeatF64] = bytemuck::cast_slice_mut(view.as_mut());
                    for (dst, (i, p)) in out.iter_mut().zip(seats.iter()) {
                        *dst = SeatF64::from_point(*i, p);
                    }
                }
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
    /// Returns sparse-staging slot written.
    pub fn dispatch_continue_multi(
        &self,
        wip_count: u32,
        bouts: u32,
        copy_seats: bool,
    ) -> Result<u8, String> {
        if wip_count == 0 || bouts == 0 {
            return Ok(self.sparse_write.get());
        }
        if wip_count > MAX_WAVE {
            return Err(format!("WIP {wip_count} exceeds MAX_WAVE"));
        }
        let slot = self.sparse_write.get() & 1;
        let staging = &self.sparse_staging[slot as usize];
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
        self.copy_sparse_header_and_eager(&mut encoder, staging, wip_count);
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
        self.sparse_wip[slot as usize].set(wip_count);
        self.sparse_write.set(1 - slot);
        Ok(slot)
    }

    fn copy_sparse_header_and_eager(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        staging: &wgpu::Buffer,
        wip_count: u32,
    ) {
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, staging, 4, 4);
        encoder.copy_buffer_to_buffer(&self.finish_count_buf, 0, &self.header_staging, 0, 4);
        encoder.copy_buffer_to_buffer(&self.iter_total_buf, 0, &self.header_staging, 4, 4);
        let eager = wip_count.min(MAX_WAVE).min(FINISH_EAGER);
        if eager > 0 {
            encoder.copy_buffer_to_buffer(
                &self.finishes_buf,
                0,
                staging,
                16,
                self.finish_stride * eager as u64,
            );
        }
    }

    pub fn harvest_iters_only(&self) -> Result<u32, String> {
        let header = map_bytes(&self.device, &self.header_staging, 8)?;
        Ok(u32::from_ne_bytes([header[4], header[5], header[6], header[7]]))
    }

    /// Finals from a sparse-staging slot. Map the 16-byte header first, then only
    /// the compact finish prefix (`count` records) — mapping `wip` empty slots
    /// made iterate-heavy fullstack lag compute (~0.60 track vs ≥0.80).
    pub fn harvest_sparse_slot(&self, slot: u8) -> Result<(Vec<HarvestedFinish>, u32), String> {
        let slot = (slot & 1) as usize;
        let wip = self.sparse_wip[slot].get().min(MAX_WAVE);
        let buf = &self.sparse_staging[slot];
        let header = map_bytes_offset(&self.device, buf, 0, 16)?;
        let count = u32::from_ne_bytes([header[0], header[1], header[2], header[3]]);
        let iter_delta = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
        let n_fin = count.min(wip).min(MAX_WAVE) as usize;
        if n_fin == 0 {
            return Ok((Vec::new(), iter_delta));
        }
        let stride = self.finish_stride;
        if n_fin as u32 > FINISH_EAGER {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("naive_compact_finishes"),
                });
            encoder.copy_buffer_to_buffer(
                &self.finishes_buf,
                0,
                buf,
                16,
                stride * n_fin as u64,
            );
            self.queue.submit(Some(encoder.finish()));
        }
        let mapped = map_bytes_offset(&self.device, buf, 16, stride * n_fin as u64)?;
        let stride = stride as usize;
        let mut finals = Vec::with_capacity(n_fin);
        for i in 0..n_fin {
            let off = i * stride;
            match self.precision {
                GpuPrecision::F32 => {
                    finals.push(finish_from_f32(pod_read_unaligned(&mapped[off..off + 64])));
                }
                GpuPrecision::F64 => {
                    finals.push(finish_from_f64(pod_read_unaligned(&mapped[off..off + 96])));
                }
            }
        }
        Ok((finals, iter_delta))
    }

    /// Finals from the most recently written sparse slot (non-pipelined callers).
    pub fn harvest_sparse_finals(&self) -> Result<(Vec<HarvestedFinish>, u32), String> {
        let slot = 1 - (self.sparse_write.get() & 1);
        self.harvest_sparse_slot(slot)
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
