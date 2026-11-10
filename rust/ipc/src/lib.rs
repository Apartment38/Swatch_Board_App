pub const SERVICE: &str = "org.apartment38.SwatchBoard1";
pub const PATH: &str = "/org/apartment38/SwatchBoard1";
pub const INTERFACE: &str = "org.apartment38.SwatchBoard1";

#[zbus::proxy(
    interface = "org.apartment38.SwatchBoard1",
    default_service = "org.apartment38.SwatchBoard1",
    default_path = "/org/apartment38/SwatchBoard1"
)]
pub trait SwatchBoard {
    fn activate(&self) -> zbus::Result<()>;
    fn pick_color(&self) -> zbus::Result<String>;
    fn get_last_color(&self) -> zbus::Result<String>;
    fn get_history(&self, limit: u32) -> zbus::Result<Vec<(String, String, String, String)>>;
    fn show_history(&self) -> zbus::Result<String>;
    fn copy_history(&self, id: &str) -> zbus::Result<String>;
    fn quit(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn color_picked(&self, id: &str, rgba: &str, timestamp: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn history_changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn color_pick_cancelled(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn color_pick_failed(&self, reason: &str) -> zbus::Result<()>;
}
