use zbus::{Connection, proxy};

#[proxy(
    interface = "org.kde.kwin.ColorPicker",
    default_service = "org.kde.KWin",
    default_path = "/ColorPicker"
)]
trait KWinColorPicker {
    fn pick(&self) -> zbus::Result<(u32,)>;
}

pub async fn pick_color() -> zbus::Result<u32> {
    let connection = Connection::session().await?;
    let proxy = KWinColorPickerProxy::new(&connection).await?;
    let (qrgb,) = proxy.pick().await?;
    Ok(qrgb)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<u32> for SelectedColor {
    fn from(qrgb: u32) -> Self {
        Self {
            a: (qrgb >> 24) as u8,
            r: (qrgb >> 16) as u8,
            g: (qrgb >> 8) as u8,
            b: qrgb as u8,
        }
    }
}
