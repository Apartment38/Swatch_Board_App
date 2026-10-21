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
    let first_frame = match frame_rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = capture_stop_tx.send(());
            let _ = capture_thread.join();
            return Err(format!("timed out waiting for a PipeWire frame: {error}").into());
        }
    };
    eprintln!(
        "Live capture ready: stream={} frame={}x{} pointer={:?}",
        first_frame.stream_id,
        first_frame.frame.width,
        first_frame.frame.height,
        first_frame.pointer
    );

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (sample_tx, sample_rx) = tokio::sync::watch::channel(None);
    let (overlay_stop_tx, overlay_stop_rx) = mpsc::channel();
    let overlay = tokio::task::spawn_blocking(move || {
        overlay::run(first_frame, frame_rx, sample_tx, ready_tx, overlay_stop_rx)
    });

    // Wait for the configured, rendered layer surface before asking KWin to
    // take the pointer. KWin remains authoritative for selection/cancellation.
    match tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => {
            let _ = overlay_stop_tx.send(());
            let _ = capture_stop_tx.send(());
            let _ = overlay.await;
            let _ = capture_thread.join();
            return Err("magnifier overlay exited before becoming visible".into());
        }
        Err(_) => {
            eprintln!("Magnifier did not receive usable cursor metadata within 10 seconds");
            let _ = overlay_stop_tx.send(());
            let _ = capture_stop_tx.send(());
            let _ = overlay.await;
            let _ = capture_thread.join();
            return Err("magnifier did not become ready: no usable cursor metadata".into());
        }
    }
    let selection = selection::pick_color().await;
    let last_sample = sample_rx.borrow().clone();
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
            if let Some(sample) = last_sample {
                let age_ms = sample.received_at.elapsed().unwrap_or_default().as_millis();
                eprintln!(
                    "Last displayed center at ({}, {}) was #{:02X}{:02X}{:02X}; frame_age={}ms pts={:?}; RGB match={}",
                    sample.source_point.x,
                    sample.source_point.y,
                    sample.color.r,
                    sample.color.g,
                    sample.color.b,
                    age_ms,
                    sample.presentation_time_ns,
                    (sample.color.r, sample.color.g, sample.color.b) == (color.r, color.g, color.b)
                );
            }
        }
        Err(error) => eprintln!("KWin color selection ended: {error}"),
    }
    Ok(())
}
