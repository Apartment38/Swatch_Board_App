use std::ffi::OsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerOptions {
    pub radius: u32,
    pub zoom: u32,
    pub diagnostics: bool,
}

impl Default for PickerOptions {
    fn default() -> Self {
        Self {
            radius: 7,
            zoom: 10,
            diagnostics: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Pick(PickerOptions),
}

pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let arguments: Vec<_> = arguments.into_iter().collect();
    let invalid =
        || "unknown argument or invalid value; use swatch-board-picker --help".to_string();
    if arguments.len() == 1 {
        match arguments[0].to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            _ => {}
        }
    }
    let mut options = PickerOptions::default();
    let mut radius_seen = false;
    let mut zoom_seen = false;
    let mut iter = arguments.iter();
    while let Some(argument) = iter.next() {
        match argument.to_str().ok_or_else(invalid)? {
            "--radius" if !radius_seen => {
                options.radius = iter
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(invalid)?;
                if options.radius > 31 {
                    return Err(invalid());
                }
                radius_seen = true;
            }
            "--zoom" if !zoom_seen => {
                options.zoom = iter
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(invalid)?;
                if !(4..=32).contains(&options.zoom) {
                    return Err(invalid());
                }
                zoom_seen = true;
            }
            "--diagnostics" if !options.diagnostics => options.diagnostics = true,
            _ => return Err(invalid()),
        }
    }
    Ok(Command::Pick(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_text(arguments: &[&str]) -> Result<Command, String> {
        parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn defaults_and_adjustable_magnifier_options() {
        assert_eq!(parse_text(&[]), Ok(Command::Pick(PickerOptions::default())));
        assert_eq!(
            parse_text(&["--radius", "3", "--zoom", "12", "--diagnostics"]),
            Ok(Command::Pick(PickerOptions {
                radius: 3,
                zoom: 12,
                diagnostics: true,
            }))
        );
    }

    #[test]
    fn magnifier_bounds_are_enforced() {
        for (radius, zoom) in [("0", "4"), ("31", "32")] {
            let Command::Pick(options) = parse_text(&["--radius", radius, "--zoom", zoom]).unwrap()
            else {
                panic!("not a pick")
            };
            assert_eq!(options.radius.to_string(), radius);
            assert_eq!(options.zoom.to_string(), zoom);
        }
        for arguments in [
            vec!["--radius", "32"],
            vec!["--zoom", "3"],
            vec!["--zoom", "33"],
        ] {
            assert!(parse_text(&arguments).is_err());
        }
    }

    #[test]
    fn informational_options_must_stand_alone() {
        for option in ["--help", "-h"] {
            assert_eq!(parse_text(&[option]), Ok(Command::Help));
        }
        for option in ["--version", "-V"] {
            assert_eq!(parse_text(&[option]), Ok(Command::Version));
        }
        assert!(parse_text(&["--help", "--diagnostics"]).is_err());
        assert!(parse_text(&["--version", "extra"]).is_err());
    }

    #[test]
    fn rejects_removed_duplicate_and_invalid_options() {
        for args in [
            vec!["--radius"],
            vec!["--zoom", "NaN"],
            vec!["--radius", "-1"],
            vec!["--radius", "32"],
            vec!["--zoom", "0"],
            vec!["--zoom", "33"],
            vec!["--radius", "1", "--radius", "2"],
            vec!["--diagnostics", "--diagnostics"],
            vec!["--native"],
            vec!["--repeat", "2"],
            vec![""],
            vec!["--not-an-option"],
        ] {
            assert!(parse_text(&args).is_err(), "accepted {args:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        assert!(parse([OsString::from_vec(vec![0xff])]).is_err());
    }
}
