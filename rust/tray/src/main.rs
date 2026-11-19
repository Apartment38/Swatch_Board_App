mod config;
mod shortcuts;
#[cfg(test)]
mod tests;
mod tray;

use std::{path::PathBuf, process::Stdio, time::Duration};
use swatch_board_color::{Color, Format};
use swatch_board_ipc::{INTERFACE, PATH, SERVICE, SwatchBoardProxy};
use swatch_board_storage::{Capture, CaptureSource, HistoryRepository, Rgba8};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{mpsc, oneshot, watch},
};
use zbus::Connection;

const PICK_TIMEOUT: Duration = Duration::from_secs(300);
const PICK_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
type HistoryRow = (String, String, String, String);

enum Request {
    Pick(oneshot::Sender<Result<&'static str, String>>),
    LastColor(oneshot::Sender<String>),
    History(u32, oneshot::Sender<Result<Vec<HistoryRow>, String>>),
    ShowHistory(oneshot::Sender<Result<&'static str, String>>),
    OpenPalette,
    CopyHistory(String, oneshot::Sender<Result<String, String>>),
    Configure(oneshot::Sender<Result<&'static str, String>>),
    OpenBoard,
    BoardFailed(String),
    ReloadConfig(oneshot::Sender<Result<String, String>>),
    ShortcutStatus(oneshot::Sender<String>),
    RebindShortcuts(oneshot::Sender<Result<String, String>>),
    Quit,
    Complete(Outcome),
}

enum Outcome {
    Selected(String),
    Cancelled,
    Failed(String),
    Shutdown,
}

struct Control {
    requests: mpsc::UnboundedSender<Request>,
}

#[zbus::interface(name = "org.apartment38.SwatchBoard1")]
impl Control {
    fn activate(&self) {}

    async fn pick_color(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::Pick(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map(str::to_owned)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn get_last_color(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::LastColor(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))
    }

    async fn get_history(&self, limit: u32) -> zbus::fdo::Result<Vec<HistoryRow>> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::History(limit, tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn show_history(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::ShowHistory(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map(str::to_owned)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn copy_history(&self, id: &str) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::CopyHistory(id.to_owned(), tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn configure(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::Configure(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map(str::to_owned)
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn reload_config(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::ReloadConfig(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }

    async fn shortcut_status(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::ShortcutStatus(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))
    }

    async fn rebind_shortcuts(&self) -> zbus::fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Request::RebindShortcuts(tx))
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?;
        rx.await
            .map_err(|_| zbus::fdo::Error::Failed("tray is shutting down".into()))?
            .map_err(zbus::fdo::Error::Failed)
    }

    fn quit(&self) {
        let _ = self.requests.send(Request::Quit);
    }

    #[zbus(signal)]
    async fn color_picked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: &str,
        rgba: &str,
        timestamp: &str,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn history_changed(emitter: &zbus::object_server::SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn color_pick_cancelled(
        emitter: &zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn color_pick_failed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        reason: &str,
    ) -> zbus::Result<()>;
}

fn picker_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("swatch-board-picker")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("swatch-board-picker"))
}

fn history_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("swatch-board-history")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("swatch-board-history"))
}

fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("swatch-board-config")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("swatch-board-config"))
}

fn board_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("swatch-board")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("swatch-board"))
}

async fn close_history(child: &mut tokio::process::Child) -> Result<(), String> {
    match child.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => child
            .kill()
            .await
            .map_err(|error| format!("failed to close history popup: {error}")),
        Err(error) => Err(format!("failed waiting for history popup: {error}")),
    }
}

async fn stop_picker(child: &mut tokio::process::Child) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {
            if let Some(pid) = child.id() {
                // An unreaped child retains its PID, so it cannot have been reused.
                unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            }
        }
        Err(_) => {}
    }
    if !matches!(
        tokio::time::timeout(PICK_CLEANUP_TIMEOUT, child.wait()).await,
        Ok(Ok(_))
    ) {
        let _ = child.kill().await;
    }
}

