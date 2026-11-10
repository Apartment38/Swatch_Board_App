use serde::Deserialize;
use std::{env, path::PathBuf};
use swatch_board_color::Format;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub history_limit: usize,
    pub history_display_limit: u32,
    pub history_columns: u32,
    pub history_rows: u32,
    pub auto_copy: bool,
    pub clipboard_format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_limit: 100,
            history_display_limit: 24,
            history_columns: 4,
            history_rows: 3,
            auto_copy: true,
            clipboard_format: "hex".into(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let base = match env::var_os("XDG_CONFIG_HOME") {
            Some(value) if PathBuf::from(&value).is_absolute() => PathBuf::from(value),
            _ => PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?).join(".config"),
        };
        let path = base.join("swatch-board/config.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("cannot read {}: {error}", path.display()).into()),
        };
        Self::parse(&text).map_err(|error| format!("invalid {}: {error}", path.display()).into())
    }

    fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if config.history_limit == 0 {
            return Err("history_limit must be greater than zero".into());
        }
        if !(1..=10_000).contains(&config.history_display_limit) {
            return Err("history_display_limit must be between 1 and 10000".into());
        }
        if !(1..=12).contains(&config.history_columns) {
            return Err("history_columns must be between 1 and 12".into());
        }
        if !(1..=12).contains(&config.history_rows) {
            return Err("history_rows must be between 1 and 12".into());
        }
        config
            .clipboard_format
            .parse::<Format>()
            .map_err(|error| format!("invalid clipboard_format: {error}"))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_history_and_clipboard_settings() {
        let defaults = Config::parse("").unwrap();
        assert_eq!(defaults.history_limit, 100);
        assert!(defaults.auto_copy);
        assert_eq!(defaults.clipboard_format, "hex");
        assert!(Config::parse("history_limit = 0").is_err());
        assert!(Config::parse("clipboard_format = 'bogus'").is_err());
        assert!(Config::parse("auto_copy = [").is_err());
    }

    #[test]
    fn validates_popup_geometry_and_display_limits() {
        let config = Config::parse("").unwrap();
        assert_eq!(
            (
                config.history_display_limit,
                config.history_columns,
                config.history_rows
            ),
            (24, 4, 3)
        );
        assert!(
            Config::parse("history_display_limit = 10000\nhistory_columns = 12\nhistory_rows = 12")
                .is_ok()
        );
        for input in [
            "history_display_limit = 0",
            "history_display_limit = 10001",
            "history_columns = 0",
            "history_columns = 13",
            "history_rows = 0",
            "history_rows = 13",
        ] {
            assert!(Config::parse(input).is_err(), "{input}");
        }
    }
}
