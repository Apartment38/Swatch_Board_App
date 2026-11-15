//! Headless integrated production layout checks, not native window evidence.
use super::*;
use swatch_board_color::Color;

// Match the theme tests' subprocess pattern: never mutate process-global XDG
// variables or open the user's normal repositories from a regression test.
fn isolated(name: &str) -> bool {
    if std::env::var("SWATCH_LAYOUT_TEST").as_deref() == Ok(name) {
        return false;
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/layout-revision")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("layout_tests::{name}"), "--nocapture"])
        .env("SWATCH_LAYOUT_TEST", name)
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_DIRS", root.join("system"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

struct Layout {
    board: Board,
    ctx: egui::Context,
    size: egui::Vec2,
    output: egui::FullOutput,
}

impl Layout {
    fn new() -> Self {
        let (tx, _) = std::sync::mpsc::channel();
        let (_, rx) = std::sync::mpsc::channel();
        let mut board = Board::new(tx, rx);
        board.loaded = true;
        board.message = "History is saved automatically.".into();
        board.mixer.replace_palette(
            (0..8)
                .map(|i| Color::new(30 + i * 20, 70, 130, 255))
                .collect(),
        );
        let ctx = egui::Context::default();
        ctx.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size = 14.0;
            }
        });
        let mut app = Self {
            board,
            ctx,
            size: egui::vec2(540.0, 360.0),
            output: egui::FullOutput::default(),
        };
        app.settle();
        app
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| self.board.show_ui(ctx),
        );
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame(vec![]);
        }
    }

    fn bounds(&self, label: &str) -> Option<(egui::Rect, egui::Rect)> {
        self.output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == label
            {
                Some((
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                    shape.clip_rect,
                ))
            } else {
                None
            }
        })
    }

    fn visible(&self, label: &str) -> bool {
        self.bounds(label).is_some_and(|(bounds, clip)| {
            clip.contains_rect(bounds)
                && egui::Rect::from_min_size(egui::Pos2::ZERO, self.size).contains_rect(bounds)
        })
    }

    fn click(&mut self, label: &str) {
        assert!(
            self.visible(label),
            "{label} must be visible before clicking: {:?}",
            self.bounds(label)
        );
        let pos = self.bounds(label).unwrap().0.center();
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }

    fn panel_height(&mut self, height: f32) {
        self.ctx.data_mut(|data| {
            data.insert_persisted(
                egui::Id::new("active_palette"),
                egui::containers::panel::PanelState {
                    rect: egui::Rect::from_min_max(
                        egui::pos2(0.0, self.size.y - height),
                        egui::pos2(self.size.x, self.size.y),
                    ),
                },
            )
        });
        self.settle();
    }

    fn wheel(&mut self, pos: egui::Pos2, delta: egui::Vec2) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.settle();
    }
}

#[test]
fn integrated_minimum_client_font14_palette_details_reachable_with_locked_refusal() {
    if isolated("integrated_minimum_client_font14_palette_details_reachable_with_locked_refusal") {
        return;
    }
    for size in [egui::vec2(540.0, 360.0), egui::vec2(960.0, 640.0)] {
        for height in [130.0, 145.0, 220.0] {
            let mut app = Layout::new();
            app.size = size;
            app.panel_height(height);
            app.click("Lock");
            app.click("Delete");
            let refusal = "Swatch 1 is locked. Unlock it before deleting.";
            assert!(
                app.visible(refusal),
                "complete refusal: {:?}",
                app.bounds(refusal)
            );
            assert_eq!(app.board.mixer.colors().len(), 8);
            assert_eq!(app.board.mixer.colors()[0], Color::new(30, 70, 130, 255));

            let scroll_y = app
                .bounds(&app.board.mixer.colors()[0].format(Format::HexRgba))
                .expect("first HEX is painted")
                .0
                .center()
                .y;
            let mut all_details = std::collections::BTreeSet::new();
            for _ in 0..24 {
                for i in 0..8 {
                    let name = format!("Swatch {}", i + 1);
                    let hex = app.board.mixer.colors()[i].format(Format::HexRgba);
                    if app.visible(&name) && app.visible(&hex) {
                        all_details.insert(i);
                    }
                }
                app.wheel(egui::pos2(270.0, scroll_y), egui::vec2(-40.0, 0.0));
            }
            println!(
                "client={size:?}; requested panel height={height}; all swatch details reached: {all_details:?}; refusal={:?}",
                app.bounds(refusal)
            );
            assert_eq!(
                all_details.len(),
                8,
                "all names and HEX values reachable horizontally"
            );
            // All eight were already reachable before the repair: characterization,
            // not proof that the critic's native screenshot was a product clipping bug.
            assert!(
                app.visible(refusal),
                "refusal remains visible when details scroll"
            );
            for label in [
                "Unlock",
                "Edit",
                "Delete",
                "Earlier",
                "Later",
                "Add current color",
            ] {
                assert!(
                    app.visible(label),
                    "essential action {label} remains visible"
                );
            }
        }
    }
}

#[test]
fn integrated_palette_preserves_expanded_height() {
    if isolated("integrated_palette_preserves_expanded_height") {
        return;
    }
    let mut app = Layout::new();
    app.panel_height(220.0);
    let rect = egui::containers::panel::PanelState::load(&app.ctx, egui::Id::new("active_palette"))
        .unwrap()
        .rect;
    println!("requested palette height=220; actual={rect:?}");
    assert!(
        rect.height() >= 219.0,
        "content shrink must not defeat the user's expanded panel"
    );
}

#[test]
fn integrated_minimum_client_font14_mixer_apply_reachable_above_expanded_palette() {
    if isolated("integrated_minimum_client_font14_mixer_apply_reachable_above_expanded_palette") {
        return;
    }
    let mut app = Layout::new();
    app.panel_height(220.0);
    let mut reached = app.visible("Apply");
    for _ in 0..40 {
        app.wheel(egui::pos2(480.0, 110.0), egui::vec2(0.0, -20.0));
        reached |= app.visible("Apply");
    }
    println!(
        "540x360 client, font14, history open: Apply reached={reached}; bounds={:?}",
        app.bounds("Apply")
    );
    assert!(
        reached,
        "mixer Apply must be reachable by scrolling, not clipped by the unscrollable channel editor above it"
    );
    assert_eq!(app.board.mixer.colors().len(), 8);
}
