use std::os::fd::OwnedFd;

use ashpd::desktop::{
    PersistMode, Session,
    screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream},
};

/// Owns the portal proxy and session for the full lifetime of capture.
pub struct CaptureSession {
    pub portal: Screencast,
    pub session: Session<Screencast>,
    pub streams: Vec<Stream>,
    pub pipewire_fd: OwnedFd,
}

fn validate_streams(streams: &[Stream]) -> Result<(), Box<dyn std::error::Error>> {
    if streams.is_empty() {
        return Err("ScreenCast portal returned no streams".into());
    }
    if streams.iter().any(|stream| stream.position().is_none()) {
        return Err("portal omitted compositor-space monitor positions".into());
    }
    Ok(())
}

// This seam tests error/cleanup ordering without claiming to emulate a portal.
async fn close_on_error<T, E, C, F>(result: Result<T, E>, close: impl FnOnce() -> F) -> Result<T, E>
where
    C: std::fmt::Display,
    F: std::future::Future<Output = Result<(), C>>,
{
    if result.is_err() {
        if let Err(error) = close().await {
            eprintln!("Failed to close ScreenCast session after setup failure: {error}");
        }
    }
    result
}

pub async fn begin_capture() -> Result<CaptureSession, Box<dyn std::error::Error>> {
    eprintln!("Connecting to the ScreenCast portal");
    let portal = Screencast::new().await?;
    let available = portal.available_cursor_modes().await?;
    if !available.contains(CursorMode::Metadata) {
        return Err("ScreenCast portal does not advertise cursor metadata mode".into());
    }

    eprintln!("Creating the ScreenCast portal session");
    let session = portal.create_session(Default::default()).await?;
    // ashpd Session has no Drop implementation that calls Close. Keep every
    // fallible post-creation step in this scope so failures explicitly close it.
    let setup: Result<_, Box<dyn std::error::Error>> = async {
        eprintln!("Requesting monitor selection and capture authorization");
        portal
            .select_sources(
                &session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::Metadata)
                    .set_sources(ashpd::enumflags2::BitFlags::from(SourceType::Monitor))
                    .set_multiple(true)
                    .set_persist_mode(PersistMode::Application),
            )
            .await?
            .response()?;

        eprintln!("Starting the authorized ScreenCast streams");
        let streams = portal
            .start(&session, None, Default::default())
            .await?
            .response()?
            .streams()
            .to_vec();
        validate_streams(&streams)?;
        eprintln!("Opening the PipeWire remote");
        let pipewire_fd = portal
            .open_pipe_wire_remote(&session, Default::default())
            .await?;
        Ok((streams, pipewire_fd))
    }
    .await;
    let (streams, pipewire_fd) = close_on_error(setup, || session.close()).await?;

    Ok(CaptureSession {
        portal,
        session,
        streams,
        pipewire_fd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn empty_streams_are_rejected() {
        assert!(
            validate_streams(&[])
                .unwrap_err()
                .to_string()
                .contains("no streams")
        );
    }

    #[tokio::test]
    async fn setup_failure_closes_once_and_preserves_original_error() {
        let closes = Cell::new(0);
        let original = Box::new(std::io::Error::other("original setup error"));
        let original_address = (&*original) as *const std::io::Error;
        let result: Result<(), _> = close_on_error(Err(original), || async {
            closes.set(closes.get() + 1);
            Err::<(), _>("close failed")
        })
        .await;
        assert_eq!(closes.get(), 1);
        assert_eq!(
            (&*result.unwrap_err()) as *const std::io::Error,
            original_address
        );
    }

    #[tokio::test]
    async fn successful_setup_does_not_close_session() {
        let closes = Cell::new(0);
        let result = close_on_error(Ok::<_, &str>(42), || async {
            closes.set(closes.get() + 1);
            Ok::<(), &str>(())
        })
        .await;
        assert_eq!(result, Ok(42));
        assert_eq!(closes.get(), 0);
    }
}
