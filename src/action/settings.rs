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

use crate::action::widgetize::*;




const RECOVER_EGUI_CRASHES:bool = false;
// ^ half-implemented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = true;

const DEFAULT_PHASE:Animable = Animable::new(0.0, (0.0, 10.0), Normalizing::NONE);
const DEFAULT_PERIOD:Animable = Animable::new(10.0, (1.0, 10.0), Normalizing::NONE);

pub(crate) const DEFAULT_SETTINGS_WINDOW_RES:(u32, u32) = (500, 800);

pub(crate) const DEFAULT_COLORING_SCRIPT:[ColoringInstruction;7] = [
    ColoringInstruction::PaintEscapeTime{id: 0, opacity:255
        , color:(128,128,128), range:64
        , shading_method: ShadingInstruction{
            shading: Shading::SINUS
            , period: DEFAULT_PERIOD
            , phase: DEFAULT_PHASE
        }
        , normalizing_method: Normalizing::NONE}
    , ColoringInstruction::PaintSmallTime{id: 1, inside_opacity:0, outside_opacity:30
        , color:(128,128,128), range:64
        , shading_method: ShadingInstruction{
            shading: Shading::SINUS
            , period: DEFAULT_PERIOD
            , phase: DEFAULT_PHASE
        }
        , normalizing_method: Normalizing::NONE}
    , ColoringInstruction::PaintSmallness{
        id: 2, inside_opacity:0, outside_opacity:0
        , color:(128,128,128), range:64
        , shading_method: ShadingInstruction{
            shading: Shading::SINUS
            , period: DEFAULT_PERIOD
            , phase: DEFAULT_PHASE
        }
        , normalizing_method: Normalizing::NONE}
    , ColoringInstruction::HighlightInFilaments{id: 3, opacity:255, color:(0,0,0)}
    , ColoringInstruction::HighlightOutFilaments{id: 4, opacity:255, color:(128,128,128)}
    , ColoringInstruction::HighlightNodes{id: 5, inside_opacity:0, outside_opacity:0
        , color:(128,128,128), thickness:10, only_fattest:true}
    , ColoringInstruction::HighlightSmallTimeEdges{id: 6, inside_opacity:30, outside_opacity:0
        , color:(128,128,128)}
];


impl Settings {
    pub(crate) const DEFAULT:Settings = Settings{
        coloring_script: None
        , bailout_radius: Animable::new(2.0, (2.0, 255.0), Normalizing::LNLN)
        , bailout_max_additional_iterations: 10
        , estimate_extra_iterations: false
        , id_counter: 7
        , currently_selected_coloring_instruction: 0
    };

    pub(crate) fn determine(&mut self) {
        self.bailout_radius.determine();
        if let Some(instructions) = &mut self.coloring_script {
            for instruction in instructions {
                match instruction {
                    ColoringInstruction::PaintEscapeTime{shading_method, ..} => {
                        shading_method.period.determine();
                        shading_method.phase.determine();
                    }
                    ColoringInstruction::PaintSmallTime{shading_method, ..} => {
                        shading_method.period.determine();
                        shading_method.phase.determine();
                    }
                    ColoringInstruction::PaintSmallness{shading_method, ..} => {
                        shading_method.period.determine();
                        shading_method.phase.determine();
                    }
                    _ => {}
                }
            }
        }
    }
}

pub const DEFAULT_SETTINGS_WINDOW_CONTEXT:SettingsWindowContext = SettingsWindowContext{
    settings: Settings::DEFAULT
    , size: egui::vec2(DEFAULT_SETTINGS_WINDOW_RES.0 as f32, DEFAULT_SETTINGS_WINDOW_RES.1 as f32)
    , location: None
    , will_close: false
    , checked: false
    , id_counter: 7
};

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) coloring_script:Option<Vec<ColoringInstruction>>
    , pub(crate) bailout_radius:Animable
    , pub(crate) bailout_max_additional_iterations:u32
    , pub(crate) estimate_extra_iterations:bool
    , pub(crate) currently_selected_coloring_instruction: u64
    , pub(crate) id_counter: u64
}


#[derive(Clone, Debug, Copy)]

pub(crate) struct Animable {
    pub(crate) start: Option<Instant>
    , pub(crate) period: Duration
    , pub(crate) value: f64
    , pub(crate) animated: bool
    , pub(crate) range: (f64, f64)
    , pub(crate) limits: (f64, f64)
    , pub(crate) normalizing: Normalizing
    , pub(crate) frame_value: f64
    , pub(crate) frame_value_reciprocal: f64
}

