use swatch_board_color::{Color, Format};

#[test]
fn golden_formats() {
    let color = Color::new(255, 128, 0, 128);
    for (format, expected) in [
        (Format::HexRgb, "#FF8000"),
        (Format::HexRgba, "#FF800080"),
        (Format::Rgb, "rgb(255, 128, 0)"),
        (Format::Rgba, "rgba(255, 128, 0, 0.502)"),
        (Format::Hsl, "hsl(30.12, 100.00%, 50.00%)"),
        (Format::Hsv, "hsv(30.12, 100.00%, 100.00%)"),
        (Format::CmykApprox, "cmyk(0.00%, 49.80%, 100.00%, 0.00%)"),
    ] {
        assert_eq!(color.format(format), expected);
        let parsed: Color = expected.parse().unwrap();
        let alpha = if matches!(format, Format::HexRgba | Format::Rgba) {
            128
        } else {
            255
        };
        assert_eq!(parsed, Color::new(255, 128, 0, alpha));
    }
}

#[test]
fn parsing_converts_and_rounds_to_canonical_rgba() {
    for (input, expected) in [
        (" #aB12eF ", Color::new(171, 18, 239, 255)),
        ("#00112233", Color::new(0, 17, 34, 51)),
        ("rgb( 0, 127, 255 )", Color::new(0, 127, 255, 255)),
        ("rgba(1, 2, 3, 0.5)", Color::new(1, 2, 3, 128)),
        ("hsl(120deg, 100%, 50%)", Color::new(0, 255, 0, 255)),
        ("hsl(360, 100%, 50%)", Color::new(255, 0, 0, 255)),
        ("hsv(240, 100%, 100%)", Color::new(0, 0, 255, 255)),
        ("cmyk(100%, 0%, 0%, 0%)", Color::new(0, 255, 255, 255)),
        ("cmyk(0%, 0%, 0%, 100%)", Color::new(0, 0, 0, 255)),
    ] {
        assert_eq!(input.parse::<Color>().unwrap(), expected, "{input}");
    }
}

#[test]
fn rejects_invalid_values_and_syntax() {
    for input in [
        "",
        "#fff",
        "#1234567",
        "#123456789",
        "#12xx56",
        "#ééé",
        "rgb(256, 0, 0)",
        "rgb(-1, 0, 0)",
        "rgb(1.0, 0, 0)",
        "rgb(0, 0)",
        "rgb(0, 0, 0, 1)",
        "rgb(0, 0, 0) trailing",
        "rgba(0, 0, 0, -0.1)",
        "rgba(0, 0, 0, 1.1)",
        "rgba(0, 0, 0, NaN)",
        "hsl(361, 50%, 50%)",
        "hsl(0, 50, 50%)",
        "hsv(0, 101%, 50%)",
        "hsv(inf, 0%, 0%)",
        "cmyk(0%, 0%, 0%, -1%)",
        "cmyk(0%, 0%, 0%)",
    ] {
        assert!(input.parse::<Color>().is_err(), "accepted {input:?}");
    }
}

#[test]
fn achromatic_and_primary_golden_values() {
    for (color, hsl, hsv, cmyk) in [
        (
            Color::new(0, 0, 0, 255),
            "hsl(0.00, 0.00%, 0.00%)",
            "hsv(0.00, 0.00%, 0.00%)",
            "cmyk(0.00%, 0.00%, 0.00%, 100.00%)",
        ),
        (
            Color::new(255, 255, 255, 255),
            "hsl(0.00, 0.00%, 100.00%)",
            "hsv(0.00, 0.00%, 100.00%)",
            "cmyk(0.00%, 0.00%, 0.00%, 0.00%)",
        ),
        (
            Color::new(255, 0, 0, 255),
            "hsl(0.00, 100.00%, 50.00%)",
            "hsv(0.00, 100.00%, 100.00%)",
            "cmyk(0.00%, 100.00%, 100.00%, 0.00%)",
        ),
    ] {
        assert_eq!(color.format(Format::Hsl), hsl);
        assert_eq!(color.format(Format::Hsv), hsv);
        assert_eq!(color.format(Format::CmykApprox), cmyk);
    }
}

#[test]
fn supported_format_names() {
    for (name, format) in [
        ("hex", Format::HexRgb),
        ("hex-rgba", Format::HexRgba),
        ("rgb", Format::Rgb),
        ("rgba", Format::Rgba),
        ("hsl", Format::Hsl),
        ("hsv", Format::Hsv),
        ("cmyk", Format::CmykApprox),
    ] {
        assert_eq!(name.parse::<Format>().unwrap(), format);
    }
    assert!("unknown".parse::<Format>().is_err());
}

#[test]
fn formatted_samples_round_trip() {
    for r in (0..=255).step_by(17) {
        for g in (0..=255).step_by(17) {
            for b in (0..=255).step_by(17) {
                let color = Color::new(r, g, b, 255);
                for format in [
                    Format::HexRgb,
                    Format::HexRgba,
                    Format::Rgb,
                    Format::Rgba,
                    Format::Hsl,
                    Format::Hsv,
                    Format::CmykApprox,
                ] {
                    let text = color.format(format);
                    assert_eq!(text.parse::<Color>().unwrap(), color, "{format:?}: {text}");
                }
            }
        }
    }
    for a in 0..=255 {
        let color = Color::new(1, 2, 3, a);
        assert_eq!(color.format(Format::Rgba).parse::<Color>().unwrap(), color);
        assert_eq!(
            color.format(Format::HexRgba).parse::<Color>().unwrap(),
            color
        );
    }
}
