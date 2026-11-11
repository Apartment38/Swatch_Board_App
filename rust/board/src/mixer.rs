use eframe::egui;
use swatch_board_color::{
    Color, Format,
    mixing::{MixingSpace, mix},
};

const FORMATS: [(Format, &str); 5] = [
    (Format::HexRgba, "HEX"),
    (Format::Rgb, "RGB"),
    (Format::Rgba, "RGBA"),
    (Format::Hsl, "HSL"),
    (Format::Hsv, "HSV"),
];

#[derive(Clone, PartialEq)]
struct State {
    current: Color,
    sources: Vec<(Color, f64)>,
    space: MixingSpace,
    // This is the Board's active unsaved palette, not capture history.
    palette: Vec<Color>,
}

pub struct Mixer {
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
    fields: [String; 5],
    error: Option<String>,
}

impl Default for Mixer {
    fn default() -> Self {
        let mut mixer = Self {
            state: State {
                current: Color::new(36, 122, 195, 255),
                sources: vec![
                    (Color::new(255, 0, 0, 255), 1.0),
                    (Color::new(0, 0, 255, 255), 1.0),
                ],
                space: MixingSpace::Srgb,
                palette: Vec::new(),
            },
            undo: Vec::new(),
            redo: Vec::new(),
            fields: Default::default(),
            error: None,
        };
        mixer.sync_fields();
        mixer
    }
}

impl Mixer {
    fn sync_fields(&mut self) {
        for (field, (format, _)) in self.fields.iter_mut().zip(FORMATS) {
            *field = self.state.current.format(format);
        }
        self.error = None;
    }

    fn commit(&mut self, previous: State) {
        if self.state != previous {
            // Bound memory without retaining an unbounded editing session.
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
            self.sync_fields();
        }
    }

    pub fn select(&mut self, color: Color) {
        let previous = self.state.clone();
        self.state.current = color;
        self.commit(previous);
        self.sync_fields();
    }

    fn apply_field(&mut self, index: usize) {
        let text = self.fields[index].trim();
        let expected = match index {
            0 => text.starts_with('#'),
            1 => text.starts_with("rgb("),
            2 => text.starts_with("rgba("),
            3 => text.starts_with("hsl("),
            4 => text.starts_with("hsv("),
            _ => false,
        };
        match if expected {
            text.parse::<Color>()
        } else {
            Err(swatch_board_color::ParseColorError)
        } {
            Ok(mut color) => {
                // These representations have no alpha channel: editing one must
                // not silently turn a translucent selection opaque.
                if matches!(FORMATS[index].0, Format::Rgb | Format::Hsl | Format::Hsv) {
                    color.a = self.state.current.a;
                }
                self.select(color);
            }
            Err(_) => {
                self.error = Some(format!(
                    "Invalid {} value; current color is unchanged.",
                    FORMATS[index].1
                ))
            }
        }
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, previous));
            self.sync_fields();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, next));
            self.sync_fields();
        }
    }

    fn blend(&mut self) {
        match mix(&self.state.sources, self.state.space) {
            Ok(color) => self.state.current = color,
            Err(error) => self.error = Some(error.into()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, format: Format) {
        ui.horizontal(|ui| {
            ui.heading("Color Mixer");
            if ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }
        });
        egui::ScrollArea::vertical().id_salt("mixer_body").show(ui, |ui| {
            let current = self.state.current;
            let target = ui.dnd_drop_zone::<Color, _>(egui::Frame::group(ui.style()), |ui| {
                swatch(ui, current, egui::vec2(ui.available_width().max(80.0), 90.0));
                ui.label("Current color · click history or drop a history swatch here");
            });
            if let Some(color) = target.1 {
                self.select(*color);
            }
            egui::Grid::new("color_values").num_columns(3).show(ui, |ui| {
                for (index, (_, label)) in FORMATS.iter().enumerate() {
                    ui.label(*label);
                    let response = ui.add(egui::TextEdit::singleline(&mut self.fields[index]).desired_width(300.0));
                    let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Apply").clicked() || enter {
                        self.apply_field(index);
                    }
                    ui.end_row();
                }
            });
            ui.label(format!("CMYK (approximate, unprofiled): {}", self.state.current.format(Format::CmykApprox)));
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Configured output: {}", self.state.current.format(format)));
                if ui.button("Copy current color").clicked() {
                    ui.ctx().copy_text(self.state.current.format(format));
                }
                if ui.button("Add to active palette").clicked() {
                    let previous = self.state.clone();
                    self.state.palette.push(self.state.current);
                    self.commit(previous);
                }
            });
            ui.separator();
            ui.strong("Mix source colors");
            ui.label("Weights are normalized. All-zero weights leave the current color unchanged.");
            let previous = self.state.clone();
            let mut changed = false;
            ui.horizontal(|ui| {
                changed |= ui.selectable_value(&mut self.state.space, MixingSpace::Srgb, "sRGB").changed();
                changed |= ui.selectable_value(&mut self.state.space, MixingSpace::Oklch, "OKLCH (perceptual)").changed();
                if ui.button("Add current as source").clicked() {
                    self.state.sources.push((self.state.current, 1.0));
                    changed = true;
                }
            });
            let mut remove = None;
            let can_remove = self.state.sources.len() > 2;
            for (index, (color, weight)) in self.state.sources.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        let (_, dropped) = ui.dnd_drop_zone::<Color, _>(egui::Frame::default(), |ui| {
                            swatch(ui, *color, egui::vec2(36.0, 26.0));
                        });
                        if let Some(dropped) = dropped {
                            *color = *dropped;
                            changed = true;
                        }
                        ui.label(color.format(Format::HexRgba));
                        changed |= ui.add(egui::Slider::new(weight, 0.0..=1.0).text(format!("Source {} weight", index + 1))).changed();
                        if ui.add_enabled(can_remove, egui::Button::new("Remove")).clicked() {
                            remove = Some(index);
                        }
                    });
                });
            }
            if let Some(index) = remove {
                self.state.sources.remove(index);
                changed = true;
            }
            if changed || ui.button("Mix sources").clicked() {
                self.blend();
                // Preserve the error across field synchronization on a weight edit.
                let error = self.error.take();
                self.commit(previous);
                self.error = error;
            }
            ui.small("OKLCH: shortest hue arc from the first chromatic source; achromatic hues ignored. Out-of-gamut RGB channels are clipped. Alpha is weighted independently.");
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.separator();
            ui.strong(format!("Active palette · unsaved draft · {} colors", self.state.palette.len()));
            ui.label("Draft stays available across tabs and hide/restore; closing Board discards it. Palette saving arrives in Phase 4.");
            ui.horizontal_wrapped(|ui| {
                for color in &self.state.palette {
                    ui.vertical(|ui| {
                        swatch(ui, *color, egui::vec2(48.0, 30.0));
                        ui.label(color.format(Format::HexRgba));
                    });
                }
            });
        });
    }
}

