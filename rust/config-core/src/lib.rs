use serde::{Deserialize, Serialize};
use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use swatch_board_color::Format;

const VERSION: u32 = 1;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(default = "version")]
    pub version: u32,
    pub history_limit: usize,
    pub history_display_limit: u32,
    pub history_columns: u32,
    pub history_rows: u32,
    pub picker_zoom: u32,
    pub auto_copy: bool,
    pub clipboard_format: String,
    pub pick_shortcut: String,
    pub history_shortcut: String,
    pub hide_board_while_picking: bool,
    pub autostart: bool,
}

fn version() -> u32 {
    VERSION
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            history_limit: 100,
            history_display_limit: 24,
            history_columns: 4,
            history_rows: 3,
            picker_zoom: 4,
            auto_copy: true,
            clipboard_format: "hex".into(),
            pick_shortcut: "Meta+Shift+C".into(),
            history_shortcut: "Meta+Shift+H".into(),
            hide_board_while_picking: false,
            autostart: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let path = config_home()?.join("swatch-board/config.toml");
        match fs::read_to_string(&path) {
            Ok(text) => {
                Self::parse(&text).map_err(|e| format!("invalid {}: {e}", path.display()).into())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display()).into()),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(table) = value.as_table_mut() {
            // Version 1 files may contain picker settings that no longer exist.
            if table
                .get("version")
                .is_none_or(|version| version.as_integer() == Some(1))
            {
                table.remove("magnifier_radius");
                table.remove("magnifier_zoom");
            }
        }
        let config: Self = value
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!(
                "unsupported settings version {}; expected {VERSION}",
                self.version
            ));
        }
        if self.history_limit == 0 {
            return Err("history_limit must be greater than zero".into());
        }
        if !(1..=10_000).contains(&self.history_display_limit) {
            return Err("history_display_limit must be between 1 and 10000".into());
        }
        if !(1..=12).contains(&self.history_columns) {
            return Err("history_columns must be between 1 and 12".into());
        }
        if !(1..=12).contains(&self.history_rows) {
            return Err("history_rows must be between 1 and 12".into());
        }
        if !(1..=32).contains(&self.picker_zoom) {
            return Err("picker_zoom must be between 1 and 32".into());
        }
        self.clipboard_format
            .parse::<Format>()
            .map_err(|e| format!("invalid clipboard_format: {e}"))?;
        let pick =
            shortcut(&self.pick_shortcut).map_err(|e| format!("invalid pick_shortcut: {e}"))?;
        let history = shortcut(&self.history_shortcut)
            .map_err(|e| format!("invalid history_shortcut: {e}"))?;
        if pick == history {
            return Err("pick_shortcut and history_shortcut must be different".into());
        }
        Ok(())
    }

    /// Save settings atomically, then synchronize the user's autostart entry.
    /// If the second step fails, the returned error explicitly reports the partial save.
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        self.validate()
            .map_err(|e| format!("invalid settings: {e}"))?;
        let base = config_home()?;
        let text = toml::to_string_pretty(self)?;
        atomic_write(&base.join("swatch-board/config.toml"), text.as_bytes())?;
        let desktop = base.join("autostart/org.apartment38.SwatchBoard.desktop");
        let result = if self.autostart {
            desktop_entry().and_then(|entry| atomic_write(&desktop, entry.as_bytes()))
        } else {
            match fs::remove_file(&desktop) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        };
        result.map_err(|e| {
            format!("settings were saved, but autostart could not be updated: {e}").into()
        })
    }
}

fn desktop_entry() -> io::Result<String> {
    let executable = env::current_exe()?.with_file_name("swatch-board-tray");
    let path = executable.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "tray executable path is not UTF-8",
        )
    })?;
    // Desktop Exec values are string-unescaped first, then command-line parsed.
    let mut escaped = String::new();
    for ch in path.chars() {
        match ch {
            '%' => escaped.push_str("%%"),
            '\\' => escaped.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                escaped.push_str("\\\\");
                escaped.push(ch);
            }
            '\n' | '\r' => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "tray executable path contains a newline",
                ));
            }
            _ => escaped.push(ch),
        }
    }
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Swatch Board\nComment=Swatch Board tray\nExec=\"{escaped}\"\nIcon=color-picker\nTerminal=false\n"
    ))
}