use core::ops::RangeInclusive;




use std::f64::consts::*;
impl Animable {

    const fn new(value:f64, limits: (f64, f64), normalizing: Normalizing) -> Animable {
        Animable {
            start:None
            , period:Duration::from_secs(10)
            , value
            , animated:false
            , range:(value, value+10.0)
            , limits
            , normalizing
            , frame_value: value
            , frame_value_reciprocal: 1.0/value
        }
    }


    pub(crate) fn determine(&mut self) -> f64 {
        match self {
            Animable{mut start, period, range, limits, normalizing, animated, value, ..} => {
                if *animated {
                    if start.is_none() {start = Some(Instant::now())}
                    let elapsed = start.unwrap().elapsed();
                    let phase_time = elapsed.as_secs_f64() % period.as_secs_f64();
                    let normalized_phase_time = phase_time / period.as_secs_f64();
                    let wave_result = (1.0-((normalized_phase_time*TAU).cos()))/2.0;

                    let min = (normalizing.normalize64)(&self.range.0);
                    let max = (normalizing.normalize64)(&self.range.1);
                    let range = max - min;
                    let result = (normalizing.denormalize64)(&(min + (range*wave_result)));
                    self.frame_value = result;
                    self.frame_value_reciprocal = 1.0/result;
                    result
                } else {
                    let result = normalizing.reshape_input(limits, value);
                    self.frame_value = result;
                    self.frame_value_reciprocal = 1.0/result;
                    result
                }
            }
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]

pub(crate) struct Normalizing {
    pub(crate) normalize64: fn(&f64)->f64
    , pub(crate) denormalize64: fn(&f64)->f64
    , pub(crate) normalize32: fn(&f32)->f32
    , pub(crate) denormalize32: fn(&f32)->f32
    //, pub(crate) spec: NormalizingSpec
}
impl Normalizing {

    pub(crate) const NONE:Normalizing = Normalizing {
        normalize64: |n| -> f64 {*n}
        , denormalize64: |n| -> f64 {*n}
        , normalize32: |n| -> f32 {*n}
        , denormalize32: |n| -> f32 {*n}
        //, spec: NormalizingSpec::None{}
    };
    pub(crate) const LNLN:Normalizing = Normalizing {
        normalize64: |n| -> f64 {n.ln().ln()}
        , denormalize64: |n| -> f64 {n.exp().exp()}
        , normalize32: |n| -> f32 {n.ln().ln()}
        , denormalize32: |n| -> f32 {n.exp().exp()}
        //, spec: NormalizingSpec::None{}
    };
    pub(crate) const LN:Normalizing = Normalizing {
        normalize64: |n| -> f64 {n.ln()}
        , denormalize64: |n| -> f64 {n.exp()}
        , normalize32: |n| -> f32 {n.ln()}
        , denormalize32: |n| -> f32 {n.exp()}
        //, spec: NormalizingSpec::None{}
    };
    pub(crate) const RECIP:Normalizing = Normalizing {
        normalize64: |n| -> f64 {1.0/n}
        , denormalize64: |n| -> f64 {1.0/n}
        , normalize32: |n| -> f32 {1.0/n}
        , denormalize32: |n| -> f32 {1.0/n}
        //, spec: NormalizingSpec::None{}
    };
    pub(crate) const RECIPLN:Normalizing = Normalizing {
        normalize64: |n| -> f64 {1.0/(n.ln())}
        , denormalize64: |n| -> f64 {(1.0/n).exp()}
        , normalize32: |n| -> f32 {1.0/(n.ln())}
        , denormalize32: |n| -> f32 {(1.0/n).exp()}
        //, spec: NormalizingSpec::None{}
    };

    pub(crate) fn reshape_input(&self, limits:&(f64, f64), input:&f64) -> f64 {

        let scalar_input = (input-limits.0)/(limits.1-limits.0);

        let normalized_min = (self.normalize64)(&limits.0);
        let normalized_max = (self.normalize64)(&limits.1);
        let normalized_range = normalized_max - normalized_min;
        (self.denormalize64)(&(normalized_min + (normalized_range*scalar_input)))
    }
}


#[derive(Clone, Debug, Copy, PartialEq)]

pub(crate) enum NormalizingSpec {
    None{}
    , LnLn{}
    , Ln{}
    , Reciprocal{}
    , RecipLn{}
}

#[derive(Clone, Debug, Copy)]

pub(crate) struct ShadingInstruction {
    pub(crate) period:Animable, pub(crate) phase:Animable
    , pub(crate) shading: Shading
}

#[derive(Clone, Debug, Copy, PartialEq)]

pub(crate) struct Shading {
    pub(crate) shade: fn(&f64, &f64, &f64, &f64) -> f64
}

impl Shading {
    const SINUS:Shading = Shading {
        shade: |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
            (1.0-((n+phase)*TAU*period_recip).cos())*0.5
        }
    };
    const MODULAR:Shading = Shading {
        shade: |phase:&f64, period:&f64, period_recip:&f64, n:&f64| -> f64 {
            ((n+phase) % period)*period_recip
        }
    };
}


#[derive(Clone, Debug, Copy)]

pub(crate) enum ColoringInstruction {
    PaintEscapeTime{
         opacity:u8
        , color:(u8,u8,u8), range:u8
        , shading_method: ShadingInstruction
        , normalizing_method: Normalizing
        , id:u64
    }
    , PaintSmallTime{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8), range:u8
        , shading_method: ShadingInstruction
        , normalizing_method: Normalizing
        , id:u64
    }
    , PaintSmallness{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8), range:u8
        , shading_method: ShadingInstruction
        , normalizing_method: Normalizing
        , id:u64
    }
    , HighlightInFilaments{
        opacity:u8, color:(u8,u8,u8)
        , id:u64
    }
    , HighlightOutFilaments{
        opacity:u8, color:(u8,u8,u8)
        , id:u64
    }
    , HighlightNodes{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8)
        , id:u64
        , thickness:u8
        , only_fattest: bool
    }
    , HighlightSmallTimeEdges{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8)
        , id:u64
    }
}

