use std::process::Command;

// These commands must work without a desktop or a portal permission prompt.
fn cli(argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_swatch-board-picker"))
        .arg(argument)
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_RUNTIME_DIR", "/nonexistent-swatch-board-test-runtime")
        .output()
        .expect("run the picker CLI")
}

#[test]
fn help_identifies_swatch_board_without_starting_capture() {
    let output = cli("--help");
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Swatch Board"));
    assert!(text.contains("swatch-board-picker"));
    assert!(text.contains("--version"));
    assert!(output.stderr.is_empty());
}

#[test]
fn version_identifies_swatch_board_without_starting_capture() {
    let output = cli("--version");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("Swatch Board Picker {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn unknown_arguments_fail_without_starting_capture() {
    let output = cli("--not-an-option");
    assert!(!output.status.success());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("unknown argument"), "{text}");
    assert!(text.contains("--help"));
}
