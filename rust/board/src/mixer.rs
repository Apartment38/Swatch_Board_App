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
    hsv: [f32; 3],
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
                hsv: hsv(Color::new(36, 122, 195, 255)),
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
    pub fn colors(&self) -> &[Color] {
        &self.state.palette
    }

    pub fn replace_palette(&mut self, colors: Vec<Color>) {
        if self.state.palette != colors {
            self.state.palette = colors;
            // Do not let an old mixer snapshot resurrect a different palette.
            self.undo.clear();
            self.redo.clear();
        }
    }

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
        let mut channels = hsv(color);
        if channels[1] == 0.0 {
            channels[0] = self.state.hsv[0];
        }
        self.state.hsv = channels;
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
            Ok(color) => {
                self.state.current = color;
                self.state.hsv = hsv(color);
            }
            Err(error) => self.error = Some(error.into()),
        }
    }

    fn channel_editor(&mut self, ui: &mut egui::Ui) {
        let previous = self.state.clone();
        let mut changed = spectrum(ui, &mut self.state.hsv);
        let mut channels = [
            self.state.hsv[0] * 360.0,
            self.state.hsv[1] * 100.0,
            self.state.hsv[2] * 100.0,
        ];
        ui.horizontal_wrapped(|ui| {
            for (index, label) in ["Hue °", "Sat %", "Value %"].iter().enumerate() {
                ui.label(*label);
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut channels[index])
                            .range(0.0..=if index == 0 { 360.0 } else { 100.0 })
                            .speed(0.5),
                    )
                    .changed();
            }
        });
        if changed {
            self.state.hsv = [
                channels[0] / 360.0,
                channels[1] / 100.0,
                channels[2] / 100.0,
            ];
            self.state.current = from_hsv(self.state.hsv, self.state.current.a);
        }
        let mut color = self.state.current;
        let mut rgb_changed = false;
        ui.horizontal_wrapped(|ui| {
            for (label, channel) in [
                ("Red", &mut color.r),
                ("Green", &mut color.g),
                ("Blue", &mut color.b),
                ("Alpha", &mut color.a),
            ] {
                ui.label(label);
                rgb_changed |= ui
                    .add(egui::DragValue::new(channel).range(0..=255))
                    .changed();
            }
        });
        if rgb_changed {
            if (color.r, color.g, color.b)
                != (
                    self.state.current.r,
                    self.state.current.g,
                    self.state.current.b,
                )
            {
                self.state.hsv = hsv(color);
            }
            self.state.current = color;
        }
        self.commit(previous);
    }

    pub fn palette_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "Active palette · {} colors",
                self.state.palette.len()
            ));
            if ui
                .add_enabled(
                    self.state.palette.len() < 64,
                    egui::Button::new("Add current color"),
                )
                .clicked()
            {
                let previous = self.state.clone();
                self.state.palette.push(self.state.current);
                self.commit(previous);
            }
        });
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
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        self.channel_editor(ui);
        egui::ScrollArea::vertical().id_salt("mixer_details").show(ui, |ui| {
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
            ui.small("The active palette is pinned below the mixer.");
        });
    }
}

fn hsv(color: Color) -> [f32; 3] {
    let value = egui::ecolor::Hsva::from_rgb([
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
    ]);
    [value.h, value.s, value.v]
}

fn from_hsv([h, s, v]: [f32; 3], alpha: u8) -> Color {
    let [r, g, b] = egui::ecolor::Hsva::new(h, s, v, 1.0).to_rgb();
    Color::new(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        alpha,
    )
}

