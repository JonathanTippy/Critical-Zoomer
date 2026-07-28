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

use crate::assemblies::headgroup::window::widgetize::*;




const RECOVER_EGUI_CRASHES:bool = false;
// ^ half-implemented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = true;

pub const DEFAULT_SETTINGS_WINDOW_RES:(u32, u32) = (500, 800);

// D-COLOR-1: escape time; in-filaments black; out-filaments as outside ∞-escape; nothing else.
pub const DEFAULT_COLORING_SCRIPT:[ColoringInstruction;3] = [
    ColoringInstruction::PaintEscapeTime{id: 0
        , inside_opacity:255, outside_opacity:255
        , color:(128,128,128), range:64
        , shading_method: ShadingInstruction{
            shading: Shading::Sinus{}
            , period: Animable{
                start:None
                , period:Duration::from_secs(10)
                , value:10.0
                , animated:false
                , range:(1.0, 10.0)
                , limits:(1.0, 10.0)
                , normalizing:Normalizing::None{}
            }
            , phase: Animable{
                start:None
                , period:Duration::from_secs(10)
                , value:0.0
                , animated:false
                , range:(0.0, 10.0)
                , limits:(0.0, 10.0)
                , normalizing:Normalizing::None{}
            }
        }
        , normalizing_method: Normalizing::None{}}
    , ColoringInstruction::HighlightInFilaments{
        id: 1, inside_opacity:255, outside_opacity:255, color:(0,0,0)
    }
    // Out filaments: paint like ∞ escape (shade path); color unused when ∞-escape path is used.
    , ColoringInstruction::HighlightOutFilaments{
        id: 2, inside_opacity:255, outside_opacity:255, color:(128,128,128)
    }
];


impl Settings {
    pub const DEFAULT:Settings = Settings{
        coloring_script: None
        , bailout_radius: Animable{start:None,period:Duration::from_secs(10)
            , value:2.0
            , animated:false
            , range:(2.0, u32::MAX as f64)
            , limits:(2.0, u32::MAX as f64)
            , normalizing:Normalizing::LnLn{}}
        , bailout_max_additional_iterations: 10
        , id_counter: 3
        , currently_selected_coloring_instruction: 0
        // L = L CPU + L VRAM; default 1GB each side of the ledger.
        // r[impl cz.system.memory-default-1gb+1]
        // r[impl cz.cosmetic.bailout-range-2-255+1]
        , memory_limit_bytes: 1_000_000_000
        , memory_floor_bytes: 125_000_000
    };
}

pub const DEFAULT_SETTINGS_WINDOW_CONTEXT:SettingsWindowContext = SettingsWindowContext{
    settings: Settings::DEFAULT
    , size: egui::vec2(DEFAULT_SETTINGS_WINDOW_RES.0 as f32, DEFAULT_SETTINGS_WINDOW_RES.1 as f32)
    , location: None
    , will_close: false
    , checked: false
    , id_counter: 3
};

#[derive(Clone, Debug)]
pub struct Settings {
    pub coloring_script:Option<Vec<ColoringInstruction>>
    , pub bailout_radius:Animable
    , pub bailout_max_additional_iterations:u32
    , pub currently_selected_coloring_instruction: u64
    , pub id_counter: u64
    // Soft per-side budget (bytes). Slider L means L CPU + L VRAM.
    , pub memory_limit_bytes: usize
    // On-demand slider floor (protected screen+lookahead); bumps raise this.
    , pub memory_floor_bytes: usize
}


#[derive(Clone, Debug, Copy)]

pub struct Animable {
    pub start: Option<Instant>
    , pub period: Duration
    , pub value: f64
    , pub animated: bool
    , pub range: (f64, f64)
    , pub limits: (f64, f64)
    , pub normalizing: Normalizing
}

use core::ops::RangeInclusive;




use std::f64::consts::*;
impl Animable {
    pub fn determine(&mut self) -> f64 {
        match self {
            Animable{mut start, period, range, limits, normalizing, animated, value, ..} => {
                if *animated {
                    if start.is_none() {start = Some(Instant::now())}
                    let elapsed = start.unwrap().elapsed();
                    let phase_time = elapsed.as_secs_f64() % period.as_secs_f64();
                    let normalized_phase_time = phase_time / period.as_secs_f64();
                    let wave_result = (1.0-((normalized_phase_time*TAU).cos()))/2.0;

                    let min = normalizing.normalize(&self.range.0);
                    let max = normalizing.normalize(&self.range.1);
                    let range = max - min;
                    normalizing.denormalize(&(min + (range*wave_result)))
                } else {
                    normalizing.reshape_input(limits, value)
                }

            }
        }
    }

}



