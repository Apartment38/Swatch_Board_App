mod native;

use std::error::Error;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Pick(Option<u32>),
}

fn parse(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> Result<Command, &'static str> {
    let mut arguments = arguments.into_iter();
    let command = match arguments.next() {
        None => Command::Pick(None),
        Some(argument) if argument == "--help" || argument == "-h" => Command::Help,
        Some(argument) if argument == "--version" || argument == "-V" => Command::Version,
        Some(argument) if argument == "--zoom" => {
            let zoom = arguments
                .next()
                .and_then(|value| value.to_str().and_then(|value| value.parse::<u32>().ok()))
                .filter(|zoom| (1..=32).contains(zoom))
                .ok_or("zoom must be an integer from 1 to 32; use --help")?;
            Command::Pick(Some(zoom))
        }
        _ => return Err("unknown argument; use swatch-board-picker --help"),
    };
    if arguments.next().is_some() {
        return Err("unknown argument; use swatch-board-picker --help");
    }
    Ok(command)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let zoom = match parse(std::env::args_os().skip(1))? {
        Command::Help => {
            println!(
                "Swatch Board Picker — live KDE compositor picker\n\nUsage: swatch-board-picker [--zoom N]\n       swatch-board-picker --help | --version\n\nSelect with left click or Enter; cancel with Escape or right click.\nThe matching Swatch Board KWin effect provides a pixel-square crosshair,\nlive color swatch and HEX readout. No screen-sharing portal is used.\nZoom: 1..32; uses the saved picker zoom when omitted (default 4)."
            );
            return Ok(());
        }
        Command::Version => {
            println!("Swatch Board Picker {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Command::Pick(Some(zoom)) => zoom,
        Command::Pick(None) => swatch_board_config_core::Config::load()?.picker_zoom,
    };

    match native::pick(zoom).await? {
        native::Outcome::Selected(rgb) => println!("#{rgb:06X}"),
        native::Outcome::Cancelled => eprintln!("Color selection cancelled"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Result<Command, &'static str> {
        parse(values.iter().map(std::ffi::OsString::from))
    }

    #[test]
    fn zoom_override_is_bounded_and_unambiguous() {
        assert_eq!(arguments(&[]), Ok(Command::Pick(None)));
        for zoom in [1, 4, 32] {
            assert_eq!(
                arguments(&["--zoom", &zoom.to_string()]),
                Ok(Command::Pick(Some(zoom)))
            );
        }
        for invalid in ["0", "33", "-1", "1.5", "invalid"] {
            assert!(arguments(&["--zoom", invalid]).is_err());
        }
        assert!(arguments(&["--zoom"]).is_err());
        assert!(arguments(&["--zoom", "4", "--zoom", "8"]).is_err());
        assert!(arguments(&["--help", "--zoom", "4"]).is_err());
    }
}
