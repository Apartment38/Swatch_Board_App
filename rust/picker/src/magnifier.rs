//! Borrow KDE's cursor-centered magnifier without changing enabled-effect settings.
use std::time::{Duration, Instant};
use zbus::{Connection, Error, Proxy};

pub struct Magnifier<'a> {
    effects: Proxy<'a>,
    shortcuts: Proxy<'a>,
    unload_magnifier: bool,
    reload_zoom: bool,
    reset_magnifier: bool,
}

impl<'a> Magnifier<'a> {
    pub async fn start(connection: &'a Connection) -> zbus::Result<Self> {
        let mut magnifier = Self {
            effects: Proxy::new(
                connection,
                "org.kde.KWin",
                "/Effects",
                "org.kde.kwin.Effects",
            )
            .await?,
            shortcuts: Proxy::new(
                connection,
                "org.kde.kglobalaccel",
                "/component/kwin",
                "org.kde.kglobalaccel.Component",
            )
            .await?,
            unload_magnifier: false,
            reload_zoom: false,
            reset_magnifier: false,
        };
        if let Err(error) = magnifier.activate().await {
            return match magnifier.restore().await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(Error::Failure(format!(
                    "{error}; restoring KDE magnification: {cleanup}"
                ))),
            };
        }
        Ok(magnifier)
    }

    async fn loaded(&self, effect: &str) -> zbus::Result<bool> {
        self.effects.call("isEffectLoaded", &(effect,)).await
    }

    async fn target(&self, effect: &str) -> zbus::Result<f64> {
        let info: String = self.effects.call("supportInformation", &(effect,)).await?;
        target_zoom(&info)
    }

    async fn load(&self, effect: &str) -> zbus::Result<()> {
        if self.loaded(effect).await? {
            return Ok(());
        }
        let loaded: bool = self.effects.call("loadEffect", &(effect,)).await?;
        if loaded {
            Ok(())
        } else {
            Err(Error::Failure(format!(
                "KWin could not load the {effect} effect"
            )))
        }
    }

    async fn activate(&mut self) -> zbus::Result<()> {
        let magnifier_loaded = self.loaded("magnifier").await?;
        if magnifier_loaded && self.target("magnifier").await? > 1.0 {
            return Ok(()); // Preserve magnification already in use for accessibility.
        }
        let zoom_loaded = self.loaded("zoom").await?;
        if zoom_loaded && self.target("zoom").await? > 1.0 {
            return Ok(());
        }
        if !magnifier_loaded {
            let supported: bool = self
                .effects
                .call("isEffectSupported", &("magnifier",))
                .await?;
            if !supported {
                return Err(Error::Failure(
                    "KWin's Magnifier effect is not supported".into(),
                ));
            }
            // Zoom and Magnifier share an exclusive group and shortcut names.
            // Do not change persistent Plugins/*Enabled preferences.
            if zoom_loaded {
                self.reload_zoom = true;
                self.effects
                    .call::<_, _, ()>("unloadEffect", &("zoom",))
                    .await?;
            }
            self.unload_magnifier = true;
            self.load("magnifier").await?;
        }
        if self.target("magnifier").await? <= 1.0 {
            self.reset_magnifier = true;
            self.toggle().await?;
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.target("magnifier").await? <= 1.0 {
                if Instant::now() >= deadline {
                    return Err(Error::Failure(
                        "KDE did not activate its Magnifier shortcut".into(),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            self.increase_zoom().await?;
        }
        Ok(())
    }

    async fn toggle(&self) -> zbus::Result<()> {
        self.shortcuts
            .call("invokeShortcut", &("view_actual_size",))
            .await
    }

    async fn increase_zoom(&self) -> zbus::Result<()> {
        const MINIMUM_ZOOM: f64 = 8.0;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut zoom = self.target("magnifier").await?;
        while zoom < MINIMUM_ZOOM {
            self.shortcuts
                .call::<_, _, ()>("invokeShortcut", &("view_zoom_in",))
                .await?;
            loop {
                let next = self.target("magnifier").await?;
                if next > zoom {
                    zoom = next;
                    break;
                }
                if next < zoom || Instant::now() >= deadline {
                    return Err(Error::Failure(
                        "KDE's Zoom In shortcut could not increase magnification to 8×".into(),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if zoom < MINIMUM_ZOOM && Instant::now() >= deadline {
                return Err(Error::Failure(
                    "KDE's Zoom In shortcut could not increase magnification to 8×".into(),
                ));
            }
        }
        Ok(())
    }

    pub async fn restore(&mut self) -> zbus::Result<()> {
        if self.reset_magnifier && self.loaded("magnifier").await? {
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.target("magnifier").await? > 1.0 {
                if Instant::now() >= deadline {
                    return Err(Error::Failure(
                        "KDE Magnifier could not reset; use Meta+0 or Desktop Effects".into(),
                    ));
                }
                // KWin's toggle uses the animated current zoom, not targetZoom.
                // A very fast pick can finish before the first rendered frame.
                self.toggle().await?;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        self.reset_magnifier = false;
        if self.unload_magnifier {
            self.effects
                .call::<_, _, ()>("unloadEffect", &("magnifier",))
                .await?;
            self.unload_magnifier = false;
        }
        if self.reload_zoom {
            self.load("zoom").await?;
            self.reload_zoom = false;
        }
        Ok(())
    }
}

fn target_zoom(info: &str) -> zbus::Result<f64> {
    info.lines()
        .find_map(|line| line.strip_prefix("targetZoom: "))
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|zoom| zoom.is_finite() && (1.0..=100.0).contains(zoom))
        .ok_or_else(|| Error::Failure("KWin did not report a valid magnification level".into()))
}

#[cfg(test)]
#[path = "magnifier_tests.rs"]
mod tests;
