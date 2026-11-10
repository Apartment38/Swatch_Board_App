#[cfg(test)]
mod tests;
mod tray;

use std::{path::PathBuf, process::Stdio, time::Duration};
use swatch_board_ipc::{INTERFACE, PATH, SERVICE, SwatchBoardProxy};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{mpsc, oneshot, watch},
};
use zbus::Connection;

const PICK_TIMEOUT: Duration = Duration::from_secs(300);

enum Request {
    Pick(oneshot::Sender<Result<&'static str, String>>),
    LastColor(oneshot::Sender<String>),
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

    fn quit(&self) {
        let _ = self.requests.send(Request::Quit);
    }

    #[zbus(signal)]
    async fn color_picked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        color: &str,
    ) -> zbus::Result<()>;
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
            let _ = child.kill().await;
            Outcome::Failed("picker timed out".into())
        },
        _ = shutdown.changed() => {
            let _ = child.kill().await;
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
    let tray = tray::start(requests.clone()).await?;
    let (shutdown, shutdown_rx) = watch::channel(false);
    let mut active = None;
    let mut last_color = String::new();
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = terminate.recv() => break,
            request = rx.recv() => match request {
                Some(Request::Pick(reply)) => {
                    if active.is_some() { let _ = reply.send(Ok("busy")); continue; }
                    match Command::new(picker_path()).stdout(Stdio::piped()).kill_on_drop(true).spawn() {
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
                            let _ = reply.send(Err(reason.clone()));
                            emit(&connection, "ColorPickFailed", &(reason,)).await;
                        }
                    }
                }
                Some(Request::LastColor(reply)) => { let _ = reply.send(last_color.clone()); }
                Some(Request::Complete(outcome)) => {
                    active = None;
                    match outcome {
                        Outcome::Selected(color) => {
                            last_color = color;
                            tray.update(|item| {
                                item.color = Some(last_color.clone());
                                item.message = None;
                            }).await;
                            emit(&connection, "ColorPicked", &(&last_color,)).await;
                        }
                        Outcome::Cancelled => {
                            tray.update(|item| item.message = Some("Color selection cancelled".into())).await;
                            emit(&connection, "ColorPickCancelled", &()).await;
                        }
                        Outcome::Failed(reason) => {
                            eprintln!("{reason}");
                            tray.update(|item| item.message = Some(reason.clone())).await;
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
    if let Some(task) = active {
        let _ = task.await;
    }
    tray.shutdown().await;
    Ok(())
}
