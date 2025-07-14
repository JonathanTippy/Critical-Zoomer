use steady_state::*;
use eframe::{egui, NativeOptions};
//use eframe::Frame::raw_window_handle;
use egui_extras;
use winit::platform::x11::EventLoopBuilderExtX11; // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use winit::event_loop::EventLoopBuilder;
use egui::{Color32, ColorImage, TextureHandle, Vec2, Pos2, ViewportInfo, ViewportId, ViewportBuilder, WindowLevel};
use winit::raw_window_handle::HasWindowHandle;
use winit::dpi::PhysicalPosition;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};




use crate::actor::computer::*;
use crate::operation::sampling::*;
use crate::actor::updater::*;
use crate::actor::window::DEFAULT_WINDOW_RES;

const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = true;

pub(crate) const DEFAULT_SETTINGS_WINDOW_RES:(u32, u32) = (800, 480);

pub const DEFAULT_SETTINGS:ZoomerSettingsState = ZoomerSettingsState{};

pub const DEFAULT_SETTINGS_WINDOW_STATE:SettingsWindowState = SettingsWindowState{
    settings: DEFAULT_SETTINGS
    , size: egui::vec2(DEFAULT_SETTINGS_WINDOW_RES.0 as f32, DEFAULT_SETTINGS_WINDOW_RES.1 as f32)
    , location: None
    , will_close: false
};


#[derive(Debug)]
struct EguiWindowError {
    state: SettingsWindowState
}

impl fmt::Display for EguiWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "egui window stopped unexpectedly")
    }
}
impl Error for EguiWindowError {}

/// State struct for the window actor.

#[derive(Clone, Debug)]
pub(crate) struct ZoomerSettingsState {
}

#[derive(Clone, Debug)]
pub(crate) struct ZoomerSettingsUpdate {
}

enum ZoomerSetting {
    Favorite_Color{
        color: (u8,u8,u8)
    },
    Controls_Settings{
        preset: ControlsSettings
    }
}
#[derive(Clone, Debug)]
pub(crate) enum ControlsSettings {
    H
}

pub(crate) struct SettingsWindowResult {
    pub(crate) will_close: bool,
    pub(crate) control_settings: ControlsSettings,
    pub(crate) settings_update:ZoomerSettingsUpdate
}


#[derive(Clone, Debug)]
pub(crate) struct SettingsWindowState {
    pub(crate) settings: ZoomerSettingsState
    , pub(crate) size: Vec2
    , pub(crate) location: Option<Pos2>
    , pub(crate) will_close: bool
}


pub(crate) fn settings (
    ctx: &egui::Context,
    state: Arc<Mutex<SettingsWindowState>>,
) -> SettingsWindowResult {

    let state1 = state.clone();
    let state2 = state.clone();

    let mut state = state.try_lock().unwrap();


    let viewport_options =
        egui::ViewportBuilder::default()
            .with_inner_size(state.size.clone())
        ;

    let mut viewport_options = match state.location {
        Some(l) => {viewport_options.with_position(l)}
        None => {viewport_options}
    };

    drop(state);

    ctx.show_viewport_deferred(
        ViewportId::from_hash_of("my_viewport"),
        viewport_options
            .with_title("Deferred Viewport")
            .with_inner_size([300.0, 200.0])

            .with_window_level(WindowLevel::AlwaysOnTop),
        move |ctx, class| {


            let mut state = state1.try_lock().unwrap();


            egui::CentralPanel::default().show(ctx, |ui| {

                ui.visuals_mut().override_text_color = Some(Color32::WHITE);

                let available_size = ui.available_size();

                //let mut state = portable_state.lock().unwrap();

                ui.label("This is a deferred viewport!");

            });


            ctx.input(|input_state| {

                match input_state.raw.viewports.get(&ViewportId::from_hash_of("my_viewport")) {
                    Some(info) => {

                        match info.outer_rect {
                            Some(r) => { state.location = Some(r.min); }
                            None => {}
                        }
                        match info.inner_rect {
                            Some(r) => { state.size = r.size();}
                            None => {}
                        }
                        for viewport_event in info.events.clone() {
                            match viewport_event {
                                egui::ViewportEvent::Close => {
                                    info!("settings window should close");
                                    state.will_close = true;
                                }
                            }
                        }
                    }
                    None => {}
                }
            });
        },
    );

    let state = state2.try_lock().unwrap();

    info!("will close: {}", state.will_close);

    SettingsWindowResult{
        will_close: state.will_close,
        control_settings: ControlsSettings::H,
        settings_update: ZoomerSettingsUpdate{}
    }
}