async fn supervise(
    mut child: tokio::process::Child,
    mut shutdown: watch::Receiver<bool>,
    timeout: Duration,
) -> Outcome {
    let mut stdout = child.stdout.take().expect("picker stdout piped");
    let mut reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        let result = (&mut stdout).take(1025).read_to_end(&mut bytes).await;
        result.map(|_| bytes)
    });
    let result = tokio::select! {
        outcome = async {
            match child.wait().await {
                Ok(status) if status.success() => match (&mut reader).await {
                    Ok(Ok(bytes)) => parse_output(&bytes),
                    _ => Outcome::Failed("failed to read picker output".into()),
                },
                Ok(status) => Outcome::Failed(format!("picker exited abnormally: {status}")),
                Err(error) => Outcome::Failed(format!("failed waiting for picker: {error}")),
            }
        } => outcome,
        _ = tokio::time::sleep(timeout) => {
            stop_picker(&mut child).await;
            Outcome::Failed("picker timed out".into())
        },
        _ = shutdown.changed() => {
            stop_picker(&mut child).await;
            Outcome::Shutdown
        },
    };
    reader.abort();
    result
}

fn parse_output(bytes: &[u8]) -> Outcome {
    if bytes.is_empty() {
        return Outcome::Cancelled;
    }
    let Some(value) = bytes.strip_suffix(b"\n") else {
        return Outcome::Failed("invalid picker output".into());
    };
    if value.len() != 7 || value[0] != b'#' || !value[1..].iter().all(u8::is_ascii_hexdigit) {
        return Outcome::Failed("invalid picker output".into());
    }
    Outcome::Selected(String::from_utf8(value.to_ascii_uppercase()).expect("ASCII hex"))
}

async fn emit(
    connection: &Connection,
    member: &str,
    body: &(impl serde::Serialize + zbus::zvariant::DynamicType),
) {
    if let Err(error) = connection
        .emit_signal(None::<&str>, PATH, INTERFACE, member, body)
        .await
    {
        eprintln!("Failed to emit {member}: {error}");
    }
}

fn notify_failure(connection: &Connection, reason: String) {
    let connection = connection.clone();
    tokio::spawn(async move {
        let hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
            std::collections::HashMap::new();
        let result = connection
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    "Swatch Board",
                    0u32,
                    "dialog-error",
                    "Swatch Board error",
                    reason.as_str(),
                    Vec::<&str>::new(),
                    hints,
                    -1i32,
                ),
            )
            .await;
        if let Err(error) = result {
            eprintln!("Failed to show error notification: {error}");
        }
    });
}

fn notify_copy(connection: &Connection, text: String) {
    let connection = connection.clone();
    tokio::spawn(async move {
        let hints: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
            std::collections::HashMap::new();
        if let Err(error) = connection
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &(
                    "Swatch Board",
                    0u32,
                    "edit-copy",
                    "Color copied",
                    text.as_str(),
                    Vec::<&str>::new(),
                    hints,
                    -1i32,
                ),
            )
            .await
        {
            eprintln!("Failed to show copy notification: {error}");
        }
    });
}

fn history_row(capture: &Capture) -> HistoryRow {
    let c = capture.color;
    let source = match capture.source {
        CaptureSource::Picker => "picker",
        CaptureSource::Board => "board",
    };
    (
        capture.id.to_string(),
        Color::new(c.r, c.g, c.b, c.a).format(Format::HexRgba),
        capture.captured_at.to_rfc3339(),
        source.into(),
    )
}

