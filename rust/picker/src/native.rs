//! Prompt-free selection provided by the matching Swatch Board KWin effect.
use zbus::{Connection, Error, Proxy};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Selected(u32),
    Cancelled,
}

pub async fn pick(zoom: u32) -> zbus::Result<Outcome> {
    use tokio::signal::unix::{SignalKind, signal};
    use zbus::fdo::{RequestNameFlags, RequestNameReply};

    let connection = Connection::session().await?;
    if connection
        .request_name_with_flags(
            "org.apartment38.SwatchBoard.Picker1",
            RequestNameFlags::DoNotQueue.into(),
        )
        .await?
        != RequestNameReply::PrimaryOwner
    {
        return Err(Error::Failure(
            "A Swatch Board pick is already in progress".into(),
        ));
    }
    let mut terminate = signal(SignalKind::terminate()).map_err(Error::from)?;
    let mut interrupt = signal(SignalKind::interrupt()).map_err(Error::from)?;
    load_effect(&connection).await?;
    let proxy = effect_proxy(&connection).await?;
    tokio::select! {
        result = pick_on(&proxy, zoom) => result,
        _ = terminate.recv() => cancel(&proxy).await,
        _ = interrupt.recv() => cancel(&proxy).await,
    }
}

async fn load_effect(connection: &Connection) -> zbus::Result<()> {
    let effects = Proxy::new(
        connection,
        "org.kde.KWin",
        "/Effects",
        "org.kde.kwin.Effects",
    )
    .await?;
    let loaded: bool = effects
        .call("isEffectLoaded", &("swatchboardpicker",))
        .await?;
    if !loaded {
        let loaded: bool = effects.call("loadEffect", &("swatchboardpicker",)).await?;
        if !loaded {
            return Err(Error::Failure(
                "KWin could not load the Swatch Board picker effect. Install the effect built against your running KWin version.".into(),
            ));
        }
    }
    Ok(())
}

async fn effect_proxy(connection: &Connection) -> zbus::Result<Proxy<'_>> {
    Proxy::new(
        connection,
        "org.kde.KWin",
        "/org/apartment38/SwatchBoard/Picker",
        "org.apartment38.SwatchBoard.PickerEffect1",
    )
    .await
}

async fn cancel(proxy: &Proxy<'_>) -> zbus::Result<Outcome> {
    proxy.call::<_, _, ()>("cancel", &()).await?;
    Ok(Outcome::Cancelled)
}

async fn pick_on(proxy: &Proxy<'_>, zoom: u32) -> zbus::Result<Outcome> {
    let result: zbus::Result<u32> = proxy.call("pick", &(zoom,)).await;
    match result {
        Ok(argb) => Ok(Outcome::Selected(argb & 0x00ff_ffff)),
        Err(Error::MethodError(name, _, _))
            if name.as_str() == "org.apartment38.SwatchBoard.Picker.Error.Cancelled" =>
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
    #[zbus(prefix = "org.apartment38.SwatchBoard.Picker.Error")]
    enum PickerError {
        Cancelled(String),
        Failed(String),
    }

    struct Picker {
        reply: Result<u32, bool>,
    }

    #[zbus::interface(name = "org.apartment38.SwatchBoard.PickerEffect1")]
    impl Picker {
        #[zbus(name = "pick")]
        async fn pick(&self, zoom: u32) -> Result<u32, PickerError> {
            if !(1..=32).contains(&zoom) {
                return Err(PickerError::Failed("invalid zoom".into()));
            }
            match self.reply {
                Ok(argb) => Ok(argb),
                Err(true) => Err(PickerError::Cancelled("Color picking cancelled".into())),
                Err(false) => Err(PickerError::Failed(
                    "Color picking is already in progress".into(),
                )),
            }
        }
    }

    async fn request(reply: Result<u32, bool>) -> zbus::Result<Outcome> {
        let (server, client) = tokio::net::UnixStream::pair().unwrap();
        let guid = zbus::Guid::generate();
        let (server, client) = tokio::join!(
            zbus::connection::Builder::unix_stream(server)
                .server(guid)
                .unwrap()
                .p2p()
                .serve_at("/org/apartment38/SwatchBoard/Picker", Picker { reply })
                .unwrap()
                .build(),
            zbus::connection::Builder::unix_stream(client).p2p().build(),
        );
        let _server = server.unwrap();
        let client = client.unwrap();
        pick_on(&effect_proxy(&client).await?, 4).await
    }

    #[tokio::test]
    async fn decodes_selected_color_without_swapping_channels_or_copying_alpha() {
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
    async fn effect_failures_are_not_treated_as_cancellation() {
        assert!(
            matches!(request(Err(false)).await, Err(Error::MethodError(name, _, _))
            if name.as_str() == "org.apartment38.SwatchBoard.Picker.Error.Failed")
        );
    }
}
