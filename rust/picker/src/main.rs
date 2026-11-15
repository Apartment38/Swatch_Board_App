mod native;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    match (arguments.next(), arguments.next()) {
        (None, None) => {}
        (Some(argument), None) if argument == "--help" || argument == "-h" => {
            println!(
                "Swatch Board Picker — native KDE color picker\n\nUsage: swatch-board-picker\n       swatch-board-picker --help | --version\n\nSelect with left click or Enter; cancel with Escape or right click.\nKWin owns selection; no screen-sharing portal is used."
            );
            return Ok(());
        }
        (Some(argument), None) if argument == "--version" || argument == "-V" => {
            println!("Swatch Board Picker {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => return Err("unknown argument; use swatch-board-picker --help".into()),
    }

    match native::pick().await? {
        native::Outcome::Selected(rgb) => println!("#{rgb:06X}"),
        native::Outcome::Cancelled => eprintln!("Color selection cancelled"),
    }
    Ok(())
}