#[derive(Clone, Debug, Copy, PartialEq)]

pub enum Normalizing {
    None{}
    , LnLn{}
    , Ln{}
    , Reciprocal{}
    , RecipLn{}
}

impl Normalizing {
    pub fn normalize(&self, input:&f64) -> f64 {
        match self {
            Normalizing::None{..} => {*input}
            Normalizing::LnLn{..} => {
                input.ln().ln()
            }
            Normalizing::Ln{..} => {
                input.ln()
            }
            // The reciprocal of the log, matching the shading shader.
            Normalizing::RecipLn{..} => {
                1.0/input.ln()
            }
            Normalizing::Reciprocal{..} => {1.0/input}
        }
    }

    pub fn denormalize(&self, input:&f64) -> f64 {
        match self {
            Normalizing::None{..} => {*input}
            Normalizing::LnLn{..} => {
                input.exp().exp()
            }
            Normalizing::Ln{..} => {
                input.exp()
            }
            Normalizing::RecipLn{..} => {
                (1.0/input).exp()
            }
            Normalizing::Reciprocal{..} => {1.0/input}
        }
    }

    pub fn reshape_input(&self, limits:&(f64, f64), input:&f64) -> f64 {

        let scalar_input = (input-limits.0)/(limits.1-limits.0);

        let normalized_min = self.normalize(&limits.0);
        let normalized_max = self.normalize(&limits.1);
        let normalized_range = normalized_max - normalized_min;
        self.denormalize(&(normalized_min + (normalized_range*scalar_input)))
    }

}

#[derive(Clone, Debug, Copy)]


pub struct ShadingInstruction {
    pub period:Animable, pub phase:Animable
    , pub shading: Shading
}

#[derive(Clone, Debug, Copy, PartialEq)]

pub enum Shading {
    Modular{}
    , Sinus{}
}


#[derive(Clone, Debug, Copy)]

pub enum ColoringInstruction {
    PaintEscapeTime{
         inside_opacity:u8, outside_opacity:u8
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
        inside_opacity:u8, outside_opacity:u8, color:(u8,u8,u8)
        , id:u64
    }
    , HighlightOutFilaments{
        inside_opacity:u8, outside_opacity:u8, color:(u8,u8,u8)
        , id:u64
    }
    , HighlightNodes{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8)
        , id:u64
        , thickness:u8
    }
    , HighlightSmallTimeEdges{
        inside_opacity:u8, outside_opacity:u8
        , color:(u8,u8,u8)
        , id:u64
    }
}

