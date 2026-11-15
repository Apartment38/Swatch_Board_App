use std::{collections::HashMap, path::PathBuf};

type Settings = HashMap<(String, String), String>;

pub struct Palette {
    pub window: u32,
    pub window_text: u32,
    pub view: u32,
    pub view_text: u32,
    pub selection: u32,
    pub selection_text: u32,
    pub border: u32,
}

impl Palette {
    pub fn load() -> Self {
        let mut config_dirs = dirs("XDG_CONFIG_DIRS", "/etc/xdg");
        if let Some(home) = user_dir("XDG_CONFIG_HOME", ".config") {
            config_dirs.insert(0, home);
        }
        let mut settings = Settings::new();
        // KConfig cascades system defaults, then applies the user's overrides.
        for dir in config_dirs.iter().rev() {
            read_into(&dir.join("kdeglobals"), &mut settings);
        }
        let mut colors = Settings::new();
        if let Some(scheme) = settings.get(&("General".into(), "ColorScheme".into())) {
            let mut data_dirs = dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share");
            if let Some(home) = user_dir("XDG_DATA_HOME", ".local/share") {
                data_dirs.insert(0, home);
            }
            // A scheme name is a file name, never a path from configuration.
            if !scheme.contains('/') && !scheme.contains('\\') {
                for dir in data_dirs {
                    let file = dir.join("color-schemes").join(format!("{scheme}.colors"));
                    if read_into(&file, &mut colors) {
                        break;
                    }
                }
            }
        }
        colors.extend(settings);
        Self::from_settings(&colors)
    }

    fn from_settings(settings: &Settings) -> Self {
        // Breeze Light is KDE's default when no color configuration exists.
        let get = |group: &str, key: &str, fallback| {
            settings
                .get(&(format!("Colors:{group}"), key.into()))
                .and_then(|value| rgb(value))
                .unwrap_or(fallback)
        };
        let window = get("Window", "BackgroundNormal", 0xffeff0f1);
        let window_text = get("Window", "ForegroundNormal", 0xff232629);
        // A subtle outline derived from the active palette works in either mode.
        let mut border = 0xff000000;
        for shift in [0, 8, 16] {
            border |=
                ((((window >> shift) & 255) * 4 + ((window_text >> shift) & 255)) / 5) << shift;
        }
        Self {
            window,
            window_text,
            view: get("View", "BackgroundNormal", 0xffffffff),
            view_text: get("View", "ForegroundNormal", 0xff232629),
            selection: get("Selection", "BackgroundNormal", 0xff3daee9),
            selection_text: get("Selection", "ForegroundNormal", 0xffffffff),
            border,
        }
    }
}

fn dirs(name: &str, fallback: &str) -> Vec<PathBuf> {
    std::env::split_paths(
        &std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| fallback.into()),
    )
    .filter(|path| path.is_absolute())
    .collect()
}

fn user_dir(name: &str, suffix: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(suffix)))
}

fn read_into(path: &std::path::Path, settings: &mut Settings) -> bool {
    let Ok(data) = std::fs::read_to_string(path) else {
        return false;
    };
    parse_into(&data, settings);
    true
}

fn parse_into(data: &str, settings: &mut Settings) {
    let mut group = "";
    for line in data.lines().map(str::trim) {
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            group = section;
        } else if let Some((key, value)) = line.split_once('=') {
            settings.insert((group.into(), key.trim().into()), value.trim().into());
        }
    }
}

fn rgb(value: &str) -> Option<u32> {
    let parts: Vec<u8> = value
        .split(',')
        .map(|part| part.trim().parse())
        .collect::<Result<_, _>>()
        .ok()?;
    match parts.as_slice() {
        [r, g, b] => Some(0xff000000 | ((*r as u32) << 16) | ((*g as u32) << 8) | *b as u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_cascades_and_ignores_invalid_colors() {
        let mut settings = Settings::new();
        parse_into(
            "[Colors:Window]\nBackgroundNormal=35,38,41\nForegroundNormal=252,252,252\n[Colors:View]\nBackgroundNormal=27,30,32\nForegroundNormal=252,252,252\n[Colors:Selection]\nBackgroundNormal=61,174,233\nForegroundNormal=252,252,252",
            &mut settings,
        );
        parse_into(
            "[Colors:Window]\nBackgroundNormal=42,46,50\n[Colors:Selection]\nForegroundNormal=300,0,0",
            &mut settings,
        );
        let palette = Palette::from_settings(&settings);
        assert_eq!(palette.window, 0xff2a2e32);
        assert_eq!(palette.window_text, 0xfffcfcfc);
        assert_eq!(palette.view, 0xff1b1e20);
        assert_eq!(palette.view_text, 0xfffcfcfc);
        assert_eq!(palette.selection, 0xff3daee9);
        assert_eq!(palette.selection_text, 0xffffffff);
        assert_eq!(Palette::from_settings(&Settings::new()).window, 0xffeff0f1);
        assert_eq!(rgb("1,2,3,4"), None);
    }
}
