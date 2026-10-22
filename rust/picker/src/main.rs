mod capture;
mod native;
mod native_guard;
mod options;
mod overlay;
mod portal;
mod selection;

use std::{error::Error, sync::mpsc};

use capture::CaptureFeed;
use portal::{CaptureSession, begin_capture};

fn verify_overlay_exit(
    result: Result<(), String>,
    selection_was_pending: bool,
) -> Result<(), String> {
    result?;
    if selection_was_pending {
        return Err("magnifier exited while KWin selection was still pending".into());
    }
    Ok(())
}

fn verify_selection(
    selected: selection::SelectedColor,
    sample: Option<&overlay::LensSample>,
) -> Result<(), String> {
    let sample = sample.ok_or("no compositor-presented sample exists for the selected output")?;
    if (selected.r, selected.g, selected.b) != (sample.color.r, sample.color.g, sample.color.b) {
        return Err(format!(
            "Phase 1A accuracy gate failed: displayed #{:02X}{:02X}{:02X}, KWin selected #{:02X}{:02X}{:02X}; asynchronous content changed or sampling disagreed",
            sample.color.r, sample.color.g, sample.color.b, selected.r, selected.g, selected.b
        ));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let options = match options::parse(std::env::args_os().skip(1))? {
        options::Command::Help => {
            println!(
                "Swatch Board Picker — live KDE Wayland magnifier\n\nUsage: swatch-board-picker [OPTIONS]\n\nWith no arguments, request screen-capture permission and pick a color.\n  -h, --help       Show help without starting capture\n  -V, --version    Show version without starting capture\n      --radius N   Source radius, 0..31 (default 7)\n      --zoom N     Physical-pixel magnification, 4..32 (default 10)\n      --diagnostics  Log source coordinates, colors, and receipt/cursor age\n      --native      Experimental Rust-owned presented-lens authority (authorized normal-transform monitors; --repeat 1 only)\n      --repeat N    Acceptance mode: 1..20 picks, one consent session (default 1)"
            );
            return Ok(());
        }
        options::Command::Version => {
            println!("Swatch Board Picker {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        options::Command::Pick(options) => options,
    };
    if options.native {
        eprintln!(
            "Experimental native backend: not a Phase 1A pass; authorized normal-transform capture outputs"
        );
        match native::pick(options).await? {
            native::Outcome::Selected(sample) => {
                println!(
                    "#{:02X}{:02X}{:02X}",
                    sample.color[0], sample.color[1], sample.color[2]
                );
                eprintln!(
                    "Native selected compositor-presented sample stream={} source={:?} video_pts={}",
                    sample.stream_id, sample.source, sample.video_pts
                );
            }
            native::Outcome::Cancelled => eprintln!("Native color selection cancelled"),
        }
        return Ok(());
    }
    eprintln!("Swatch Board picker Phase 1A prototype");
    let CaptureSession {
        portal,
        session,
        streams,
        pipewire_fd,
    } = begin_capture().await?;
    // Keep both objects alive until the helpers have stopped; close the portal
    // explicitly rather than assuming process exit is resource management.
    if options.repeats > 1 {
        eprintln!("Acceptance preparation window: 3 seconds before first pick");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    let mut first_fd = Some(pipewire_fd);
    let mut result = Ok(());
    for round in 1..=options.repeats {
        eprintln!("Picker round {round} of {}", options.repeats);
        let fd = if let Some(fd) = first_fd.take() {
            fd
        } else {
            match portal
                .open_pipe_wire_remote(&session, Default::default())
                .await
            {
                Ok(fd) => fd,
                Err(error) => {
                    result = Err(error.into());
                    break;
                }
            }
        };
        result = run_picker(streams.clone(), fd, options).await;
        if result.is_err() {
            break;
        }
    }
    let close_result = session.close().await;
    drop(portal);
    if let Err(error) = close_result {
        eprintln!("ScreenCast session close failed: {error}");
        if result.is_ok() {
            return Err(error.into());
        }
    } else {
        eprintln!("ScreenCast session closed");
    }
    result
}

async fn run_picker(
    streams: Vec<ashpd::desktop::screencast::Stream>,
    pipewire_fd: std::os::fd::OwnedFd,
    options: options::PickerOptions,
) -> Result<(), Box<dyn Error>> {
    if streams.is_empty() {
        return Err("the portal authorized no monitor streams".into());
    }
    let feed = CaptureFeed::new();
    let capture_feed = feed.clone();
    let (capture_stop_tx, capture_stop_rx) = mpsc::channel();
    let capture_thread = std::thread::Builder::new()
        .name("swatch-board-pipewire".into())
        .spawn(move || capture::run(pipewire_fd, streams, capture_feed, capture_stop_rx))?;
    let first_frame = match feed.wait_for_frame(std::time::Duration::from_secs(10)) {
        Ok(frame) => frame,
        Err(error) => {
            let _ = capture_stop_tx.send(());
            let _ = capture_thread.join();
            return Err(error.into());
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
    let overlay_feed = feed.clone();
    let mut overlay = tokio::task::spawn_blocking(move || {
        overlay::run(
            first_frame,
            overlay_feed,
            sample_tx,
            ready_tx,
            overlay_stop_rx,
            options,
        )
    });
    let startup = match tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err("magnifier exited before becoming visible"),
        Err(_) => Err("magnifier did not become ready: no usable cursor metadata"),
    };
    if let Err(error) = startup {
        let _ = overlay_stop_tx.send(());
        let _ = capture_stop_tx.send(());
        let overlay_result = overlay.await;
        let capture_result = capture_thread.join();
        eprintln!("Startup cleanup: overlay={overlay_result:?}, capture={capture_result:?}");
        return Err(feed.failure().unwrap_or_else(|| error.into()).into());
    }
    eprintln!("Magnifier ready; KWin owns selection (left click/Enter, right click/Escape)");
    // KWin exposes pick(), but no cancellation method. Do not drop the pending
    // call on a renderer failure and pretend that this releases its input grab.
    let pending_selection = selection::pick_color();
    tokio::pin!(pending_selection);
    let mut early_overlay = None;
    let selection = tokio::select! {
        // Prefer an already-finished renderer over a simultaneously ready pick:
        // it must not turn an early renderer exit into apparent success.
        biased;
        result = &mut overlay => {
            eprintln!("Magnifier stopped during selection: {result:?}. Press Escape or right click to release KWin selection.");
            early_overlay = Some(result);
            let _ = capture_stop_tx.send(());
            pending_selection.await
        }
        result = &mut pending_selection => result,
    };
    let last_sample = sample_rx.borrow().clone();
    let _ = overlay_stop_tx.send(());
    let _ = capture_stop_tx.send(());
    // Always join both helpers, including when one failed or panicked.
    let selection_was_pending = early_overlay.is_some();
    let overlay_result = match early_overlay {
        Some(result) => result,
        None => overlay.await,
    };
    let capture_result = capture_thread.join();
    eprintln!("Capture and overlay helpers stopped");
    let overlay_result = overlay_result.map_err(|error| format!("overlay task failed: {error}"))?;
    let capture_result = capture_result.map_err(|_| "PipeWire capture thread panicked")?;
    verify_overlay_exit(overlay_result, selection_was_pending)?;
    capture_result?;

    match selection {
        Ok(qrgb) => {
            let color = selection::SelectedColor::from(qrgb);
            eprintln!(
                "KWin selected #{:02X}{:02X}{:02X}",
                color.r, color.g, color.b
            );
            if let Some(sample) = &last_sample {
                let age_ms = sample.received_at.elapsed().unwrap_or_default().as_millis();
                eprintln!(
                    "Last displayed center stream={} at ({}, {}) was #{:02X}{:02X}{:02X}; frame_age={}ms pts={:?}; RGB match={}",
                    sample.stream_id,
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
            verify_selection(color, last_sample.as_ref())?;
            eprintln!("Verified exact RGB agreement with the compositor-presented sample");
        }
        Err(error) if selection::is_cancelled(&error) => {
            eprintln!("KWin color selection cancelled")
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_successful_overlay_exit_fails_closed_even_with_matching_sample() {
        assert!(verify_overlay_exit(Ok(()), true).is_err());
        assert!(verify_overlay_exit(Ok(()), false).is_ok());
        assert_eq!(
            verify_overlay_exit(Err("buffer allocation failed".into()), true),
            Err("buffer allocation failed".into())
        );
    }

    #[test]
    fn changing_content_mismatch_cannot_be_reported_as_success() {
        let sample = overlay::LensSample {
            stream_id: 1,
            source_point: swatch_board_picker_core::PixelPoint { x: 4, y: 5 },
            color: swatch_board_picker_core::Rgba8 {
                r: 255,
                g: 0,
                b: 255,
                a: 255,
            },
            presentation_time_ns: Some(42),
            received_at: std::time::SystemTime::now(),
        };
        assert!(
            verify_selection(selection::SelectedColor::from(0xff00ffff), Some(&sample)).is_err()
        );
        assert!(
            verify_selection(selection::SelectedColor::from(0xffff00ff), Some(&sample)).is_ok()
        );
    }

    #[test]
    fn no_displayed_sample_cannot_be_reported_as_success() {
        assert!(verify_selection(selection::SelectedColor::from(0xff0000ff), None).is_err());
    }
}
