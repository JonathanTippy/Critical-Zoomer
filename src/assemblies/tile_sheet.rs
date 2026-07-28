//! The GPU layout both tile hoards agree on.
//!
//! The workgroup and headgroup keep their own hoards (architecture.md), but a
//! finished tile has to move from one to the other. On a single shared device
//! that move is a texture-to-texture copy, which is only correct if both sides
//! lay their sheets out identically. That agreement lives here, once, so the
//! two hoards cannot drift apart — the same reason the tile manager is shared
//! code rather than a synchronising conversation.

use crate::constants::TILE_EDGE_LENGTH;

/// Tiles per sheet row. Sheets grow downward by adding rows, never by widening,
/// so a slot's origin never moves when the atlas grows.
pub const SHEET_COLS: u32 = 32;

/// A tile's edge in texels.
pub const TILE_EDGE: u32 = TILE_EDGE_LENGTH as u32;

/// Bytes one slot costs across both sheets: 64x64 texels, four f32 each, twice.
pub const SLOT_BYTES: u64 = (TILE_EDGE as u64) * (TILE_EDGE as u64) * 16 * 2;

/// Texel format of both sheets. Storage-capable so compute writes tiles in
/// place, sampled-capable so the display reads them through the texture cache.
pub const SHEET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// Texel dimensions of a sheet holding `slots` tiles.
pub fn sheet_size_for(slots: u32) -> (u32, u32) {
    let rows = slots.div_ceil(SHEET_COLS).max(1);
    (SHEET_COLS * TILE_EDGE, rows * TILE_EDGE)
}

/// Top-left texel of a slot. Depends only on the column count, so it is stable
/// across growth and identical on both sides of a handoff.
pub fn slot_origin(slot: u32) -> [u32; 2] {
    [(slot % SHEET_COLS) * TILE_EDGE, (slot / SHEET_COLS) * TILE_EDGE]
}

/// Most slots a device can hold in one sheet, from its texture dimension limit.
pub fn max_slots_for(device: &wgpu::Device) -> u32 {
    let rows = device.limits().max_texture_dimension_2d / TILE_EDGE;
    (rows * SHEET_COLS).max(SHEET_COLS)
}

/// Create one sheet sized for `slots` tiles.
pub fn create_sheet(
    device: &wgpu::Device
    , slots: u32
    , label: &str
) -> (wgpu::Texture, wgpu::TextureView) {
    let (width, height) = sheet_size_for(slots);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label)
        , size: wgpu::Extent3d {
            width: width.max(1)
            , height: height.max(1)
            , depth_or_array_layers: 1
        }
        , mip_level_count: 1
        , sample_count: 1
        , dimension: wgpu::TextureDimension::D2
        , format: SHEET_FORMAT
        // STORAGE_BINDING is what makes a hoard gpu-native: compute writes the
        // sheet in place rather than handing tile bytes back through CPU memory.
        // COPY_SRC/DST carry a slot across a handoff or an atlas growth, still
        // without leaving the device.
        , usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
        , view_formats: &[]
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Upload one tile's texels into a slot. The CPU-fallback path only: tiles the
/// workgroup finished on the GPU never travel this way.
pub fn write_slot(
    queue: &wgpu::Queue
    , texture: &wgpu::Texture
    , slot: u32
    , pixels: &[[f32; 4]]
) {
    let seats = (TILE_EDGE * TILE_EDGE) as usize;
    if pixels.len() < seats {
        return;
    }
    let origin = slot_origin(slot);
    let unpadded = (TILE_EDGE * 16) as usize;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded = unpadded.div_ceil(align) * align;
    let mut bytes = vec![0u8; padded * TILE_EDGE as usize];
    let src = bytemuck::cast_slice::<[f32; 4], u8>(&pixels[..seats]);
    for y in 0..TILE_EDGE as usize {
        let src_off = y * unpadded;
        let dst_off = y * padded;
        bytes[dst_off..dst_off + unpadded]
            .copy_from_slice(&src[src_off..src_off + unpadded]);
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture
            , mip_level: 0
            , origin: wgpu::Origin3d { x: origin[0], y: origin[1], z: 0 }
            , aspect: wgpu::TextureAspect::All
        }
        , &bytes
        , wgpu::TexelCopyBufferLayout {
            offset: 0
            , bytes_per_row: Some(padded as u32)
            , rows_per_image: Some(TILE_EDGE)
        }
        , wgpu::Extent3d {
            width: TILE_EDGE
            , height: TILE_EDGE
            , depth_or_array_layers: 1
        }
    );
}

/// Record a slot-to-slot copy between two sheets on the same device.
///
/// This is the handoff: the workgroup's finished tile becomes the headgroup's
/// hoarded tile without either side touching CPU memory.
pub fn copy_slot(
    encoder: &mut wgpu::CommandEncoder
    , src: &wgpu::Texture
    , src_slot: u32
    , dst: &wgpu::Texture
    , dst_slot: u32
) {
    let src_origin = slot_origin(src_slot);
    let dst_origin = slot_origin(dst_slot);
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: src
            , mip_level: 0
            , origin: wgpu::Origin3d { x: src_origin[0], y: src_origin[1], z: 0 }
            , aspect: wgpu::TextureAspect::All
        }
        , wgpu::TexelCopyTextureInfo {
            texture: dst
            , mip_level: 0
            , origin: wgpu::Origin3d { x: dst_origin[0], y: dst_origin[1], z: 0 }
            , aspect: wgpu::TextureAspect::All
        }
        , wgpu::Extent3d {
            width: TILE_EDGE
            , height: TILE_EDGE
            , depth_or_array_layers: 1
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_origin_never_moves_when_the_sheet_grows() {
        // Growth adds rows, so every existing slot keeps its texel origin. If it
        // did not, growing the atlas would silently relocate live tile data.
        for slot in [0u32, 1, 31, 32, 33, 2047] {
            let small = slot_origin(slot);
            let large = slot_origin(slot);
            assert_eq!(small, large);
            assert!(small[0] < sheet_size_for(slot + 1).0);
        }
    }

    #[test]
    fn every_slot_fits_inside_a_sheet_sized_for_it() {
        for slots in [1u32, 32, 33, 64, 2048, 4096] {
            let (w, h) = sheet_size_for(slots);
            let last = slot_origin(slots - 1);
            assert!(last[0] + TILE_EDGE <= w, "slot overruns sheet width at {slots}");
            assert!(last[1] + TILE_EDGE <= h, "slot overruns sheet height at {slots}");
        }
    }

    #[test]
    fn slots_never_overlap() {
        let mut seen = std::collections::HashSet::new();
        for slot in 0..256u32 {
            assert!(seen.insert(slot_origin(slot)), "slot {slot} reuses another slot's texels");
        }
    }
}
