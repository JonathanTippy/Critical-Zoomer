use eframe::emath::Rect;
use egui::{color_picker, Ui};
use egui_dnd::dnd;
use crate::assemblies::structs::{ColorerMode, EscaperMode, KernelMode};
use crate::settings::*;

fn settings_section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.add_space(6.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            body(ui);
        });
}

impl Settings {
    pub fn widgetize(&mut self, ui:&mut Ui) {

        settings_section(ui, "Coloring", |ui| {
            ui.label("Steps");

            if self.coloring_script.is_none() {
                self.coloring_script = Some(DEFAULT_COLORING_SCRIPT.into());
            }

            let mut items = self.coloring_script.clone().unwrap();

            let mut rect = Rect::ZERO;

            dnd(ui, "dnd_example").show_vec(&mut items, |ui, item, handle, state| {
                ui.horizontal(|ui| {
                    handle.ui(ui, |ui| {
                        ui.label("|☰☰|");
                    });
                    ui.label(*item);
                    ui.radio_value(&mut self.currently_selected_coloring_instruction, item.id(), "select")
                });
            });

            self.coloring_script = Some(items.clone());

            if let Some(s) = &mut self.coloring_script {
                for i in s {
                    if i.id() == self.currently_selected_coloring_instruction {
                        ui.add_space(8.0);
                        i.widgetize(ui);
                    }
                }
            }
        });

        settings_section(ui, "Compute", |ui| {
            ui.label("Bailout radius");
            self.bailout_radius.widgetize(ui);
            ui.add_space(4.0);
            ui.label("Kernel");
            ui.checkbox(&mut self.manual_gear_enabled, "Manual gear");
            ui.add_enabled_ui(self.manual_gear_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.manual_gear,
                        KernelMode::Naive,
                        KernelMode::Naive.manual_gear_label(),
                    );
                    ui.radio_value(
                        &mut self.manual_gear,
                        KernelMode::NaiveGpu,
                        KernelMode::NaiveGpu.manual_gear_label(),
                    );
                    ui.radio_value(
                        &mut self.manual_gear,
                        KernelMode::Pert,
                        KernelMode::Pert.manual_gear_label(),
                    );
                });
            });
            ui.label("Naive GPU and Perturbation are early-dev and still buggy.");
        });

        settings_section(ui, "Shade", |ui| {
            ui.label("Colorer");
            ui.checkbox(&mut self.manual_color_gear_enabled, "Manual color gear");
            ui.add_enabled_ui(self.manual_color_gear_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.manual_color_gear,
                        ColorerMode::Og,
                        ColorerMode::Og.manual_gear_label(),
                    );
                    ui.radio_value(
                        &mut self.manual_color_gear,
                        ColorerMode::Gpu,
                        ColorerMode::Gpu.manual_gear_label(),
                    );
                });
            });
            ui.add_space(4.0);
            ui.label("Escaper");
            ui.checkbox(&mut self.manual_escape_gear_enabled, "Manual escape gear");
            ui.add_enabled_ui(self.manual_escape_gear_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.manual_escape_gear,
                        EscaperMode::Og,
                        EscaperMode::Og.manual_gear_label(),
                    );
                    ui.radio_value(
                        &mut self.manual_escape_gear,
                        EscaperMode::Gpu,
                        EscaperMode::Gpu.manual_gear_label(),
                    );
                });
            });
            ui.add_space(4.0);
            ui.label("C-generator");
            ui.add(egui::Slider::new(&mut self.c_generator_margin_bits, 0..=32).text("margin bits"));
        });

        settings_section(ui, "Eye tracking", |ui| {
            ui.checkbox(&mut self.eye_tracking_enabled, "Enable gaze spiral");
            if ui.button("Calibrate gaze").clicked() {
                self.eye_tracking_enabled = true;
                self.request_gaze_calibrate = true;
            }
            ui.label("Gaze spiral is early-dev and still buggy.");
        });

        ui.add_space(12.0);
    }
}