pub fn swatch(ui: &mut egui::Ui, color: Color, size: egui::Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    // Checkerboard makes alpha visible rather than presenting it as opaque RGB.
    let tile = 10.0;
    for y in 0..(rect.height() / tile).ceil() as usize {
        for x in 0..(rect.width() / tile).ceil() as usize {
            let cell = egui::Rect::from_min_size(
                rect.min + egui::vec2(x as f32 * tile, y as f32 * tile),
                egui::vec2(tile, tile),
            )
            .intersect(rect);
            ui.painter().rect_filled(
                cell,
                0.0,
                if (x + y) % 2 == 0 {
                    egui::Color32::GRAY
                } else {
                    egui::Color32::LIGHT_GRAY
                },
            );
        }
    }
    ui.painter().rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_different_colors_appends_to_draft() {
        fn render(mixer: &mut Mixer, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::Pos2 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 1200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        mixer.ui(ui, Format::HexRgb);
                    });
                },
            );
            output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.text() == "Add to active palette"
                    {
                        Some(text.pos + text.galley.size() * 0.5)
                    } else {
                        None
                    }
                })
                .expect("add button is visible")
        }
        let mut mixer = Mixer::default();
        let ctx = egui::Context::default();
        let colors = [Color::new(255, 0, 0, 255), Color::new(0, 0, 255, 255)];
        for color in colors {
            mixer.select(color);
            let pos = render(&mut mixer, &ctx, vec![]);
            render(
                &mut mixer,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            render(
                &mut mixer,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert_eq!(mixer.state.palette, colors);
        mixer.undo();
        assert_eq!(mixer.state.palette, colors[..1]);
        mixer.redo();
        assert_eq!(mixer.state.palette, colors);
    }

    #[test]
    fn invalid_edit_preserves_selection_and_undo() {
        let mut mixer = Mixer::default();
        let initial = mixer.state.current;
        mixer.fields[0] = "#XYZ123".into();
        mixer.apply_field(0);
        assert_eq!(mixer.state.current, initial);
        assert!(mixer.undo.is_empty());
        assert!(mixer.error.is_some());
    }

    #[test]
    fn editing_non_alpha_formats_preserves_alpha_and_supports_undo_redo() {
        let mut mixer = Mixer::default();
        let translucent = Color::new(12, 34, 56, 78);
        mixer.select(translucent);
        mixer.fields[1] = "rgb(255, 0, 0)".into();
        mixer.apply_field(1);
        assert_eq!(mixer.state.current, Color::new(255, 0, 0, 78));
        mixer.undo();
        assert_eq!(mixer.state.current, translucent);
        mixer.redo();
        assert_eq!(mixer.state.current, Color::new(255, 0, 0, 78));
        mixer.undo();
        mixer.select(Color::new(1, 2, 3, 4));
        assert!(mixer.redo.is_empty());
    }

    #[test]
    fn zero_weight_mix_does_not_replace_current_color() {
        let mut mixer = Mixer::default();
        let initial = mixer.state.current;
        for (_, weight) in &mut mixer.state.sources {
            *weight = 0.0;
        }
        mixer.blend();
        assert_eq!(mixer.state.current, initial);
        assert!(mixer.error.is_some());
    }
}
