use zbus::{Connection, proxy};

#[proxy(
    interface = "org.kde.kwin.ColorPicker",
    default_service = "org.kde.KWin",
    default_path = "/ColorPicker"
)]
trait KWinColorPicker {
    #[zbus(name = "pick")]
    fn pick(&self) -> zbus::Result<(u32,)>;
}

/// Only KWin's exact cancellation reply denotes a normal user cancellation.
pub fn is_cancelled(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::MethodError(name, _, _)
        if name.as_str() == "org.kde.kwin.ColorPicker.Error.Cancelled")
}

/// Wait for KWin's selection reply.
///
/// Dropping this future stops waiting locally; it does not cancel KWin's pointer
/// grab. KWin exposes only `pick`, with Escape/right-click as user cancellation.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn method_error(name: &str, detail: &str) -> zbus::Error {
        let call = zbus::Message::method_call("/ColorPicker", "pick")
            .unwrap()
            .build(&())
            .unwrap();
        let reply = zbus::Message::error(&call.header(), name)
            .unwrap()
            .build(&detail)
            .unwrap();
        zbus::Error::from(reply)
    }

    #[test]
    fn cancellation_requires_exact_method_error_name() {
        assert!(is_cancelled(&method_error(
            "org.kde.kwin.ColorPicker.Error.Cancelled",
            "arbitrary detail"
        )));
        for name in [
            "org.freedesktop.DBus.Error.Failed",
            "org.freedesktop.DBus.Error.AccessDenied",
            "org.freedesktop.DBus.Error.Disconnected",
            "org.kde.kwin.ColorPicker.Error.CancelledExtra",
            "org.kde.kwin.ColorPicker.Error.cancelled",
        ] {
            assert!(
                !is_cancelled(&method_error(
                    name,
                    "org.kde.kwin.ColorPicker.Error.Cancelled: Color picking got cancelled"
                )),
                "misclassified {name}"
            );
        }
        assert!(!is_cancelled(&zbus::Error::Failure(
            "org.kde.kwin.ColorPicker.Error.Cancelled".into()
        )));
        assert!(!is_cancelled(&zbus::Error::InputOutput(
            std::sync::Arc::new(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Cancelled"
            ))
        )));
    }

    #[test]
    fn qrgb_is_argb_not_rgba_and_preserves_alpha() {
        assert_eq!(
            SelectedColor::from(0x12345678),
            SelectedColor {
                a: 0x12,
                r: 0x34,
                g: 0x56,
                b: 0x78,
            }
        );
        assert_eq!(SelectedColor::from(0x00112233).a, 0);
        assert_eq!(SelectedColor::from(0xff112233).a, 255);
    }
}
