use serde_json::{Value, json};
use std::error::Error;
use swatch_board_ipc::SwatchBoardProxyBlocking;

type Entry = (String, String, String, String);
type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

const DISPLAY_LIMIT_RANGE: std::ops::RangeInclusive<u32> = 1..=10_000;
const GRID_DIMENSION_RANGE: std::ops::RangeInclusive<u32> = 1..=12;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Run(Options),
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    limit: u32,
    columns: u32,
    rows: u32,
}

fn parse_bounded_u32(
    flag: &str,
    args: &mut impl Iterator<Item = String>,
    range: std::ops::RangeInclusive<u32>,
) -> std::result::Result<u32, String> {
    let raw = args
        .next()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    let value: u32 = raw
        .parse()
        .map_err(|error| format!("invalid value for {flag}: {error}"))?;
    if !range.contains(&value) {
        return Err(format!(
            "{flag} must be between {} and {}",
            range.start(),
            range.end()
        ));
    }
    Ok(value)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> std::result::Result<Command, String> {
    let mut options = Options {
        limit: 24,
        columns: 4,
        rows: 3,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--display-limit" => {
                options.limit =
                    parse_bounded_u32("--display-limit", &mut args, DISPLAY_LIMIT_RANGE)?;
            }
            "--columns" => {
                options.columns = parse_bounded_u32("--columns", &mut args, GRID_DIMENSION_RANGE)?;
            }
            "--rows" => {
                options.rows = parse_bounded_u32("--rows", &mut args, GRID_DIMENSION_RANGE)?;
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(Command::Run(options))
}

fn run() -> Result<i32> {
    let options = match parse_args(std::env::args().skip(1))? {
        Command::Help => {
            println!(
                "Swatch Board History — quick color history popup\n\n\
                 Usage: swatch-board-history [--display-limit N] [--columns N] [--rows N]\n       \
                 swatch-board-history --help | --version\n\n\
                 Options:\n  \
                 --display-limit N  Maximum captures to display (1..10000, default 24)\n  \
                 --columns N        Grid columns (1..12, default 4)\n  \
                 --rows N           Grid rows (1..12, default 3)\n  \
                 -h, --help         Show this help message\n  \
                 -V, --version      Show version information"
            );
            return Ok(0);
        }
        Command::Version => {
            println!("Swatch Board History {}", env!("CARGO_PKG_VERSION"));
            return Ok(0);
        }
        Command::Run(options) => options,
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> std::result::Result<Command, String> {
        parse_args(args.iter().map(|arg| (*arg).to_string()))
    }

    #[test]
    fn parses_defaults_and_valid_flags() {
        assert_eq!(
            parse(&[]),
            Ok(Command::Run(Options {
                limit: 24,
                columns: 4,
                rows: 3,
            }))
        );
        assert_eq!(
            parse(&["--display-limit", "10000", "--columns", "12", "--rows", "1"]),
            Ok(Command::Run(Options {
                limit: 10_000,
                columns: 12,
                rows: 1,
            }))
        );
        assert_eq!(
            parse(&["--display-limit", "1", "--columns", "1", "--rows", "12"]),
            Ok(Command::Run(Options {
                limit: 1,
                columns: 1,
                rows: 12,
            }))
        );
    }

    #[test]
    fn supports_help_and_version_flags() {
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&["-h"]), Ok(Command::Help));
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
        assert_eq!(parse(&["-V"]), Ok(Command::Version));
    }

    #[test]
    fn rejects_unknown_flags_before_reading_values() {
        assert_eq!(parse(&["--bogus"]), Err("unknown argument: --bogus".into()));
        assert_eq!(
            parse(&["--bogus", "abc"]),
            Err("unknown argument: --bogus".into())
        );
        assert_eq!(
            parse(&["--columns", "4", "--bogus", "abc"]),
            Err("unknown argument: --bogus".into())
        );
    }

    #[test]
    fn reports_missing_and_non_numeric_values_accurately() {
        assert_eq!(
            parse(&["--display-limit"]),
            Err("missing value for --display-limit".into())
        );
        assert_eq!(
            parse(&["--columns"]),
            Err("missing value for --columns".into())
        );
        assert_eq!(parse(&["--rows"]), Err("missing value for --rows".into()));

        for (flag, value) in [
            ("--display-limit", "abc"),
            ("--columns", "-1"),
            ("--rows", "3.5"),
        ] {
            let error = parse(&[flag, value]).unwrap_err();
            assert!(
                error.starts_with(&format!("invalid value for {flag}: ")),
                "unexpected error for {flag} {value}: {error}"
            );
        }
    }

    #[test]
    fn reports_out_of_range_values_with_flag_bounds() {
        for value in ["0", "10001"] {
            assert_eq!(
                parse(&["--display-limit", value]),
                Err("--display-limit must be between 1 and 10000".into())
            );
        }
        for value in ["0", "13"] {
            assert_eq!(
                parse(&["--columns", value]),
                Err("--columns must be between 1 and 12".into())
            );
            assert_eq!(
                parse(&["--rows", value]),
                Err("--rows must be between 1 and 12".into())
            );
        }
    }
}
