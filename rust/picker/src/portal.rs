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

pub async fn begin_capture() -> Result<CaptureSession, Box<dyn std::error::Error>> {
    eprintln!("Connecting to the ScreenCast portal");
    let portal = Screencast::new().await?;
    let available = portal.available_cursor_modes().await?;
    if !available.contains(CursorMode::Metadata) {
        return Err("ScreenCast portal does not advertise cursor metadata mode".into());
    }

    eprintln!("Creating the ScreenCast portal session");
    let session = portal.create_session(Default::default()).await?;
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
    if streams.iter().any(|stream| stream.position().is_none()) {
        return Err("portal omitted compositor-space monitor positions".into());
    }
    eprintln!("Opening the PipeWire remote");
    let pipewire_fd = portal
        .open_pipe_wire_remote(&session, Default::default())
        .await?;

    Ok(CaptureSession {
        portal,
        session,
        streams,
        pipewire_fd,
    })
}