fn capture_hex(capture: &Capture) -> String {
    let c = capture.color;
    Color::new(c.r, c.g, c.b, c.a).format(Format::HexRgb)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;
    let (requests, mut rx) = mpsc::unbounded_channel();
    connection
        .object_server()
        .at(
            PATH,
            Control {
                requests: requests.clone(),
            },
        )
        .await?;
    if let Err(error) = connection
        .request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .await
    {
        let proxy = SwatchBoardProxy::new(&connection).await?;
        proxy.activate().await.map_err(|_| error)?;
        return Ok(());
    }
    let mut config = config::Config::load()?;
    let mut format: Format = config
        .clipboard_format
        .parse()
        .map_err(|error| format!("invalid clipboard format: {error}"))?;
    let mut history = HistoryRepository::open_default()?;
    history.set_retention(config.history_limit);
    let mut last_color = history
        .last()?
        .as_ref()
        .map(capture_hex)
        .unwrap_or_default();
    let mut clipboard = swatch_board_clipboard::Clipboard::new();
    let tray = tray::start(
        requests.clone(),
        (!last_color.is_empty()).then(|| last_color.clone()),
    )
    .await?;
    let (shutdown, shutdown_rx) = watch::channel(false);
    let mut active = None;
    let (shortcut_events, mut shortcut_rx) = mpsc::unbounded_channel();
    let mut shortcuts = shortcuts::Shortcuts::start(
        config.pick_shortcut.clone(),
        config.history_shortcut.clone(),
        shortcut_events,
    );
    let mut config_child: Option<tokio::process::Child> = None;
    let mut history_child: Option<tokio::process::Child> = None;
    let mut clipboard_events = tokio::time::interval(Duration::from_secs(1));
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            action = shortcut_rx.recv() => {
                match action {
                    Some(shortcuts::Action::Pick) => { let (reply, _) = oneshot::channel(); let _ = requests.send(Request::Pick(reply)); }
                    Some(shortcuts::Action::History) => { let (reply, _) = oneshot::channel(); let _ = requests.send(Request::ShowHistory(reply)); }
                    None => {}
                }
            },
            _ = tokio::signal::ctrl_c() => break,
            _ = clipboard_events.tick() => {
                for event in clipboard.drain_events() {
                    if let swatch_board_clipboard::ClipboardEvent::ServingFailed { error, .. } = event {
                        let reason = format!("Clipboard serving failed: {error}");
                        eprintln!("{reason}");
                        tray.update(|item| item.message = Some(reason.clone())).await;
                        notify_failure(&connection, reason);
                    }
                }
            }
            _ = terminate.recv() => break,
            result = async {
                match history_child.as_mut() {
                    Some(child) => child.wait().await,
                    None => std::future::pending().await,
                }
            } => {
                history_child = None;
                let error = match result {
                    Ok(status) if !status.success() => Some(format!("History window exited abnormally: {status}")),
                    Err(error) => Some(format!("Failed waiting for history window: {error}")),
                    _ => None,
                };
                if let Some(reason) = error {
                    eprintln!("{reason}");
                    tray.update(|item| item.message = Some(reason.clone())).await;
                    notify_failure(&connection, reason);
                }
            },
            result = async {
                match config_child.as_mut() {
                    Some(child) => child.wait().await,
                    None => std::future::pending().await,
                }
            } => {
                config_child = None;
                match result {
                    Ok(status) if !status.success() => eprintln!("settings exited abnormally: {status}"),
                    Err(error) => eprintln!("failed waiting for settings: {error}"),
                    _ => {}
                }
            },
            request = rx.recv() => match request {
                Some(Request::Pick(reply)) => {
                    if active.is_some() { let _ = reply.send(Ok("busy")); continue; }
                    if let Some(child) = history_child.as_mut() {
                        if let Err(reason) = close_history(child).await {
                            let _ = reply.send(Err(reason));
                            continue;
                        }
                        history_child = None;
                    }
                    match Command::new(picker_path()).arg("--zoom").arg(config.picker_zoom.to_string()).stdout(Stdio::piped()).kill_on_drop(true).spawn() {
                        Ok(child) => {
                            let tx = requests.clone();
                            let stop = shutdown_rx.clone();
                            active = Some(tokio::spawn(async move {
                                let outcome = supervise(child, stop, PICK_TIMEOUT).await;
                                let _ = tx.send(Request::Complete(outcome));
                            }));
                            tray.update(|item| item.message = None).await;
                            let _ = reply.send(Ok("started"));
                        }
                        Err(error) => {
                            let reason = format!("failed to launch picker: {error}");
                            eprintln!("{reason}");
                            tray.update(|item| item.message = Some(reason.clone())).await;
                            notify_failure(&connection, reason.clone());
                            let _ = reply.send(Err(reason.clone()));
                            emit(&connection, "ColorPickFailed", &(reason,)).await;
                        }
                    }
                }
                Some(Request::LastColor(reply)) => { let _ = reply.send(last_color.clone()); }
                Some(Request::History(limit, reply)) => {
                    let rows = history.list().map(|captures| captures.iter().take(limit as usize).map(history_row).collect()).map_err(|error| error.to_string());
                    let _ = reply.send(rows);
                }
                Some(Request::ShowHistory(reply)) => {
                    if active.is_some() {
                        let _ = reply.send(Ok("busy"));
                        continue;
                    }
                    if let Some(child) = history_child.as_mut() {
                        match child.try_wait() {
                            Ok(Some(_)) => history_child = None,
                            Ok(None) => {
                                let result = close_history(child).await.map(|_| "closed");
                                if result.is_ok() {
                                    history_child = None;
                                }
                                let _ = reply.send(result);
                                continue;
                            }
                            Err(error) => {
                                let _ = reply.send(Err(format!("failed waiting for history popup: {error}")));
                                continue;
                            }
                        }
                    }
                    match Command::new(history_path())
                        .arg("--display-limit").arg(config.history_display_limit.to_string())
                        .arg("--columns").arg(config.history_columns.to_string())
                        .arg("--rows").arg(config.history_rows.to_string())
                        .kill_on_drop(true)
                        .spawn()
                    {
                        Ok(child) => {
                            history_child = Some(child);
                            let _ = reply.send(Ok("started"));
                        }
                        Err(error) => {
                            let reason = format!("failed to launch history popup: {error}");
                            eprintln!("{reason}");
                            tray.update(|item| item.message = Some(reason.clone())).await;
                            notify_failure(&connection, reason.clone());
                            let _ = reply.send(Err(reason));
                        }
                    }
                }
                Some(Request::CopyHistory(id, reply)) => {
                    let result = history.list()
                        .map_err(|error| error.to_string())
                        .and_then(|captures| captures.into_iter()
                            .find(|capture| capture.id.to_string() == id)
                            .ok_or_else(|| "history capture not found".to_owned()))
                        .and_then(|capture| {
                            let c = capture.color;
                            let text = Color::new(c.r, c.g, c.b, c.a).format(format);
                            clipboard.copy_text(&text)
                                .map_err(|error| format!("Clipboard copy failed: {error}"))?;
                            Ok(text)
                        });
                    if let Ok(text) = &result {
                        notify_copy(&connection, text.clone());
                    }
                    let _ = reply.send(result);
                }
                Some(Request::Configure(reply)) => {
                    if config_child.is_some() {
                        let _ = reply.send(Ok("already-open"));
                    } else {
                        match Command::new(config_path()).kill_on_drop(true).spawn() {
                            Ok(child) => { config_child = Some(child); let _ = reply.send(Ok("started")); }
                            Err(error) => { let _ = reply.send(Err(format!("failed to launch settings: {error}"))); }
                        }
                    }
                }
                Some(request @ (Request::OpenBoard | Request::OpenPalette)) => {
                    let mut command = Command::new(board_path());
                    if matches!(request, Request::OpenPalette) {
                        command.arg("--palette");
                    }
                    match command.spawn() {
                        Ok(mut child) => {
                            let requests = requests.clone();
                            tokio::spawn(async move {
                                let reason = match child.wait().await {
                                    Ok(status) if status.success() => None,
                                    Ok(status) => Some(format!("Board exited abnormally: {status}")),
                                    Err(error) => Some(format!("failed waiting for Board: {error}")),
                                };
                                if let Some(reason) = reason {
                                    let _ = requests.send(Request::BoardFailed(reason));
                                }
                            });
                            tray.update(|item| item.message = None).await;
                        }
                        Err(error) => {
                            let _ = requests.send(Request::BoardFailed(format!("failed to launch Board: {error}")));
                        }
                    }
                }
                Some(Request::BoardFailed(reason)) => {
                    eprintln!("{reason}");
                    tray.update(|item| item.message = Some(reason.clone())).await;
                    notify_failure(&connection, reason);
                }
                Some(Request::ReloadConfig(reply)) => {
                    let result = match config::Config::load() {
                        Ok(new) => match new.clipboard_format.parse::<Format>() {
                            Ok(new_format) => {
                                history.set_retention(new.history_limit);
                                format = new_format;
                                config = new;
                                shortcuts.rebind(config.pick_shortcut.clone(), config.history_shortcut.clone()).await.map(|()| "applied".to_owned())
                            }
                            Err(error) => Err(error.to_string()),
                        },
                        Err(error) => Err(error.to_string()),
                    };
                    let _ = reply.send(result);
                }
                Some(Request::ShortcutStatus(reply)) => { let _ = reply.send(shortcuts.status()); }
                Some(Request::RebindShortcuts(reply)) => {
                    let result = shortcuts.rebind(config.pick_shortcut.clone(), config.history_shortcut.clone()).await.map(|()| shortcuts.status());
                    let _ = reply.send(result);
                }
                Some(Request::Complete(outcome)) => {
                    active = None;
                    match outcome {
                        Outcome::Selected(color) => {
                            let parsed: Color = color.parse().expect("validated picker output");
                            match history.record(Rgba8 { r: parsed.r, g: parsed.g, b: parsed.b, a: parsed.a }, CaptureSource::Picker) {
                                Ok(capture) => {
                                    last_color = color;
                                    let copy_error = if config.auto_copy { clipboard.copy_text(&parsed.format(format)).err().map(|e| format!("Clipboard copy failed: {e}")) } else { None };
                                    if let Some(reason) = &copy_error { eprintln!("{reason}"); notify_failure(&connection, reason.clone()); }
                                    tray.update(|item| {
                                        item.color = Some(last_color.clone());
                                        item.message = copy_error;
                                    }).await;
                                    let (id, rgba, timestamp, _) = history_row(&capture);
                                    emit(&connection, "ColorPicked", &(id, rgba, timestamp)).await;
                                    emit(&connection, "HistoryChanged", &()).await;
                                }
                                Err(error) => {
                                    let reason = format!("Failed to save capture: {error}");
                                    eprintln!("{reason}");
                                    tray.update(|item| item.message = Some(reason.clone())).await;
                                    notify_failure(&connection, reason.clone());
                                    emit(&connection, "ColorPickFailed", &(reason,)).await;
                                }
                            }
                        }
                        Outcome::Cancelled => {
                            tray.update(|item| item.message = Some("Color selection cancelled".into())).await;
                            emit(&connection, "ColorPickCancelled", &()).await;
                        }
                        Outcome::Failed(reason) => {
                            eprintln!("{reason}");
                            tray.update(|item| item.message = Some(reason.clone())).await;
                            notify_failure(&connection, reason.clone());
                            emit(&connection, "ColorPickFailed", &(reason,)).await;
                        }
                        Outcome::Shutdown => {}
                    }
                }
                Some(Request::Quit) | None => break,
            }
        }
    }
    let _ = shutdown.send(true);
    shortcuts.shutdown().await;
    if let Some(mut child) = config_child {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    if let Some(mut child) = history_child {
        let _ = close_history(&mut child).await;
    }
    if let Some(task) = active {
        let _ = task.await;
    }
    tray.shutdown().await;
    Ok(())
}
