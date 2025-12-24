use steady_state::*;
use eframe::egui;
use eframe::egui::*;
//use eframe::Frame::raw_window_handle;
 // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use egui::{Color32, Vec2, Pos2, ViewportId, WindowLevel};
use std::sync::{Arc, Mutex};

use egui_dnd::dnd;






const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = true;

pub(crate) const DEFAULT_SETTINGS_WINDOW_RES:(u32, u32) = (300, 200);

pub const DEFAULT_SETTINGS:Settings = Settings{
    coloring_order: [
        ColoringInstruction::PaintEscapeTime{

        }
        , ColoringInstruction::PaintSmallTime{

        }
        , ColoringInstruction::PaintSmallness{

        }
        , ColoringInstruction::HighlightInFilaments{}
        , ColoringInstruction::HighlightOutFilaments{}
        , ColoringInstruction::HighlightNodes{

        }
    ]
    , escape_time_coloring: EscapeTimeColoring{ opacity:255
        , color:(128,128,128), range:64
        , shading_method: Shading::Sinus{period: 10.0, phase: Animable::Value{val:0.0}}
        , normalizing_method: Normalizing::None{}}
    , small_time_coloring: SmallTimeColoring{inside_opacity:0, outside_opacity:0
        , color:(128,128,128), range:64
        , shading_method: Shading::Sinus{period: 10.0, phase: Animable::Value{val:0.0}}
        , normalizing_method: Normalizing::None{}}
    , smallness_coloring: SmallnessColoring{inside_opacity:0, outside_opacity:0
        , color:(128,128,128), range:64
        , shading_method: Shading::Sinus{period: 10.0, phase: Animable::Value{val:0.0}}
        , normalizing_method: Normalizing::None{}}
    , in_filament_highlighting: InFilamentHighlighting{opacity:0, color:(128,128,128)}
    , out_filament_highlighting: OutFilamentHighlighting{opacity:0, color:(128,128,128)}
    , node_highlighting: NodeHighlighting{
        inside_opacity:0, outside_opacity:0
        , color:(128,128,128)
    }
    , bailout_radius: Animable::Value{val:2.0}
};

pub const DEFAULT_SETTINGS_WINDOW_CONTEXT:SettingsWindowContext = SettingsWindowContext{
    settings: DEFAULT_SETTINGS
    , size: egui::vec2(DEFAULT_SETTINGS_WINDOW_RES.0 as f32, DEFAULT_SETTINGS_WINDOW_RES.1 as f32)
    , location: None
    , will_close: false
    , checked: false
};

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    coloring_order:[ColoringInstruction;6]
    , escape_time_coloring: EscapeTimeColoring
    , small_time_coloring: SmallTimeColoring
    , smallness_coloring: SmallnessColoring
    , in_filament_highlighting: InFilamentHighlighting
    , out_filament_highlighting: OutFilamentHighlighting
    , node_highlighting: NodeHighlighting
    , bailout_radius:Animable
}
#[derive(Clone, Debug)]

enum Animable {
    Value{val:f64}
    , Animation{
        start: Instant
        , period: Duration
        , amplitude: f64
        , min:f64
        , max:f64
    }
}

#[derive(Clone, Debug)]

enum Normalizing {
    None{}
    , Log{repeat:bool, base:f64}
    , Reciprocal{}
}
#[derive(Clone, Debug)]

enum Shading {
    Modular{period:f64, phase:Animable}
    , Sinus{period:f64, phase:Animable}
    , Linear{}
    , Histogram{}
}


#[derive(Clone, Debug)]

struct EscapeTimeColoring {
    opacity:u8
    , color:(u8,u8,u8), range:u8
    , shading_method: Shading
    , normalizing_method: Normalizing
}
#[derive(Clone, Debug)]

struct SmallTimeColoring {
    inside_opacity:u8, outside_opacity:u8
    , color:(u8,u8,u8), range:u8
    , shading_method: Shading
    , normalizing_method: Normalizing
}
#[derive(Clone, Debug)]

struct SmallnessColoring {
    inside_opacity:u8, outside_opacity:u8
    , color:(u8,u8,u8), range:u8
    , shading_method: Shading
    , normalizing_method: Normalizing
}
#[derive(Clone, Debug)]

struct InFilamentHighlighting {
    opacity:u8, color:(u8,u8,u8)
}
#[derive(Clone, Debug)]

struct OutFilamentHighlighting {
    opacity:u8, color:(u8,u8,u8)
}
#[derive(Clone, Debug)]

struct NodeHighlighting {
    inside_opacity:u8, outside_opacity:u8
    , color:(u8,u8,u8)
}

#[derive(Clone, Debug, Hash, Copy)]

enum ColoringInstruction {
    PaintEscapeTime{}
    , PaintSmallTime{}
    , PaintSmallness{}
    , HighlightInFilaments{}
    , HighlightOutFilaments{}
    , HighlightNodes{}
}

impl ColoringInstruction {
    fn name(self) -> String {
        match self {
            ColoringInstruction::PaintEscapeTime{
            } => {String::from("PaintEscapeTime")}
            , ColoringInstruction::PaintSmallTime{
            } => {String::from("PaintSmallTime")}
            , ColoringInstruction::PaintSmallness{
            } => {String::from("PaintSmallness")}
            , ColoringInstruction::HighlightInFilaments{
            } => {String::from("HighlightInFilaments")}
            , ColoringInstruction::HighlightOutFilaments{
            } => {String::from("HighlightOutFilaments")}
            , ColoringInstruction::HighlightNodes{
            } => {String::from("HighlightNodes")}
        }
    }
}

impl From<ColoringInstruction> for WidgetText {
    fn from(ci:ColoringInstruction) -> WidgetText {
        WidgetText::from(ci.name())
    }
}


#[derive(Clone, Debug)]
pub(crate) enum ControlsSettings {
    H
}

pub(crate) struct SettingsWindowResult {
    pub(crate) will_close: bool,
    pub(crate) settings: Settings
}


#[derive(Clone, Debug)]
pub(crate) struct SettingsWindowContext {
    pub(crate) settings: Settings
    , pub(crate) size: Vec2
    , pub(crate) location: Option<Pos2>
    , pub(crate) will_close: bool
    , pub(crate) checked: bool
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
                //if ui.add(Button::new("Click me")).clicked() {println!("clicked")}

                ui.add(egui::Checkbox::new(&mut state.checked, "Checked"));
                if state.checked {
                    if ui.add(Button::new("Click me")).clicked() {println!("clicked")}
                }


                let mut items = state.settings.coloring_order.to_vec();

                let mut rect = Rect::ZERO;

                dnd(ui, "dnd_example").show_vec(&mut items, |ui, item, handle, state| {
                    ui.horizontal(|ui| {
                        handle.ui(ui, |ui| {
                            if state.dragged {
                                ui.label("dragging");
                            } else {
                                ui.label("drag");
                            }
                        });
                        ui.label(*item);
                    });
                });

                state.settings.coloring_order = items.try_into().unwrap();

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
        settings: state.settings.clone()
    }
}