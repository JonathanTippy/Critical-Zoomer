use steady_state::*;

use crate::actor::window::*;
use crate::actor::computer::*;


const DEFAULT_AVERAGE_COLOR:(u8,u8,u8) = (255, 0, 255);

pub(crate) struct ZoomerScreen {
    real_center: String
    , imag_center: String
    , zoom: String
    , screen_height: u32
    , screen_width: u32
}


pub(crate) struct PixelsForWindow {
    pub(crate) pixels: Vec<(u8, u8, u8)>
    , pub(crate) report: Option<ZoomerReport>
}

pub(crate) struct ZoomedScreen {
    pixels: Vec<(u8, u8, u8)>
    , zoom_power: i32
}


#[derive(Clone, Debug)]
pub(crate) struct SamplingState {
    pub(crate) average_color: (u8, u8, u8),
    pub(crate) screen_collection: Vec<Vec<(u8, u8, u8)>>,
    pub(crate) viewport_position_real: String,
    pub(crate) viewport_position_imag: String,
    pub(crate) viewport_zoom: String,
    pub(crate) zoom_power_base: u8,
    pub(crate) window_res: (u32, u32)
}

pub(crate) fn sample(
    mut command_package: ZoomerCommandPackage,
    mut state: &mut Option<SamplingState>
) -> PixelsForWindow {

    // initialize state

    match &mut state {
        Some(_) => {}
        None => {
            *state = Some(SamplingState {
                average_color: DEFAULT_AVERAGE_COLOR,
                screen_collection: vec!(),
                viewport_position_real: String::from("0"),
                viewport_position_imag: String::from("0"),
                viewport_zoom: String::from("1"),
                zoom_power_base: 2,
                window_res: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
            });
        }
    };

    let mut state = state.as_mut().unwrap();

    // handle commands

    for command in &mut command_package.commands {
        match command {
            ZoomerCommand::SetAttention{pixel_x, pixel_y} => {
                // send the jobs and stuff
            }
            ZoomerCommand::SetRes{hori, verti} => {
                state.window_res = (*hori, *verti);
            }
            ZoomerCommand::ZoomClean{factor_power} => {
            }
            ZoomerCommand::SetZoomPowerBase{base} => {
            }
            ZoomerCommand::ZoomUnclean{factor} => {
            }
            ZoomerCommand::SetZoom{factor} => {
            }
            ZoomerCommand::MoveClean{pixels_x, pixels_y} => {
            }
            ZoomerCommand::SetPos{real, imag} => {
            }
            ZoomerCommand::TrackPoint{point_id, point_real, point_imag} => {
            }
            ZoomerCommand::UntrackPoint{point_id} => {
            }
            ZoomerCommand::UntrackAllPoints{} => {
            }
        }
    }

    // sample frame
    //bucket.into_iter().for_each(|p| p = state.average_color);

    for mut i in 0..command_package.bucket[0].len() {
        command_package.bucket[0][i] = state.average_color;
    }

    // send frame
    PixelsForWindow{
        pixels: command_package.bucket.pop().unwrap(),
        report: None /*Some(
            ZoomerReport{
                actor_start: actor_start,
                actor_wake: actor_wake,
                time_to_xyz: Vec::from([
                    (String::from("time to notice command:"), noticing_time)
                    , (String::from("time to process command:"), my_code_start.elapsed())
                ])
            }
        )*/
    }
}