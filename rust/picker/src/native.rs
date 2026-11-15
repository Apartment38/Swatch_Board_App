//! Use the same compositor-owned picker as Plasma's color picker widget.
use zbus::{Connection, Error, Proxy};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Selected(u32),
    Cancelled,
}

pub async fn pick() -> zbus::Result<Outcome> {
    let connection = Connection::session().await?;
    pick_on(&connection).await
}

async fn pick_on(connection: &Connection) -> zbus::Result<Outcome> {
    let proxy = Proxy::new(
        connection,
        "org.kde.KWin",
        "/ColorPicker",
        "org.kde.kwin.ColorPicker",
    )
    .await?;
    // QColor is a struct containing QRgb (0xAARRGGBB), not a bare uint32.
    // The outer tuple is the D-Bus method's argument list.
    let result: zbus::Result<((u32,),)> = proxy.call("pick", &()).await;
    match result {
        Ok(((argb,),)) => Ok(Outcome::Selected(argb & 0x00ff_ffff)),
        Err(Error::MethodError(name, _, _))
            if name.as_str() == "org.kde.kwin.ColorPicker.Error.Cancelled" =>
        {
            Ok(Outcome::Cancelled)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, zbus::DBusError)]
    #[zbus(prefix = "org.kde.kwin.ColorPicker.Error")]
    enum PickerError {
        Cancelled(String),
        Failed(String),
    }

    struct Picker {
        reply: Result<u32, bool>,
    }

    #[zbus::interface(name = "org.kde.kwin.ColorPicker")]
    impl Picker {
        #[zbus(name = "pick")]
        async fn pick(&self) -> Result<((u32,),), PickerError> {
            match self.reply {
                Ok(argb) => Ok(((argb,),)),
                Err(true) => Err(PickerError::Cancelled("Color picking got cancelled".into())),
                Err(false) => Err(PickerError::Failed(
                    "Color picking is already in progress".into(),
                )),
            }
        }
    }

    async fn request(reply: Result<u32, bool>) -> zbus::Result<Outcome> {
        // A private peer connection cannot reach the user's compositor or portals.
        let (server, client) = tokio::net::UnixStream::pair().unwrap();
        let guid = zbus::Guid::generate();
        let (server, client) = tokio::join!(
            zbus::connection::Builder::unix_stream(server)
                .server(guid)
                .unwrap()
                .p2p()
                .serve_at("/ColorPicker", Picker { reply })
                .unwrap()
                .build(),
            zbus::connection::Builder::unix_stream(client).p2p().build(),
        );
        let _server = server.unwrap();
        pick_on(&client.unwrap()).await
    }

    #[tokio::test]
    async fn decodes_qcolor_struct_without_swapping_channels_or_copying_alpha() {
        assert_eq!(
            request(Ok(0xff247ac3)).await.unwrap(),
            Outcome::Selected(0x247ac3)
        );
        assert_eq!(request(Ok(0xff000007)).await.unwrap(), Outcome::Selected(7));
    }

    #[tokio::test]
    async fn cancellation_is_not_a_failed_pick_or_a_color() {
        assert_eq!(request(Err(true)).await.unwrap(), Outcome::Cancelled);
    }

    #[tokio::test]
    async fn compositor_failures_are_not_treated_as_cancellation() {
        assert!(
            matches!(request(Err(false)).await, Err(Error::MethodError(name, _, _))
            if name.as_str() == "org.kde.kwin.ColorPicker.Error.Failed")
        );
    }
}