impl ColoringInstruction {
    pub fn widgetize(&mut self, ui: &mut Ui) {
        match self {
            ColoringInstruction::PaintEscapeTime{
                opacity, color, range, shading_method, normalizing_method, ..
            } => {
                ui.label("Escape Time Coloring Settings");
                ui.label("Escape Time shading method:");
                shading_method.widgetize(ui);
                ui.label("Escape Time normalizing method:");
                normalizing_method.widgetize(ui);
                ui.label("Escape Time range of shading:");
                ui.add(egui::Slider::new(range, 0..=255));
                ui.label("Escape Time color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Escape Time opacity of shading:");
                ui.add(egui::Slider::new(opacity, 0..=255));
            }
            , ColoringInstruction::PaintSmallTime{
                inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
            } => {
                ui.label("Small Time Coloring Settings");
                ui.label("Small Time shading method:");
                shading_method.widgetize(ui);
                ui.label("Small Time normalizing method:");
                normalizing_method.widgetize(ui);
                ui.label("Small Time range of shading:");
                ui.add(egui::Slider::new(range, 0..=255));
                ui.label("Small Time color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Small Time opacity of inside shading:");
                ui.add(egui::Slider::new(inside_opacity, 0..=255));
                ui.label("Small Time opacity of outside shading:");
                ui.add(egui::Slider::new(outside_opacity, 0..=255));
            }
            , ColoringInstruction::PaintSmallness{
                inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
            } => {
                ui.label("Smallness Coloring Settings");
                ui.label("Smallness shading method:");
                shading_method.widgetize(ui);
                ui.label("Smallness normalizing method:");
                normalizing_method.widgetize(ui);
                ui.label("Smallness range of shading:");
                ui.add(egui::Slider::new(range, 0..=255));
                ui.label("Smallness color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Smallness opacity of inside shading:");
                ui.add(egui::Slider::new(inside_opacity, 0..=255));
                ui.label("Smallness opacity of outside shading:");
                ui.add(egui::Slider::new(outside_opacity, 0..=255));
            }
            , ColoringInstruction::HighlightInFilaments{
                opacity, color, ..
            } => {
                ui.label("In Filament Highlighting Settings");
                ui.label("In Filament color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("In Filament opacity of shading:");
                ui.add(egui::Slider::new(opacity, 0..=255));
            }
            , ColoringInstruction::HighlightOutFilaments{
                opacity, color, ..
            } => {
                ui.label("Out Filament Highlighting Settings");
                ui.label("Out Filament color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Out Filament opacity of shading:");
                ui.add(egui::Slider::new(opacity, 0..=255));
            }
            , ColoringInstruction::HighlightNodes{
                inside_opacity, outside_opacity, color, thickness, only_fattest, ..
            } => {
                ui.label("Node Highlighting Settings");
                ui.label("Node Highlighting color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Node Highlighting opacity of inside shading:");
                ui.add(egui::Slider::new(inside_opacity, 0..=255));
                ui.label("Node Highlighting opacity of outside shading:");
                ui.add(egui::Slider::new(outside_opacity, 0..=255));
                ui.label("Node Highlighting thickness:");
                ui.add(egui::Slider::new(thickness, 0..=10));
                ui.label("Node Highlighting only show fattest?:");
                ui.checkbox(only_fattest, "fat");
            }
            , ColoringInstruction::HighlightSmallTimeEdges{
                inside_opacity, outside_opacity, color, ..
            } => {
                ui.label("Small Time Edges Highlighting Settings")
                    .on_hover_text("The pixels where the iteration count at which points reach their smallest changes");
                ui.label("Small Time Edge color of shading:");
                let mut color_array = [color.0, color.1, color.2];
                color_picker::color_edit_button_srgb(ui, &mut color_array);
                *color = (color_array[0], color_array[1], color_array[2]);
                ui.label("Small Time Edge opacity of inside shading:");
                ui.add(egui::Slider::new(inside_opacity, 0..=255));
                ui.label("Small Time Edge opacity of outside shading:");
                ui.add(egui::Slider::new(outside_opacity, 0..=255));
            }
        }
    }
}

impl ShadingInstruction {
    pub fn widgetize(&mut self, ui:&mut Ui) {
        let cyclical = match self.shading {
            Shading::Modular{..} => {true}
            Shading::Sinus{..} => {true}
            /*Shading::Linear{..} => {false}
            Shading::Histogram{..} => {false}*/
        };
        ui.label("Shading");
        self.shading.widgetize(ui);
        if cyclical {
            ui.label("period:");
            self.period.widgetize(ui);
            ui.label("phase:");
            self.phase.widgetize(ui);
        }
    }
}

impl Shading {
    pub fn widgetize(&mut self, ui:&mut Ui) {
        ui.radio_value(self, Shading::Modular{}, "Modular");
        ui.radio_value(self, Shading::Sinus{},"Sinus");
        /*ui.radio_value(self, Shading::Linear{},"Linear");
        ui.radio_value(self, Shading::Histogram{},"Histogram");*/
    }
}
use std::time::*;
impl Animable {
    pub fn widgetize(&mut self, ui:&mut Ui) {

        let formatter = |n, _| {
            let n2 = self.normalizing.reshape_input(&self.limits, &n);
            format!("{}",n2)
        };

        let pre = self.animated;
        ui.checkbox(&mut self.animated, "🔁");
        if !pre && self.animated {self.start = Some(Instant::now())}

        if self.animated {
            ui.label("animation min");
            ui.add(egui::Slider::new(&mut self.range.0, self.limits.0..=self.limits.1).custom_formatter(formatter));
            ui.label("animation max");
            ui.add(egui::Slider::new(&mut self.range.1, self.limits.0..=self.limits.1).custom_formatter(formatter));
            ui.label("animation period");
            let mut period = self.period.as_secs_f64();
            ui.add(egui::Slider::new(&mut period, self.limits.0..=self.limits.1).custom_formatter(formatter));
            self.period = Duration::from_secs_f64(period);
        } else {
            ui.add(egui::Slider::new(&mut self.value, self.limits.0..=self.limits.1).custom_formatter(formatter));
        }
        self.normalizing.widgetize(ui);
    }
}

impl Normalizing {
    pub fn widgetize(&mut self, ui:&mut Ui) {

        ui.radio_value(self, Normalizing::None{},"None");
        ui.radio_value(self, Normalizing::LnLn{},"LnLn");
        ui.radio_value(self, Normalizing::Ln{},"Ln");
        ui.radio_value(self, Normalizing::RecipLn{},"Reciprocal + Ln");
        ui.radio_value(self, Normalizing::Reciprocal{},"Reciprocal");

    }
}