fn spectrum(ui: &mut egui::Ui, hsv: &mut [f32; 3]) -> bool {
    let mut changed = false;
    let width = (ui.available_width() - 36.0).clamp(120.0, 480.0);
    let height = (width * 0.65)
        .min(ui.available_height() * 0.45)
        .clamp(80.0, 250.0);
    ui.horizontal(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
        // Keep endpoint values and their marker inside the pointer hit region.
        let rect = rect.shrink(6.0);
        if let Some(pos) = response.interact_pointer_pos()
            && (response.clicked() || response.dragged())
        {
            hsv[0] = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            hsv[1] = (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
            changed = true;
        }
        let mut mesh = egui::Mesh::default();
        for sector in 0..=6 {
            let hue = sector as f32 / 6.0;
            let color = from_hsv([hue, 1.0, 1.0], 255);
            let x = egui::lerp(rect.x_range(), hue);
            mesh.colored_vertex(
                egui::pos2(x, rect.top()),
                egui::Color32::from_rgb(color.r, color.g, color.b),
            );
            mesh.colored_vertex(egui::pos2(x, rect.bottom()), egui::Color32::WHITE);
            if sector < 6 {
                let i = sector * 2;
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i + 1, i + 3, i + 2);
            }
        }
        ui.painter().add(egui::Shape::mesh(mesh));
        let marker = egui::pos2(
            egui::lerp(rect.x_range(), hsv[0]),
            egui::lerp(rect.y_range(), 1.0 - hsv[1]),
        );
        ui.painter().circle_stroke(
            marker,
            5.0,
            egui::Stroke::new(3.0_f32, egui::Color32::BLACK),
        );
        ui.painter().circle_stroke(
            marker,
            5.0,
            egui::Stroke::new(1.0_f32, egui::Color32::WHITE),
        );
        response
            .on_hover_text("Hue left to right; saturation bottom to top. Click or drag to select.");
        let (bar, response) =
            ui.allocate_exact_size(egui::vec2(24.0, height), egui::Sense::click_and_drag());
        let bar = bar.shrink2(egui::vec2(0.0, 6.0));
        if let Some(pos) = response.interact_pointer_pos()
            && (response.clicked() || response.dragged())
        {
            hsv[2] = (1.0 - (pos.y - bar.top()) / bar.height()).clamp(0.0, 1.0);
            changed = true;
        }
        let color = from_hsv([hsv[0], hsv[1], 1.0], 255);
        let top = egui::Color32::from_rgb(color.r, color.g, color.b);
        let mut mesh = egui::Mesh::default();
        for (pos, color) in [
            (bar.left_top(), top),
            (bar.right_top(), top),
            (bar.left_bottom(), egui::Color32::BLACK),
            (bar.right_bottom(), egui::Color32::BLACK),
        ] {
            mesh.colored_vertex(pos, color);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        ui.painter().add(egui::Shape::mesh(mesh));
        let y = egui::lerp(bar.y_range(), 1.0 - hsv[2]);
        for (width, color) in [
            (3.0_f32, egui::Color32::BLACK),
            (1.0_f32, egui::Color32::WHITE),
        ] {
            ui.painter().line_segment(
                [egui::pos2(bar.left(), y), egui::pos2(bar.right(), y)],
                egui::Stroke::new(width, color),
            );
        }
        response.on_hover_text("Brightness / HSV value. Click or drag to adjust.");
    });
    changed
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

    fn mixer_frame(
        mixer: &mut Mixer,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        mixer_frame_at(mixer, ctx, events, egui::vec2(960.0, 640.0), 145.0)
    }

    fn mixer_frame_at(
        mixer: &mut Mixer,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        size: egui::Vec2,
        palette_height: f32,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::TopBottomPanel::top("toolbar")
                    .exact_height(54.0)
                    .show(ctx, |_| {});
                egui::TopBottomPanel::bottom("active_palette")
                    .resizable(true)
                    .exact_height(palette_height)
                    .min_height(130.0)
                    .max_height(220.0)
                    .show(ctx, |ui| mixer.palette_ui(ui));
                egui::SidePanel::left("history_sidebar")
                    .default_width(200.0)
                    .show(ctx, |_| {});
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Ready. History is saved automatically.");
                    ui.add_space(8.0);
                    mixer.ui(ui, Format::HexRgb);
                });
            },
        )
    }

    fn visible_text_position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == label
            {
                let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                if shape.clip_rect.contains(bounds.center()) {
                    return Some(bounds.center());
                }
            }
            None
        })
    }

    fn complete_text_visible(output: &egui::FullOutput, label: &str) -> bool {
        output.shapes.iter().any(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                text.galley.text() == label
                    && shape
                        .clip_rect
                        .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
            } else {
                false
            }
        })
    }

    fn mixer_click(mixer: &mut Mixer, ctx: &egui::Context, pos: egui::Pos2) -> egui::FullOutput {
        let mut output = egui::FullOutput::default();
        for pressed in [true, false] {
            output = mixer_frame(
                mixer,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        output
    }

    #[test]
    fn invalid_apply_has_visible_feedback_in_default_board_layout() {
        let mut mixer = Mixer::default();
        let initial = mixer.state.current;
        let ctx = egui::Context::default();
        mixer_frame(&mut mixer, &ctx, vec![]);
        let output = mixer_frame(&mut mixer, &ctx, vec![]);
        let field = visible_text_position(&output, "#247AC3FF").expect("HEX field visible");
        mixer_click(&mut mixer, &ctx, field);
        mixer_frame(
            &mut mixer,
            &ctx,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Text("not-a-color".into()),
            ],
        );
        assert_eq!(
            mixer.fields[0], "not-a-color",
            "actual field editing is retained"
        );
        let output = mixer_frame(&mut mixer, &ctx, vec![]);
        let apply = visible_text_position(&output, "Apply").expect("HEX Apply visible");
        mixer_click(&mut mixer, &ctx, apply);
        let output = mixer_frame(&mut mixer, &ctx, vec![]);
        assert_eq!(mixer.state.current, initial);
        assert!(mixer.undo.is_empty());
        let error = mixer
            .error
            .as_deref()
            .expect("Apply processes invalid input")
            .to_owned();
        assert!(
            complete_text_visible(&output, &error),
            "Apply must show its complete error without requiring an unrelated scroll"
        );
        for size in [egui::vec2(960.0, 640.0), egui::vec2(540.0, 360.0)] {
            for palette_height in [145.0, 220.0] {
                mixer_frame_at(&mut mixer, &ctx, vec![], size, palette_height);
                let output = mixer_frame_at(&mut mixer, &ctx, vec![], size, palette_height);
                assert!(
                    complete_text_visible(&output, &error),
                    "complete error visible at {size:?}, palette height {palette_height}"
                );
            }
        }
    }

    fn mixer_key(mixer: &mut Mixer, ctx: &egui::Context, key: egui::Key) {
        mixer_frame(
            mixer,
            ctx,
            vec![true, false]
                .into_iter()
                .map(|pressed| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                })
                .collect(),
        );
    }

    fn activate_mixer_button(mixer: &mut Mixer, ctx: &egui::Context, label: &str, keyboard: bool) {
        let output = mixer_frame(mixer, ctx, vec![]);
        let target = visible_text_position(&output, label).expect("button label visible");
        if keyboard {
            for _ in 0..120 {
                let focused = ctx
                    .memory(|memory| memory.focused())
                    .and_then(|id| ctx.read_response(id));
                if focused.is_some_and(|response| response.rect.contains(target)) {
                    mixer_key(mixer, ctx, egui::Key::Enter);
                    return;
                }
                mixer_key(mixer, ctx, egui::Key::Tab);
            }
            panic!("keyboard could not reach {label}");
        } else {
            mixer_click(mixer, ctx, target);
        }
    }

    #[test]
    fn actual_apply_undo_redo_work_with_pointer_and_keyboard_for_changed_color() {
        for keyboard in [false, true] {
            let mut mixer = Mixer::default();
            let initial = mixer.state.current;
            let changed = Color::new(224, 108, 117, 255);
            assert_ne!(initial, changed);
            let ctx = egui::Context::default();
            mixer_frame(&mut mixer, &ctx, vec![]);
            let output = mixer_frame(&mut mixer, &ctx, vec![]);
            let field = visible_text_position(&output, "#247AC3FF").expect("HEX field visible");
            mixer_click(&mut mixer, &ctx, field);
            mixer_frame(
                &mut mixer,
                &ctx,
                vec![
                    egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    },
                    egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    },
                    egui::Event::Text("#E06C75FF".into()),
                ],
            );
            assert_eq!(mixer.fields[0], "#E06C75FF");
            activate_mixer_button(&mut mixer, &ctx, "Apply", keyboard);
            assert_eq!(mixer.state.current, changed, "Apply keyboard={keyboard}");
            assert!(mixer.error.is_none());
            activate_mixer_button(&mut mixer, &ctx, "Undo", keyboard);
            assert_eq!(mixer.state.current, initial, "Undo keyboard={keyboard}");
            activate_mixer_button(&mut mixer, &ctx, "Redo", keyboard);
            assert_eq!(mixer.state.current, changed, "Redo keyboard={keyboard}");
        }
    }

    #[test]
    fn spectrum_click_changes_hue_and_saturation_without_changing_value() {
        let ctx = egui::Context::default();
        let mut channels = [0.0, 0.0, 0.5];
        let mut rect = egui::Rect::NOTHING;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rect = ui.available_rect_before_wrap();
                spectrum(ui, &mut channels);
            });
        });
        let pos = rect.min + egui::vec2(240.0, 1.0);
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        spectrum(ui, &mut channels);
                    });
                },
            );
        }
        let color = from_hsv(channels, 77);
        assert_eq!(channels[1], 1.0, "top edge reaches full saturation");
        assert_eq!(color, Color::new(0, 128, 128, 77));
    }

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
                    egui::TopBottomPanel::bottom("palette").show(ctx, |ui| {
                        mixer.palette_ui(ui);
                    });
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
                        && text.galley.text() == "Add current color"
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
