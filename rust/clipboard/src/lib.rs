//! Wayland clipboard ownership for the long-running tray process.
//!
//! Each successful copy is served by a thread in this process, independent of
//! the picker. The compositor normally ends the thread when the selection is
//! replaced. A clipboard manager may retain the data after the tray exits, but
//! that is not guaranteed. This crate never clears the clipboard on cancel.
//!
//! The compositor must expose ext-data-control or wlr-data-control to the tray.
//! Preparation and thread creation failures are returned to the caller; later
//! serving failures and normal ownership loss are reported by `drain_events`.
//! Preparation queues the selection request; compositor processing is asynchronous.
//! Keep the `Clipboard` for the tray's lifetime to receive events. Dropping it
//! does not stop a thread currently serving a selection; process exit does.

use std::{
    error::Error,
    fmt,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use wl_clipboard_rs::copy::{MimeType, Options, Source};

/// An error starting clipboard ownership or serving a paste request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardError(String);

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ClipboardError {}

/// A serving thread has stopped. `id` identifies the successful copy that
/// started it; older selections can end after a newer copy has been requested.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardEvent {
    /// All selection sources were destroyed, normally because another client
    /// replaced the clipboard contents.
    OwnershipEnded { id: u64 },
    /// Clipboard serving failed after the copy was prepared.
    ServingFailed { id: u64, error: ClipboardError },
}

/// Clipboard copy service intended to be held by the resident tray.
pub struct Clipboard {
    events_tx: Sender<ClipboardEvent>,
    events_rx: Receiver<ClipboardEvent>,
    next_id: u64,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    /// Create a service without contacting the compositor or changing the clipboard.
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            events_tx,
            events_rx,
            next_id: 0,
        }
    }

    /// Offer exact UTF-8 text on the regular clipboard for unlimited pastes.
    ///
    /// This blocks while connecting to Wayland and preparing the offer. Call it
    /// from a blocking worker rather than an async runtime's event-loop thread.
    /// An error does not deliberately clear the existing clipboard. Returns only
    /// after setup succeeds; later ownership loss or serving errors are events.
    pub fn copy_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let text = text.as_bytes().to_vec().into_boxed_slice();
        self.start(move || {
            let mut options = Options::new();
            options.foreground(true);
            let prepared = options
                .prepare_copy(Source::Bytes(text), MimeType::Text)
                .map_err(|error| ClipboardError(format!("preparing Wayland clipboard: {error}")))?;
            Ok(move || {
                prepared
                    .serve()
                    .map_err(|error| ClipboardError(format!("serving Wayland clipboard: {error}")))
            })
        })
    }

    /// Drain completed ownership and serving-error notifications without blocking.
    pub fn drain_events(&mut self) -> Vec<ClipboardEvent> {
        self.events_rx.try_iter().collect()
    }

    fn start<P, S>(&mut self, prepare: P) -> Result<(), ClipboardError>
    where
        P: FnOnce() -> Result<S, ClipboardError> + Send + 'static,
        S: FnOnce() -> Result<(), ClipboardError>,
    {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| ClipboardError("clipboard operation IDs exhausted".into()))?;
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let events = self.events_tx.clone();
        thread::Builder::new()
            .name(format!("clipboard-{id}"))
            .spawn(move || {
                let prepared = match prepare() {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                // If the caller disappeared, do not claim the selection.
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(prepared));
                let event = match result {
                    Ok(Ok(())) => ClipboardEvent::OwnershipEnded { id },
                    Ok(Err(error)) => ClipboardEvent::ServingFailed { id, error },
                    Err(_) => ClipboardEvent::ServingFailed {
                        id,
                        error: ClipboardError("clipboard serving thread panicked".into()),
                    },
                };
                let _ = events.send(event);
            })
            .map_err(|error| ClipboardError(format!("starting clipboard thread: {error}")))?;
        ready_rx.recv().map_err(|_| {
            ClipboardError("clipboard preparation thread stopped unexpectedly".into())
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn preparation_failure_is_returned_without_an_event() {
        let mut clipboard = Clipboard::new();
        let result = clipboard.start(|| -> Result<fn() -> Result<(), ClipboardError>, _> {
            Err(ClipboardError("no protocol".into()))
        });
        assert_eq!(result.unwrap_err().to_string(), "no protocol");
        assert!(clipboard.drain_events().is_empty());
    }

    #[test]
    fn ownership_and_errors_are_reported_with_distinct_ids() {
        let mut clipboard = Clipboard::new();
        clipboard.start(|| Ok(|| Ok(()))).unwrap();
        clipboard
            .start(|| Ok(|| Err(ClipboardError("disconnected".into()))))
            .unwrap();
        let first = clipboard
            .events_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let second = clipboard
            .events_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let events = [first, second];
        assert!(events.contains(&ClipboardEvent::OwnershipEnded { id: 0 }));
        assert!(events.contains(&ClipboardEvent::ServingFailed {
            id: 1,
            error: ClipboardError("disconnected".into()),
        }));
        assert!(clipboard.drain_events().is_empty());
    }

    #[test]
    fn serving_panic_is_reported() {
        let mut clipboard = Clipboard::new();
        clipboard
            .start(|| Ok(|| -> Result<(), ClipboardError> { panic!("test panic") }))
            .unwrap();
        assert!(matches!(
            clipboard
                .events_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap(),
            ClipboardEvent::ServingFailed { id: 0, .. }
        ));
    }
}