fn config_home() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Ok(path);
    }
    Ok(PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?).join(".config"))
}

fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;
    fs::create_dir_all(parent)?;
    loop {
        let temp = parent.join(format!(
            ".swatch-board-{}-{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            result => result?,
        };
        let result = (|| {
            file.write_all(data)?;
            file.sync_all()?;
            fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        return result;
    }
}

// Portal preferred triggers use modifier names followed by a single keysym.
fn shortcut(value: &str) -> Result<(u8, String), String> {
    let parts: Vec<_> = value.split('+').collect();
    if parts.len() < 2 {
        return Err("use modifiers and a key, for example Meta+Shift+C".into());
    }
    let mut modifiers = 0u8;
    for part in &parts[..parts.len() - 1] {
        let bit = match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => 1,
            "alt" => 2,
            "shift" => 4,
            "meta" | "super" => 8,
            _ => return Err(format!("unknown modifier '{part}'")),
        };
        if modifiers & bit != 0 {
            return Err(format!("duplicate modifier '{part}'"));
        }
        modifiers |= bit;
    }
    let key = parts[parts.len() - 1].to_ascii_uppercase();
    let valid = (key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric())
        || key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n) && key == format!("F{n}"));
    if !valid {
        return Err("key must be a letter, digit, or F1–F24".into());
    }
    Ok((modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_flat_settings_and_round_trip() {
        let config =
            Config::parse("history_limit = 42\nhistory_columns = 3\nauto_copy = false\n").unwrap();
        assert_eq!(config.history_limit, 42);
        assert_eq!(config.history_columns, 3);
        assert!(!config.auto_copy);
        assert_eq!(config.picker_zoom, 4);
        assert_eq!(
            Config::parse(&toml::to_string_pretty(&config).unwrap()).unwrap(),
            config
        );
        assert!(Config::parse("version = 2").is_err());
    }

    #[test]
    fn validates_shortcuts() {
        for input in [
            "pick_shortcut = 'Shift+Shift+C'",
            "pick_shortcut = 'Meta+NoSuchKey'",
            "pick_shortcut = 'C'",
            "pick_shortcut = 'Super+shift+c'\nhistory_shortcut = 'Shift+Meta+C'",
        ] {
            assert!(Config::parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn picker_zoom_boundaries_and_round_trip() {
        assert_eq!(Config::parse("version = 1\n").unwrap().picker_zoom, 4);
        for zoom in [1, 4, 32] {
            let config = Config::parse(&format!("version = 1\npicker_zoom = {zoom}\n")).unwrap();
            assert_eq!(config.picker_zoom, zoom);
            let saved = toml::to_string_pretty(&config).unwrap();
            assert_eq!(Config::parse(&saved).unwrap(), config);
        }
        for zoom in [0, 33] {
            assert!(Config::parse(&format!("picker_zoom = {zoom}\n")).is_err());
        }
    }

    #[test]
    fn migrates_legacy_magnifier_settings_without_accepting_unknown_fields() {
        for prefix in ["", "version = 1\n"] {
            let text =
                format!("{prefix}history_limit = 42\nmagnifier_radius = 7\nmagnifier_zoom = 10\n");
            let config = Config::parse(&text).unwrap();
            assert_eq!(config.history_limit, 42);
            assert_eq!(config.picker_zoom, 4);
            let saved = toml::to_string_pretty(&config).unwrap();
            assert!(!saved.contains("magnifier_radius"));
            assert!(!saved.contains("magnifier_zoom"));
            assert_eq!(Config::parse(&saved).unwrap(), config);
            assert!(Config::parse(&format!("{text}unknown_setting = 1\n")).is_err());
        }
        assert!(Config::parse("version = 2\nmagnifier_radius = 7").is_err());
    }
}
