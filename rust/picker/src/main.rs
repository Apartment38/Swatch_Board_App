mod capture;
mod portal;
mod selection;

use std::{error::Error, sync::mpsc};

use capture::CapturedFrame;
use portal::{CaptureSession, begin_capture};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    eprintln!("Colorboard picker Phase 1A prototype");
    let CaptureSession {
        portal: _portal,
        session: _session,
        streams,
        pipewire_fd,
    } = begin_capture().await?;
    if streams.is_empty() {
        return Err("the portal authorized no monitor streams".into());
    }

    let (frame_tx, frame_rx) = mpsc::sync_channel::<CapturedFrame>(2);
    std::thread::Builder::new()
        .name("colorboard-pipewire".into())
        .spawn(move || capture::run(pipewire_fd, streams, frame_tx))?;

    // Do not begin KWin's selection grab until at least one live frame arrives.
    let first_frame = tokio::task::spawn_blocking(move || frame_rx.recv()).await??;
    eprintln!(
        "Live capture ready: stream={} frame={}x{} pointer={:?}",
        first_frame.stream_id,
        first_frame.frame.width,
        first_frame.frame.height,
        first_frame.pointer
    );

    // Phase 1A continues by showing the non-interactive Wayland overlay here;
    // KWin selection is intentionally not triggered until that overlay exists.
    drop(first_frame);
    Ok(())
}
