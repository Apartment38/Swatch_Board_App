use serde_json::{Value, json};
use std::error::Error;
use swatch_board_ipc::SwatchBoardProxyBlocking;

type Entry = (String, String, String, String);
type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

struct Options {
    limit: u32,
    columns: u32,
    rows: u32,
}

fn options() -> Result<Options> {
    let mut options = Options {
        limit: 24,
        columns: 4,
        rows: 3,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value: u32 = args.next().ok_or("missing argument value")?.parse()?;
        match arg.as_str() {
            "--display-limit" if (1..=10000).contains(&value) => options.limit = value,
            "--columns" if (1..=12).contains(&value) => options.columns = value,
            "--rows" if (1..=12).contains(&value) => options.rows = value,
            _ => return Err(format!("invalid argument: {arg}").into()),
        }
    }
    Ok(options)
}

fn run() -> Result<i32> {
    let options = options()?;
    let bus = zbus::blocking::Connection::session()?;
    let proxy = SwatchBoardProxyBlocking::new(&bus)?;
    let entries: Vec<Entry> = proxy.get_history(options.limit)?;
    let snapshot = json!({
        "entries": entries.iter().map(|entry| json!({
            "id": entry.0, "color": entry.1, "text": entry.1,
            "detail": format!("{}\n{}", entry.2, entry.3),
        })).collect::<Vec<_>>(),
        "columns": options.columns, "rows": options.rows, "error": null,
    });
    Ok(swatch_board_qt::run("history", move |request| -> Value {
        let failure = |message: String| {
            let mut state = snapshot.clone();
            state["error"] = Value::String(message);
            state
        };
        match request["op"].as_str().unwrap_or("") {
            "init" => snapshot.clone(),
            "poll" => Value::Null,
            "select" => {
                let Some(id) = request["id"]
                    .as_str()
                    .filter(|id| entries.iter().any(|entry| entry.0 == *id))
                else {
                    return failure("The selected capture no longer exists.".into());
                };
                match proxy.copy_history(id) {
                    Ok(_) => json!({"close": true}),
                    Err(error) => failure(format!("Copy failed: {error}")),
                }
            }
            _ => failure("Unknown history operation.".into()),
        }
    }))
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("swatch-board-history: {error}");
            std::process::exit(1);
        }
    }
}
