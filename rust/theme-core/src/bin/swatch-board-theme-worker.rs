fn main() {
    if swatch_board_theme_core::theme_worker_main().is_err() {
        // Never emit unbounded diagnostics or protocol fragments on failure.
        std::process::exit(1);
    }
}
