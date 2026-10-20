mod capture;
mod overlay;
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
    let (capture_stop_tx, capture_stop_rx) = mpsc::channel();
    let capture_thread = std::thread::Builder::new()
        .name("colorboard-pipewire".into())
        .spawn(move || capture::run(pipewire_fd, streams, frame_tx, capture_stop_rx))?;

    // Do not begin KWin's selection grab until at least one live frame arrives.
    let first_frame = frame_rx.recv()?;
    eprintln!(
        "Live capture ready: stream={} frame={}x{} pointer={:?}",
        first_frame.stream_id,
        first_frame.frame.width,
        first_frame.frame.height,
        first_frame.pointer
    );

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (overlay_stop_tx, overlay_stop_rx) = mpsc::channel();
    let overlay = tokio::task::spawn_blocking(move || {
        overlay::run(first_frame, frame_rx, ready_tx, overlay_stop_rx)
    });

    // Wait for the configured, rendered layer surface before asking KWin to
    // take the pointer. KWin remains authoritative for selection/cancellation.
    if ready_rx.await.is_err() {
        let _ = overlay_stop_tx.send(());
        let _ = capture_stop_tx.send(());
        let _ = overlay.await;
        let _ = capture_thread.join();
        return Err("magnifier overlay exited before becoming visible".into());
    }
    let selection = selection::pick_color().await;
    let _ = overlay_stop_tx.send(());
    let _ = capture_stop_tx.send(());
    let overlay_result = overlay.await?;
    capture_thread
        .join()
        .map_err(|_| "PipeWire capture thread panicked")?;
    overlay_result?;

    match selection {
        Ok(qrgb) => {
            let color = selection::SelectedColor::from(qrgb);
            eprintln!(
                "KWin selected #{:02X}{:02X}{:02X}",
                color.r, color.g, color.b
            );
        }
        Err(error) => eprintln!("KWin color selection ended: {error}"),
    }
    Ok(())
}
