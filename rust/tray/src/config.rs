use serde::Deserialize;
use std::{env, path::PathBuf};
use swatch_board_color::Format;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub history_limit: usize,
    pub auto_copy: bool,
    pub clipboard_format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_limit: 100,
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
}