impl ColoringInstruction {
    pub(crate) fn id(self) -> u64 {
        match self {
            ColoringInstruction::PaintEscapeTime{id, ..
            } => {id}
            , ColoringInstruction::PaintSmallTime{id,..
            } => {id}
            , ColoringInstruction::PaintSmallness{id,..
            } => {id}
            , ColoringInstruction::HighlightInFilaments{id,..
            } => {id}
            , ColoringInstruction::HighlightOutFilaments{id,..
            } => {id}
            , ColoringInstruction::HighlightNodes{id,..
            } => {id}
            , ColoringInstruction::HighlightSmallTimeEdges{id,..
            } => {id}
        }
    }
}

use std::hash::*;
impl Hash for ColoringInstruction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ColoringInstruction::PaintEscapeTime{id, ..
            } => {id.hash(state);}
            , ColoringInstruction::PaintSmallTime{id,..
            } => {id.hash(state);}
            , ColoringInstruction::PaintSmallness{id,..
            } => {id.hash(state);}
            , ColoringInstruction::HighlightInFilaments{id,..
            } => {id.hash(state);}
            , ColoringInstruction::HighlightOutFilaments{id,..
            } => {id.hash(state);}
            , ColoringInstruction::HighlightNodes{id,..
            } => {id.hash(state);}
            , ColoringInstruction::HighlightSmallTimeEdges{id,..
            } => {id.hash(state);}
        }
    }
}


/*impl Hash for Person {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.phone.hash(state);
    }
}*/

impl ColoringInstruction {
    fn name(self) -> String {
        match self {
            ColoringInstruction::PaintEscapeTime{..
            } => {String::from("Escape Time")}
            , ColoringInstruction::PaintSmallTime{..
            } => {String::from("Small Time")}
            , ColoringInstruction::PaintSmallness{..
            } => {String::from("Small PB")}
            , ColoringInstruction::HighlightInFilaments{..
            } => {String::from("In Filaments")}
            , ColoringInstruction::HighlightOutFilaments{..
            } => {String::from("Out Filaments")}
            , ColoringInstruction::HighlightNodes{..
            } => {String::from("Minis")}
            , ColoringInstruction::HighlightSmallTimeEdges{..
            } => {String::from("Small Time Edges")}
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
    , pub(crate) id_counter: u64
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

                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    state.settings.widgetize(ui);
                });


                ctx.request_repaint();

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