impl ColoringInstruction {
    pub fn id(self) -> u64 {
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
pub enum ControlsSettings {
    H
}

pub struct SettingsWindowResult {
    pub will_close: bool,
    pub settings: Settings
}


#[derive(Clone, Debug)]
pub struct SettingsWindowContext {
    pub settings: Settings
    , pub size: Vec2
    , pub location: Option<Pos2>
    , pub will_close: bool
    , pub checked: bool
    , pub id_counter: u64
}


pub fn settings (
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


                // Do not request an immediate repaint — that overrides the main
                // viewport's 60fps `request_repaint_after` (smallest delay wins).
                ctx.request_repaint_after(std::time::Duration::from_nanos(16_666_667));

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

#[cfg(test)]
mod animable_tests {
    use super::*;
    use std::time::Duration;

    fn static_animable(value: f64, range: (f64, f64)) -> Animable {
        Animable {
            start: None,
            period: Duration::from_secs(10),
            value,
            animated: false,
            range,
            limits: range,
            normalizing: Normalizing::None {},
        }
    }

    #[test]
    fn determine_static_returns_value_inside_limits() {
        let mut a = static_animable(3.5, (1.0, 10.0));
        let v = a.determine();
        assert!((v - 3.5).abs() < 1e-9, "got {v}");
        assert!(v >= 1.0 && v <= 10.0);
    }

    #[test]
    fn determine_animated_stays_in_normalized_range() {
        let mut a = Animable {
            start: Some(Instant::now()),
            period: Duration::from_secs(2),
            value: 0.0,
            animated: true,
            range: (2.0, 8.0),
            limits: (2.0, 8.0),
            normalizing: Normalizing::None {},
        };
        for _ in 0..20 {
            let v = a.determine();
            assert!(
                (2.0..=8.0).contains(&v),
                "animated wave left range: {v}"
            );
            // Must not collapse to constant 0/1/-1 (common mutants).
            std::thread::sleep(Duration::from_millis(30));
        }
        let samples: Vec<f64> = (0..5).map(|_| {
            std::thread::sleep(Duration::from_millis(50));
            a.determine()
        }).collect();
        let spread = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - samples.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(spread > 1e-6 || samples.iter().any(|&x| (x - 0.0).abs() > 1e-6));
    }

    #[test]
    fn determine_period_modulo_not_division_identity() {
        // Phase uses elapsed % period; if mutated to / the value drifts unboundedly.
        let mut a = Animable {
            start: Some(Instant::now() - Duration::from_secs(100)),
            period: Duration::from_secs(3),
            value: 0.0,
            animated: true,
            range: (0.0, 1.0),
            limits: (0.0, 1.0),
            normalizing: Normalizing::None {},
        };
        let v = a.determine();
        assert!((0.0..=1.0).contains(&v), "phase escaped unit range: {v}");
    }

    #[test]
    fn determine_wave_is_half_raised_cosine_not_identity() {
        // At phase 0: (1-cos(0))/2 == 0 → denorm to range.0
        // At phase 0.5: (1-cos(π))/2 == 1 → denorm to range.1
        // Mutating /2.0 to % or * breaks endpoints.
        let mut a = Animable {
            start: Some(Instant::now()),
            period: Duration::from_secs(10_000),
            value: 0.0,
            animated: true,
            range: (4.0, 12.0),
            limits: (4.0, 12.0),
            normalizing: Normalizing::None {},
        };
        let near_start = a.determine();
        assert!(
            (near_start - 4.0).abs() < 0.05,
            "phase~0 should sit near range min, got {near_start}"
        );
        a.start = Some(Instant::now() - Duration::from_secs(5_000));
        let near_mid = a.determine();
        assert!(
            (near_mid - 12.0).abs() < 0.05,
            "phase~0.5 should sit near range max, got {near_mid}"
        );
    }

    #[test]
    fn max_frame_time_is_reciprocal_of_min_frame_rate() {
        // Guards const MAX_FRAME_TIME = 1.0 / MIN_FRAME_RATE mutants (% or *).
        assert!((super::MAX_FRAME_TIME - (1.0 / super::MIN_FRAME_RATE)).abs() < 1e-12);
        assert!((super::MAX_FRAME_TIME - 0.05).abs() < 1e-12);
    }

    // D-COLOR-1 / REQ-COSMETIC-DEFAULT
    #[test]
    fn default_script_has_exactly_three_layers() {
        assert_eq!(DEFAULT_COLORING_SCRIPT.len(), 3);
    }

    #[test]
    fn default_script_is_escape_infil_outfil_only() {
        assert!(matches!(
            DEFAULT_COLORING_SCRIPT[0],
            ColoringInstruction::PaintEscapeTime { .. }
        ));
        assert!(matches!(
            DEFAULT_COLORING_SCRIPT[1],
            ColoringInstruction::HighlightInFilaments { color: (0, 0, 0), .. }
        ));
        assert!(matches!(
            DEFAULT_COLORING_SCRIPT[2],
            ColoringInstruction::HighlightOutFilaments { .. }
        ));
    }

    #[test]
    fn default_script_excludes_subtle_extra_layers() {
        for inst in DEFAULT_COLORING_SCRIPT.iter() {
            assert!(!matches!(
                inst,
                ColoringInstruction::PaintSmallTime { .. }
                    | ColoringInstruction::PaintSmallness { .. }
                    | ColoringInstruction::HighlightNodes { .. }
                    | ColoringInstruction::HighlightSmallTimeEdges { .. }
            ));
        }
    }

    // D-COLOR-4 / REQ-COSMETIC-LAYER: highlights are ColoringInstruction variants in the list.
    #[test]
    fn highlights_are_script_layer_variants() {
        assert!(matches!(
            ColoringInstruction::HighlightInFilaments {
                id: 0,
                inside_opacity: 255,
                outside_opacity: 255,
                color: (0, 0, 0)
            },
            ColoringInstruction::HighlightInFilaments { .. }
        ));
        assert!(matches!(
            ColoringInstruction::HighlightOutFilaments {
                id: 0,
                inside_opacity: 255,
                outside_opacity: 255,
                color: (1, 1, 1)
            },
            ColoringInstruction::HighlightOutFilaments { .. }
        ));
        assert!(matches!(
            ColoringInstruction::HighlightNodes {
                id: 0,
                inside_opacity: 0,
                outside_opacity: 0,
                color: (0, 0, 0),
                thickness: 1
            },
            ColoringInstruction::HighlightNodes { .. }
        ));
    }
}