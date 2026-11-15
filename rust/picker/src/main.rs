mod capture;
mod native;
mod native_guard;
mod options;
mod portal;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let options = match options::parse(std::env::args_os().skip(1))? {
        options::Command::Help => {
            println!(
                "Swatch Board Picker — live KDE Wayland magnifier\n\nUsage: swatch-board-picker [OPTIONS]\n\nWith no arguments, request screen-capture permission and pick a color.\n  -h, --help       Show help without starting capture\n  -V, --version    Show version without starting capture\n      --radius N   Source radius, 0..31 (default 7)\n      --zoom N     Physical-pixel magnification, 4..32 (default 10)\n      --diagnostics  Log source coordinates, colors, and receipt/cursor age"
            );
            return Ok(());
        }
        options::Command::Version => {
            println!("Swatch Board Picker {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        options::Command::Pick(options) => options,
    };

    match native::pick(options).await? {
        native::Outcome::Selected(sample) => {
            println!(
                "#{:02X}{:02X}{:02X}",
                sample.color[0], sample.color[1], sample.color[2]
            );
            eprintln!(
                "Selected compositor-presented sample stream={} source={:?} video_pts={}",
                sample.stream_id, sample.source, sample.video_pts
            );
        }
        native::Outcome::Cancelled => eprintln!("Color selection cancelled"),
    }
    Ok(())
}
