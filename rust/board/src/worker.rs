use std::{
    path::PathBuf,
    process::Stdio,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::Duration,
};

use futures_util::StreamExt;
use swatch_board_config_core::Config;
use swatch_board_ipc::{SERVICE, SwatchBoardProxy};
use swatch_board_storage::{Capture, HistoryRepository};
use tokio::sync::mpsc as async_mpsc;
use zbus::Connection;

const BOARD_SERVICE: &str = "org.apartment38.SwatchBoard.Board1";
const BOARD_PATH: &str = "/org/apartment38/SwatchBoard/Board1";

pub enum Command {
    Pick,
    Configure,
}

pub enum Event {
    Snapshot {
        config: Config,
        history: Vec<Capture>,
        connected: bool,
    },
    PickStarted,
    PickFinished(Result<(), String>),
    Error(String),
    Activate,
    ActivatePalette,
}

#[derive(Clone)]
struct Events {
    sender: Sender<Event>,
}

impl Events {
    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

struct BoardControl(Events);

#[zbus::interface(name = "org.apartment38.SwatchBoard.Board1")]
impl BoardControl {
    fn activate(&self) {
        self.0.send(Event::Activate);
    }

    fn activate_palette(&self) {
        self.0.send(Event::ActivatePalette);
    }
}

// The reply and completion signal can arrive in either order.
enum PickState {
    Idle,
    Pending {
        generation: u64,
        completion: Option<Result<(), String>>,
    },
    Active,
}

pub fn start(show_palette: bool) -> Result<(Sender<Command>, Receiver<Event>), String> {
    let (commands, command_rx) = mpsc::channel();
    let (event_tx, events) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("swatch-board-worker".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("Cannot start Board worker: {error}")));
                    return;
                }
            };
            runtime.block_on(run(
                command_rx,
                Events { sender: event_tx },
                ready_tx,
                show_palette,
            ));
        })
        .map_err(|error| format!("Cannot start Board worker: {error}"))?;
    ready_rx
        .recv()
        .map_err(|_| "Board worker stopped during startup".to_string())??;
    Ok((commands, events))
}

fn snapshot(events: &Events, config: &Config, history: &[Capture], connected: bool) {
    events.send(Event::Snapshot {
        config: config.clone(),
        history: history.to_vec(),
        connected,
    });
}

fn refresh_history(events: &Events, history: &mut Vec<Capture>) {
    match HistoryRepository::open_default().and_then(|repository| repository.list()) {
        Ok(captures) => *history = captures,
        Err(error) => events.send(Event::Error(format!(
            "Cannot load capture history: {error}"
        ))),
    }
}

fn config_program() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("swatch-board-config"))
        })
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("swatch-board-config"))
}

fn finish_pick(state: &mut PickState, result: Result<(), String>, events: &Events) {
    match state {
        PickState::Pending { completion, .. } => *completion = Some(result),
        PickState::Active => {
            *state = PickState::Idle;
            events.send(Event::PickFinished(result));
        }
        PickState::Idle => {}
    }
}

