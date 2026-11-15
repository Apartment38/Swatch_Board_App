use swatch_board_picker_core::{PixelPoint, place_lens};

#[test]
fn lens_keeps_entire_source_neighborhood_clear_at_edges_and_corners() {
    for x in [0, 1, 7, 100, 500, 999, 1000, 1023] {
        for y in [0, 1, 7, 100, 500, 767] {
            let position = place_lens((1024, 768), PixelPoint { x, y }, (200, 200), 8).unwrap();
            assert!(position.x + 200 <= 1024 && position.y + 200 <= 768);
            let source_left = x.saturating_sub(8);
            let source_top = y.saturating_sub(8);
            let source_right = x.saturating_add(9);
            let source_bottom = y.saturating_add(9);
            assert!(
                position.x + 200 <= source_left
                    || position.x >= source_right
                    || position.y + 200 <= source_top
                    || position.y >= source_bottom,
                "lens {position:?} overlaps the source at ({x}, {y})"
            );
        }
    }
}

#[test]
fn narrow_output_uses_vertical_space() {
    let position = place_lens((220, 800), PixelPoint { x: 110, y: 400 }, (200, 200), 8).unwrap();
    assert!(position.y + 200 <= 392 || position.y >= 409);
}

#[test]
fn no_safe_placement_is_reported_instead_of_capturing_the_lens() {
    assert_eq!(
        place_lens((200, 200), PixelPoint { x: 100, y: 100 }, (200, 200), 8),
        None
    );
    assert_eq!(
        place_lens((0, 768), PixelPoint { x: 0, y: 0 }, (200, 200), 8),
        None
    );
    assert_eq!(
        place_lens((1024, 768), PixelPoint { x: 1024, y: 0 }, (200, 200), 8),
        None
    );
}
