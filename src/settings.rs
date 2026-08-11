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

pub const DEFAULT_COLORING_SCRIPT:[ColoringInstruction;7] = [
    ColoringInstruction::PaintEscapeTime{id: 0, opacity:255
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
    , ColoringInstruction::PaintSmallTime{id: 1, inside_opacity:0, outside_opacity:30
        , color:(128,128,128), range:64
        , shading_method: ShadingInstruction{
            shading: Shading::Sinus{}
            , period: Animable{
                start:None
                , period:Duration::from_secs(10)
                , value:3.0
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
    , ColoringInstruction::PaintSmallness{
        id: 2, inside_opacity:0, outside_opacity:0
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
    , ColoringInstruction::HighlightInFilaments{id: 3, opacity:255, color:(0,0,0)}
    , ColoringInstruction::HighlightOutFilaments{id: 4, opacity:255, color:(128,128,128)}
    , ColoringInstruction::HighlightNodes{id: 5, inside_opacity:0, outside_opacity:0
        , color:(128,128,128), thickness:10, only_fattest:true}
    , ColoringInstruction::HighlightSmallTimeEdges{id: 6, inside_opacity:30, outside_opacity:0
        , color:(128,128,128)}
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
        , estimate_extra_iterations: false
        , id_counter: 7
        , currently_selected_coloring_instruction: 0
        // Debug: force compute kernel; host type stays auto from depth.
        , manual_gear_enabled: false
        , manual_gear: crate::assemblies::structs::KernelMode::Naive
        // Debug: force colorer path; default GPU when disabled.
        , manual_color_gear_enabled: false
        , manual_color_gear: crate::assemblies::structs::ColorerMode::Gpu
        // Debug: force escaper path; default OG when disabled.
        , manual_escape_gear_enabled: false
        , manual_escape_gear: crate::assemblies::structs::EscaperMode::Og
        // Content-tier refresh (collector → shade). Automatic uses head-reported vsync Hz.
        , content_refresh_automatic: true
        , content_refresh_hz: 60.0
        , auto_vsync_hz: 60.0
        // Head present: vsync on by default; when off, cap with head_max_fps.
        , head_vsync_enabled: true
        , head_max_fps: 120.0
    };

    /// Resolved manual gear for the screen worker (`None` = automatic policy).
    pub fn manual_gear_override(&self) -> Option<crate::assemblies::structs::KernelMode> {
        if self.manual_gear_enabled {
            Some(self.manual_gear)
        } else {
            None
        }
    }

    /// Resolved colorer path. Default GPU; manual gear forces OG or GPU.
    pub fn resolved_color_gear(&self) -> crate::assemblies::structs::ColorerMode {
        if self.manual_color_gear_enabled {
            self.manual_color_gear
        } else {
            crate::assemblies::structs::ColorerMode::Gpu
        }
    }

    /// Resolved manual colorer when the override checkbox is on (`None` = use default).
    pub fn manual_color_gear_override(&self) -> Option<crate::assemblies::structs::ColorerMode> {
        if self.manual_color_gear_enabled {
            Some(self.manual_color_gear)
        } else {
            None
        }
    }

    /// Resolved escaper path. Default OG; manual gear forces OG or GPU.
    pub fn resolved_escape_gear(&self) -> crate::assemblies::structs::EscaperMode {
        if self.manual_escape_gear_enabled {
            self.manual_escape_gear
        } else {
            crate::assemblies::structs::EscaperMode::Og
        }
    }

    /// Resolved manual escaper when the override checkbox is on (`None` = use default).
    pub fn manual_escape_gear_override(&self) -> Option<crate::assemblies::structs::EscaperMode> {
        if self.manual_escape_gear_enabled {
            Some(self.manual_escape_gear)
        } else {
            None
        }
    }

    /// Content refresh Hz for collector/escaper/colorer timers (clamped 1–240).
    pub fn resolved_content_hz(&self) -> f64 {
        let hz = if self.content_refresh_automatic {
            self.auto_vsync_hz
        } else {
            self.content_refresh_hz
        };
        if !hz.is_finite() || hz < 1.0 {
            1.0
        } else if hz > 240.0 {
            240.0
        } else {
            hz
        }
    }

    /// Content-tier wake period from [`Self::resolved_content_hz`].
    pub fn resolved_content_period(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(1.0 / self.resolved_content_hz())
    }

    /// Head uncapped present period when vsync is disabled.
    pub fn resolved_head_max_period(&self) -> std::time::Duration {
        let hz = if !self.head_max_fps.is_finite() || self.head_max_fps < 1.0 {
            1.0
        } else if self.head_max_fps > 1000.0 {
            1000.0
        } else {
            self.head_max_fps
        };
        std::time::Duration::from_secs_f64(1.0 / hz)
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
pub struct Settings {
    pub coloring_script:Option<Vec<ColoringInstruction>>
    , pub bailout_radius:Animable
    , pub bailout_max_additional_iterations:u32
    , pub estimate_extra_iterations:bool
    , pub currently_selected_coloring_instruction: u64
    , pub id_counter: u64
    // When true, `manual_gear` selects the entire compute kernel (debug).
    // Host stack / type remains automatic from depth admission.
    , pub manual_gear_enabled: bool
    , pub manual_gear: crate::assemblies::structs::KernelMode
    // When true, `manual_color_gear` overrides the default GPU colorer.
    // Automatic PPS/kernel gearbox must never pick GPU color for the worker —
    // this is shade-path only.
    , pub manual_color_gear_enabled: bool
    , pub manual_color_gear: crate::assemblies::structs::ColorerMode
    // When true, `manual_escape_gear` overrides the default OG escaper.
    , pub manual_escape_gear_enabled: bool
    , pub manual_escape_gear: crate::assemblies::structs::EscaperMode
    // When true, content actors use `auto_vsync_hz`; else `content_refresh_hz`.
    , pub content_refresh_automatic: bool
    , pub content_refresh_hz: f64
    // Head-measured / bootstrap vsync Hz for Automatic content refresh.
    , pub auto_vsync_hz: f64
    , pub head_vsync_enabled: bool
    , pub head_max_fps: f64
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
        if self.animated {
            // Latch onto `self` — `Animable` is Copy, so a pattern-bound
            // `mut start` would only mutate a local and leave phase stuck at ~0.
            if self.start.is_none() {
                self.start = Some(Instant::now());
            }
            let elapsed = self.start.unwrap().elapsed();
            let period_secs = self.period.as_secs_f64();
            let phase_time = elapsed.as_secs_f64() % period_secs;
            let normalized_phase_time = phase_time / period_secs;
            let wave_result = (1.0 - ((normalized_phase_time * TAU).cos())) / 2.0;

            let min = self.normalizing.normalize(&self.range.0);
            let max = self.normalizing.normalize(&self.range.1);
            let range = max - min;
            self.normalizing
                .denormalize(&(min + (range * wave_result)))
        } else {
            self.normalizing.reshape_input(&self.limits, &self.value)
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
            Normalizing::RecipLn{..} => {
                (1.0/input).ln()
            }
            Normalizing::Reciprocal{..} => {1.0/input}
        }
    }

    /// f32 twin of `normalize` — shared OG/GPU shade math for exact Color32 parity.
    pub fn normalize_f32(&self, input: f32) -> f32 {
        match self {
            Normalizing::None { .. } => input,
            Normalizing::LnLn { .. } => input.ln().ln(),
            Normalizing::Ln { .. } => input.ln(),
            Normalizing::RecipLn { .. } => (1.0 / input).ln(),
            Normalizing::Reciprocal { .. } => 1.0 / input,
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
                1.0/(input.exp())
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

    pub fn get_normalizer(&self) -> Normalizer {
        match self {
            Normalizing::None{..} => {
                Normalizer{
                    normalize64: |n| {*n}
                    , denormalize64: |n| {*n}
                    , normalize32: |n| {*n}
                    , denormalize32: |n| {*n}
                }
            }
            Normalizing::LnLn{..} => {
                Normalizer{
                    normalize64: |n| {n.ln().ln()}
                    , denormalize64: |n| {n.exp().exp()}
                    , normalize32: |n| {n.ln().ln()}
                    , denormalize32: |n| {n.exp().exp()}
                }
            }
            Normalizing::Ln{..} => {
                Normalizer{
                    normalize64: |n| {n.ln()}
                    , denormalize64: |n| {n.exp()}
                    , normalize32: |n| {n.ln()}
                    , denormalize32: |n| {n.exp()}
                }
            }
            Normalizing::RecipLn{..} => {
                Normalizer{
                    // Must match `normalize`/`denormalize`: RecipLn is ln(1/x), not 1/ln(x).
                    normalize64: |n| {(1.0 / n).ln()}
                    , denormalize64: |n| {1.0 / n.exp()}
                    , normalize32: |n| {(1.0 / n).ln()}
                    , denormalize32: |n| {1.0 / n.exp()}
                }
            }
            Normalizing::Reciprocal{..} => {
                Normalizer{
                    normalize64: |n| {1.0/n}
                    , denormalize64: |n| {1.0/n}
                    , normalize32: |n| {1.0/n}
                    , denormalize32: |n| {1.0/n}
                }
            }
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]

pub struct Normalizer {
    pub normalize64: fn(&f64)->f64
    , pub denormalize64: fn(&f64)->f64
    , pub normalize32: fn(&f32)->f32
    , pub denormalize32: fn(&f32)->f32
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


                ctx.request_repaint_after(std::time::Duration::from_millis(100));

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
mod mutant_kill {
    use super::{Animable, ColoringInstruction, Normalizing, Settings, Shading, ShadingInstruction};
    use crate::assemblies::structs::KernelMode;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{Duration, Instant};

    #[test]
    fn mutant_kill_normalizing_roundtrip_and_arms() {
        let x = 2.5f64;
        assert_eq!(Normalizing::None {}.normalize(&x), x);
        assert_eq!(Normalizing::None {}.denormalize(&x), x);

        let ln = Normalizing::Ln {}.normalize(&x);
        assert!((ln - x.ln()).abs() < 1e-12);
        assert!((Normalizing::Ln {}.denormalize(&ln) - x).abs() < 1e-12);
        assert_ne!(ln, x.exp()); // ln↔exp swap

        let lnln = Normalizing::LnLn {}.normalize(&x);
        assert!((lnln - x.ln().ln()).abs() < 1e-12);
        assert!((Normalizing::LnLn {}.denormalize(&lnln) - x).abs() < 1e-9);

        let recip = Normalizing::Reciprocal {}.normalize(&x);
        assert!((recip - 1.0 / x).abs() < 1e-12);
        assert!((Normalizing::Reciprocal {}.denormalize(&recip) - x).abs() < 1e-12);
        assert_ne!(recip, x); // *↔/ identity

        // RecipLn is ln(1/x), not 1/ln(x).
        let rl = Normalizing::RecipLn {}.normalize(&x);
        assert!((rl - (1.0 / x).ln()).abs() < 1e-12);
        assert!((rl - (-x.ln())).abs() < 1e-12);
        assert_ne!(rl, 1.0 / x.ln());
        assert!((Normalizing::RecipLn {}.denormalize(&rl) - x).abs() < 1e-12);

        let n = Normalizing::RecipLn {}.get_normalizer();
        assert!(((n.normalize64)(&x) - (1.0 / x).ln()).abs() < 1e-12);
        assert_ne!((n.normalize64)(&x), 1.0 / x.ln());

        // reshape: linear in normalized space between limits.
        let limits = (2.0f64, 8.0f64);
        let mid = Normalizing::None {}.reshape_input(&limits, &5.0);
        assert!((mid - 5.0).abs() < 1e-12);
        let lo = Normalizing::Ln {}.reshape_input(&limits, &2.0);
        assert!((lo - 2.0).abs() < 1e-9);
        let hi = Normalizing::Ln {}.reshape_input(&limits, &8.0);
        assert!((hi - 8.0).abs() < 1e-9);
        // Midpoint in linear domain is not midpoint in ln-space.
        let mid_ln = Normalizing::Ln {}.reshape_input(&limits, &5.0);
        assert!((mid_ln - 5.0).abs() > 0.1);
        // None reshape is identity in the domain; Ln uses normalized interpolation
        // so swapped limits change the result for an interior sample.
        let q_ln = Normalizing::Ln {}.reshape_input(&limits, &3.0);
        let q_ln_swapped = Normalizing::Ln {}.reshape_input(&(8.0, 2.0), &3.0);
        assert_ne!(q_ln, q_ln_swapped);
        // scalar uses (input-lo)/(hi-lo): wrong op would break roundtrip endpoints.
        assert!((Normalizing::Reciprocal {}.reshape_input(&limits, &2.0) - 2.0).abs() < 1e-9);
        assert!((Normalizing::Reciprocal {}.reshape_input(&limits, &8.0) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn mutant_kill_animable_determine_static_wave_and_latch() {
        let mut a = Animable {
            start: None,
            period: Duration::from_secs(10),
            value: 5.0,
            animated: false,
            range: (1.0, 10.0),
            limits: (1.0, 10.0),
            normalizing: Normalizing::None {},
        };
        assert!((a.determine() - 5.0).abs() < 1e-12);
        // Static uses reshape/value, not the phase≈0 wave floor.
        assert_ne!(a.determine(), 1.0);
        assert!(a.start.is_none());

        a.animated = true;
        a.range = (2.0, 8.0);
        a.start = Some(Instant::now());
        let near_min = a.determine();
        assert!(
            (near_min - 2.0).abs() < 0.05,
            "phase≈0 must map near range.0, got {near_min}"
        );

        a.start = Some(Instant::now() - Duration::from_millis(5000));
        a.period = Duration::from_secs(10);
        let near_max = a.determine();
        assert!(
            (near_max - 8.0).abs() < 0.05,
            "phase=0.5 → cos(π)=-1 → wave=1 → range.1, got {near_max}"
        );
        assert_ne!(near_max, near_min);
        assert!(near_max > 7.0);

        // Latch writes through to self (Copy-bound mut start would leave None).
        a.start = None;
        let _ = a.determine();
        assert!(a.start.is_some());
        let first = a.start;
        let _ = a.determine();
        assert_eq!(a.start, first);
    }

    #[test]
    fn mutant_kill_manual_gear_override_enabled_gate() {
        let mut s = Settings::DEFAULT;
        assert!(s.manual_gear_override().is_none());
        s.manual_gear_enabled = true;
        s.manual_gear = KernelMode::Pert;
        assert_eq!(s.manual_gear_override(), Some(KernelMode::Pert));
        s.manual_gear = KernelMode::Naive;
        assert_eq!(s.manual_gear_override(), Some(KernelMode::Naive));
        s.manual_gear_enabled = false;
        assert!(s.manual_gear_override().is_none());
    }

    #[test]
    fn mutant_kill_manual_color_gear_override_enabled_gate() {
        use crate::assemblies::structs::ColorerMode;
        let mut s = Settings::DEFAULT;
        assert!(s.manual_color_gear_override().is_none());
        assert_eq!(s.resolved_color_gear(), ColorerMode::Gpu);
        s.manual_color_gear_enabled = true;
        s.manual_color_gear = ColorerMode::Gpu;
        assert_eq!(s.manual_color_gear_override(), Some(ColorerMode::Gpu));
        s.manual_color_gear = ColorerMode::Og;
        assert_eq!(s.manual_color_gear_override(), Some(ColorerMode::Og));
        assert_eq!(s.resolved_color_gear(), ColorerMode::Og);
        s.manual_color_gear_enabled = false;
        assert!(s.manual_color_gear_override().is_none());
        assert_eq!(s.resolved_color_gear(), ColorerMode::Gpu);
    }

    #[test]
    fn mutant_kill_manual_escape_gear_override_enabled_gate() {
        use crate::assemblies::structs::EscaperMode;
        let mut s = Settings::DEFAULT;
        assert!(s.manual_escape_gear_override().is_none());
        s.manual_escape_gear_enabled = true;
        s.manual_escape_gear = EscaperMode::Gpu;
        assert_eq!(s.manual_escape_gear_override(), Some(EscaperMode::Gpu));
        s.manual_escape_gear = EscaperMode::Og;
        assert_eq!(s.manual_escape_gear_override(), Some(EscaperMode::Og));
        s.manual_escape_gear_enabled = false;
        assert!(s.manual_escape_gear_override().is_none());
    }

    #[test]
    fn mutant_kill_content_refresh_period_auto_manual_clamp() {
        let mut s = Settings::DEFAULT;
        assert!(s.content_refresh_automatic);
        assert!((s.resolved_content_hz() - 60.0).abs() < 1e-9);
        s.auto_vsync_hz = 120.0;
        assert!((s.resolved_content_hz() - 120.0).abs() < 1e-9);
        s.content_refresh_automatic = false;
        s.content_refresh_hz = 30.0;
        assert!((s.resolved_content_hz() - 30.0).abs() < 1e-9);
        assert_eq!(
            s.resolved_content_period(),
            Duration::from_secs_f64(1.0 / 30.0)
        );
        s.content_refresh_hz = 0.5;
        assert!((s.resolved_content_hz() - 1.0).abs() < 1e-9);
        s.content_refresh_hz = 999.0;
        assert!((s.resolved_content_hz() - 240.0).abs() < 1e-9);
        s.head_vsync_enabled = false;
        s.head_max_fps = 60.0;
        assert_eq!(
            s.resolved_head_max_period(),
            Duration::from_secs_f64(1.0 / 60.0)
        );
    }

    #[test]
    fn mutant_kill_coloring_instruction_id_and_hash() {
        let shade = ShadingInstruction {
            period: Animable {
                start: None,
                period: Duration::from_secs(1),
                value: 1.0,
                animated: false,
                range: (1.0, 2.0),
                limits: (1.0, 2.0),
                normalizing: Normalizing::None {},
            },
            phase: Animable {
                start: None,
                period: Duration::from_secs(1),
                value: 0.0,
                animated: false,
                range: (0.0, 1.0),
                limits: (0.0, 1.0),
                normalizing: Normalizing::None {},
            },
            shading: Shading::Modular {},
        };
        let pe = ColoringInstruction::PaintEscapeTime {
            id: 0,
            opacity: 255,
            color: (0, 0, 0),
            range: 0,
            shading_method: shade,
            normalizing_method: Normalizing::None {},
        };
        let pst = ColoringInstruction::PaintSmallTime {
            id: 1,
            inside_opacity: 0,
            outside_opacity: 30,
            color: (1, 2, 3),
            range: 10,
            shading_method: shade,
            normalizing_method: Normalizing::None {},
        };
        let hif = ColoringInstruction::HighlightInFilaments {
            id: 3,
            opacity: 255,
            color: (0, 0, 0),
        };
        assert_eq!(pe.id(), 0);
        assert_eq!(pst.id(), 1);
        assert_eq!(hif.id(), 3);
        assert_ne!(pe.id(), pst.id());

        // Hash is id-only: payload differences with same id must not diverge.
        let pe_dim = ColoringInstruction::PaintEscapeTime {
            id: 0,
            opacity: 1,
            color: (9, 9, 9),
            range: 99,
            shading_method: shade,
            normalizing_method: Normalizing::Ln {},
        };
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        pe.hash(&mut h1);
        pe_dim.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
        let mut h3 = DefaultHasher::new();
        pst.hash(&mut h3);
        assert_ne!(h1.finish(), h3.finish());
    }
}