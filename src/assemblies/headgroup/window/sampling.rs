use egui::{Color32, Pos2};
use std::cmp::*;

use crate::utils::*;

use crate::assemblies::structs::*;
pub enum ZoomerCommand {
    SetFocus { pixel_x: u32, pixel_y: u32 }
    ,
    SetZoom { pot: i32 }
    ,
    Zoom { pot: i32, center_screenspace_pos: (i32, i32) } // zoom in or out
    ,
    Move { pixels_x: IntExp, pixels_y: IntExp }
    ,
    MoveTo { x: IntExp, y: IntExp }
    ,
    SetPos { real: IntExp, imag: IntExp }
    ,
    TrackPoint { point_id: u64, point_real: IntExp, point_imag: IntExp }
    ,
    UntrackPoint { point_id: u64 }
    ,
    UntrackAllPoints
}
pub const NUMBER_OF_COMMANDS: u16 = 10;

#[derive(Clone, Debug)]
pub struct SamplingContext {
    pub screen: Option<View<Color32>>
    , pub screen_size: (u32, u32)
    , pub location: ObjectivePosAndZoom
    , pub updated: bool
    , pub mouse_drag_start: Option<(ObjectivePosAndZoom, Pos2)>
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewportLocation {
    pub pos: (i32, i32) // This is objective
    , pub zoom_pot: i32
    , pub counter: u64
}

pub fn resample(
    sampling_context: &mut SamplingContext
) -> Vec<Color32> {
    //let bucket = output_buffer;
    let context = sampling_context;

    let viewport_stencil = PointStencil {
        location: (
            context.location.pos.0.clone()
            , IntExp::ZERO-context.location.pos.1.clone()
            , context.location.zoom_pot
        )
        , resolution: (context.screen_size.0 as usize, context.screen_size.1 as usize)
        , serial_number: 0
    }.correct_precision();


    let mut viewport_view = View::new(viewport_stencil, Color32::BLACK);

    if let Some(source) = &context.screen {
        viewport_view.fill_from(source)
    }
    viewport_view.data
}

pub fn update_sampling_context(context: &mut SamplingContext, screen: View<Color32>) {

    let l = ObjectivePosAndZoom {
        pos: (screen.stencil.clone().location.0, IntExp::ZERO-screen.stencil.clone().location.1)
        , zoom_pot: screen.stencil.clone().location.2
    };

    if context.location == l {
        context.updated = false;
    }
    
    /*if let Some(old_screen) = context.screen.take() {
        drop(old_screen);
    }*/
    context.screen = Some(View{
        data: screen.data
        , bitmap: screen.bitmap
,         stencil: PointStencil{
            location:(screen.stencil.location.0, screen.stencil.location.1, screen.stencil.location.2)
            , resolution: screen.stencil.resolution
            , serial_number: screen.stencil.serial_number
        }
    });

}