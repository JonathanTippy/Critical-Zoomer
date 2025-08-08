use steady_state::*;
use eframe::egui;
//use eframe::Frame::raw_window_handle;
 // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use egui::{Color32, Vec2, Pos2, ViewportId, WindowLevel};
use std::sync::{Arc, Mutex};





const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = true;

pub(crate) const DEFAULT_SETTINGS_WINDOW_RES:(u32, u32) = (300, 200);

pub const DEFAULT_SETTINGS:SettingsState = SettingsState{};

pub const DEFAULT_SETTINGS_WINDOW_CONTEXT:SettingsWindowContext = SettingsWindowContext{
    settings: DEFAULT_SETTINGS
    , size: egui::vec2(DEFAULT_SETTINGS_WINDOW_RES.0 as f32, DEFAULT_SETTINGS_WINDOW_RES.1 as f32)
    , location: None
    , will_close: false
    , settings_updates: None
};

/// State struct for the window actor.

#[derive(Clone, Debug)]
pub(crate) struct SettingsState {
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsUpdate {
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
    pub(crate) settings_updates: Option<SettingsUpdate>
}


#[derive(Clone, Debug)]
pub(crate) struct SettingsWindowContext {
    pub(crate) settings: SettingsState
    , pub(crate) settings_updates: Option<SettingsUpdate>
    , pub(crate) size: Vec2
    , pub(crate) location: Option<Pos2>
    , pub(crate) will_close: bool
}


pub(crate) fn settings (
    ctx: &egui::Context,
    state: Arc<Mutex<SettingsWindowContext>>,
) -> SettingsWindowResult {

    let state1 = state.clone();
    let state2 = state.clone();

    let state = state.try_lock().unwrap();

    let viewport_options =
        egui::ViewportBuilder::default()
            .with_inner_size(state.size.clone());

    let viewport_options = match state.location {
        Some(l) => {viewport_options.with_position(l)}
        None => {viewport_options}
    };

    drop(state);

    ctx.show_viewport_deferred(
        ViewportId::from_hash_of("my_viewport"),
        viewport_options
            .with_title("Deferred Viewport")
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
                                    //info!("settings window should close");
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

    let mut state = state2.try_lock().unwrap();

    //info!("will close: {}", state.will_close);

    let will_close = state.will_close.clone();

    state.will_close = false;

    SettingsWindowResult{
        will_close: will_close,
        settings_updates: state.settings_updates.clone()
    }
}