pub use swatch_board_config_core::Config;

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
