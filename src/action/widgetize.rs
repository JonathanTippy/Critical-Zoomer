use eframe::emath::Rect;
use egui::{color_picker, Ui};
use egui_dnd::dnd;
use crate::action::settings::*;
impl Settings {
    pub(crate) fn widgetize(&mut self, ui:&mut Ui) {

        ui.label("bailout radius:");
        self.bailout_radius.widgetize(ui);

        ui.label("order of coloring steps:");

        //ui.add(egui::Slider::new(&mut state.settings.bailout_max_additional_iterations,  0..=100000).logarithmic(true));

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
                    i.widgetize(ui);
                }
            }
        }


    }
}

impl ColoringInstruction {
    pub(crate) fn widgetize(&mut self, ui: &mut Ui) {
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
    pub(crate) fn widgetize(&mut self, ui:&mut Ui) {
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
    pub(crate) fn widgetize(&mut self, ui:&mut Ui) {
        ui.radio_value(self, Shading::Modular{}, "Modular");
        ui.radio_value(self, Shading::Sinus{},"Sinus");
        /*ui.radio_value(self, Shading::Linear{},"Linear");
        ui.radio_value(self, Shading::Histogram{},"Histogram");*/
    }
}
use std::time::*;
impl Animable {
    pub(crate) fn widgetize(&mut self, ui:&mut Ui) {

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
    pub(crate) fn widgetize(&mut self, ui:&mut Ui) {

        ui.radio_value(self, Normalizing::NONE,"None");
        ui.radio_value(self, Normalizing::LNLN,"LnLn");
        ui.radio_value(self, Normalizing::LN,"Ln");
        ui.radio_value(self, Normalizing::RECIPLN,"Reciprocal + Ln");
        ui.radio_value(self, Normalizing::RECIP,"Reciprocal");

    }
}