async fn run(
    commands: Receiver<Command>,
    events: Events,
    ready: mpsc::SyncSender<Result<(), String>>,
    show_palette: bool,
) {
    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot connect to the session bus: {error}")));
            return;
        }
    };
    if let Err(error) = connection
        .object_server()
        .at(BOARD_PATH, BoardControl(events.clone()))
        .await
    {
        let _ = ready.send(Err(format!("Cannot register Board activation: {error}")));
        return;
    }
    match connection
        .request_name_with_flags(
            BOARD_SERVICE,
            zbus::fdo::RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        Ok(_) => {}
        Err(zbus::Error::NameTaken) => {
            let result = async {
                let proxy =
                    zbus::Proxy::new(&connection, BOARD_SERVICE, BOARD_PATH, BOARD_SERVICE).await?;
                proxy
                    .call::<_, _, ()>(
                        if show_palette {
                            "ActivatePalette"
                        } else {
                            "Activate"
                        },
                        &(),
                    )
                    .await
            }
            .await;
            let _ = ready.send(match result {
                Ok(()) => Err("Board is already open".into()),
                Err(error) => Err(format!("Cannot activate the existing Board: {error}")),
            });
            return;
        }
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot own Board service: {error}")));
            return;
        }
    }
    let proxy = match SwatchBoardProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot initialize tray connection: {error}")));
            return;
        }
    };
    // A bus match can be installed even when the tray is not running.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)
        .expect("valid tray service name")
        .path(swatch_board_ipc::PATH)
        .expect("valid tray path")
        .interface(swatch_board_ipc::INTERFACE)
        .expect("valid tray interface")
        .build();
    let mut signals = match zbus::MessageStream::for_match_rule(rule, &connection, None).await {
        Ok(signals) => signals,
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot subscribe to tray events: {error}")));
            return;
        }
    };
    let mut owners = match proxy.inner().receive_owner_changed().await {
        Ok(owners) => owners,
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot monitor tray connection: {error}")));
            return;
        }
    };
    let dbus = match zbus::fdo::DBusProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            let _ = ready.send(Err(format!("Cannot monitor session bus: {error}")));
            return;
        }
    };
    let mut owner = dbus
        .get_name_owner(SERVICE.try_into().expect("valid tray service name"))
        .await
        .ok()
        .map(|name| name.to_string());
    if ready.send(Ok(())).is_err() {
        return;
    }
    let mut config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            events.send(Event::Error(format!("Cannot load settings: {error}")));
            Config::default()
        }
    };
    let mut history = Vec::new();
    refresh_history(&events, &mut history);
    snapshot(&events, &config, &history, owner.is_some());

    let (reply_tx, mut replies) = async_mpsc::unbounded_channel();
    let mut state = PickState::Idle;
    let mut generation = 0u64;
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    let mut settings_tick = 0u8;
    let mut last_settings_error: Option<String> = None;
    let mut children: Vec<tokio::process::Child> = Vec::new();
    loop {
        tokio::select! {
            _ = tick.tick() => {
                loop {
                    match commands.try_recv() {
                        Ok(Command::Configure) => {
                            match tokio::process::Command::new(config_program()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).spawn() {
                                Ok(child) => children.push(child),
                                Err(error) => events.send(Event::Error(format!("Cannot launch settings: {error}"))),
                            }
                        }
                        Ok(Command::Pick) => {
                            if !matches!(state, PickState::Idle) {
                                events.send(Event::PickFinished(Err("A color pick is already in progress".into())));
                            } else if owner.is_none() {
                                events.send(Event::PickFinished(Err("The tray is not connected; start the tray to pick a color".into())));
                            } else {
                                generation = generation.wrapping_add(1);
                                let id = generation;
                                state = PickState::Pending { generation: id, completion: None };
                                let connection = connection.clone();
                                let replies = reply_tx.clone();
                                tokio::spawn(async move {
                                    let result = match SwatchBoardProxy::new(&connection).await {
                                        Ok(proxy) => proxy.pick_color().await.map_err(|error| format!("Cannot start color picker: {error}")),
                                        Err(error) => Err(format!("Cannot connect to tray: {error}")),
                                    };
                                    let _ = replies.send((id, result));
                                });
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                children.retain_mut(|child| match child.try_wait() {
                    Ok(Some(status)) => { if !status.success() { events.send(Event::Error(format!("Settings exited abnormally: {status}"))); } false }
                    Ok(None) => true,
                    Err(error) => { events.send(Event::Error(format!("Cannot wait for settings: {error}"))); false }
                });
                settings_tick += 1;
                if settings_tick >= 8 {
                    settings_tick = 0;
                    match Config::load() {
                        Ok(updated) => {
                            last_settings_error = None;
                            if updated != config { config = updated; snapshot(&events, &config, &history, owner.is_some()); }
                        }
                        Err(error) => {
                            let message = format!("Cannot load settings: {error}");
                            if last_settings_error.as_ref() != Some(&message) { events.send(Event::Error(message.clone())); last_settings_error = Some(message); }
                        }
                    }
                }
            }
            Some((id, result)) = replies.recv() => {
                if matches!(&state, PickState::Pending { generation, .. } if *generation == id) {
                    let previous = std::mem::replace(&mut state, PickState::Idle);
                    if let PickState::Pending { completion, .. } = previous {
                        match result {
                            Ok(reply) if reply == "started" => {
                                events.send(Event::PickStarted);
                                if let Some(result) = completion { events.send(Event::PickFinished(result)); } else { state = PickState::Active; }
                            }
                            Ok(reply) => events.send(Event::PickFinished(Err(format!("Tray did not start the picker: {reply}")))),
                            Err(error) => events.send(Event::PickFinished(Err(error))),
                        }
                    }
                }
            }
            signal = signals.next() => {
                let signal = match signal {
                    Some(Ok(signal)) => signal,
                    _ => {
                        if !matches!(state, PickState::Idle) { events.send(Event::PickFinished(Err("Lost the tray connection while picking".into()))); }
                        events.send(Event::Error("Lost the session bus connection".into()));
                        return;
                    }
                };
                match signal.header().member().map(|member| member.as_str()) {
                    Some("HistoryChanged") => { refresh_history(&events, &mut history); snapshot(&events, &config, &history, owner.is_some()); }
                    Some("ColorPicked") => finish_pick(&mut state, Ok(()), &events),
                    Some("ColorPickCancelled") => finish_pick(&mut state, Ok(()), &events),
                    Some("ColorPickFailed") => {
                        let reason = signal.body().deserialize::<(String,)>().map(|(reason,)| reason).unwrap_or_else(|_| "Color picker failed".into());
                        finish_pick(&mut state, Err(reason), &events);
                    }
                    _ => {}
                }
            }
            changed = owners.next() => {
                let Some(new_owner) = changed else {
                    if !matches!(state, PickState::Idle) { events.send(Event::PickFinished(Err("Lost the tray connection while picking".into()))); }
                    events.send(Event::Error("Lost the session bus connection".into()));
                    return;
                };
                let new_owner = new_owner.map(|name| name.to_string());
                if new_owner != owner {
                    if owner.is_some() && !matches!(state, PickState::Idle) {
                        state = PickState::Idle;
                        events.send(Event::PickFinished(Err("The tray disconnected while picking".into())));
                    }
                    owner = new_owner;
                    if owner.is_some() { refresh_history(&events, &mut history); }
                    snapshot(&events, &config, &history, owner.is_some());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command as ProcessCommand;
    use std::time::Instant;

    // A separate bus makes the name-ownership and activation test independent of
    // any Board or tray running in the user's session.
    #[test]
    fn second_board_activates_palette_over_private_bus() {
        const CHILD: &str = "SWATCH_BOARD_ACTIVATION_TEST";
        if std::env::var_os(CHILD).is_none() {
            let root = std::env::temp_dir()
                .join(format!("swatch-board-activation-{}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let runtime = root.join("runtime");
            std::fs::create_dir_all(&runtime).unwrap();
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
            let result = ProcessCommand::new("dbus-run-session")
                .arg("--")
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "worker::tests::second_board_activates_palette_over_private_bus",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("XDG_DATA_HOME", root.join("data"))
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("XDG_STATE_HOME", root.join("state"))
                .env("XDG_RUNTIME_DIR", runtime)
                .env("QT_QPA_PLATFORM", "offscreen")
                .output()
                .expect("start a private D-Bus session");
            let _ = std::fs::remove_dir_all(root);
            assert!(
                result.status.success(),
                "private activation test failed:\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }

        let (commands, events) = start(false).expect("start the first Board worker");
        // Normal binary is built before tests so the theme worker is also present.
        let board = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("swatch-board");
        assert!(
            board.is_file(),
            "build the workspace before running this test"
        );
        for (args, palette) in [(&["--palette"][..], true), (&[][..], false)] {
            let mut child = ProcessCommand::new(&board)
                .args(args)
                .spawn()
                .expect("launch the second Board");
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "second Board exited with {status}");
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("second Board did not exit after activating the existing one");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let event = events.recv_timeout(remaining).expect("activation event");
                match event {
                    Event::ActivatePalette if palette => break,
                    Event::Activate if !palette => break,
                    Event::ActivatePalette | Event::Activate => panic!("wrong activation event"),
                    _ => {}
                }
            }
        }
        drop(commands);
    }
}
