use crate::Request;
use ksni::{Tray, TrayMethods, menu::StandardItem};
use tokio::sync::{mpsc, oneshot};

pub struct Item {
    requests: mpsc::UnboundedSender<Request>,
    pub color: Option<String>,
    pub message: Option<String>,
}

impl Item {
    fn pick(&self) {
        let (reply, _) = oneshot::channel();
        let _ = self.requests.send(Request::Pick(reply));
    }
}

impl Tray for Item {
    fn id(&self) -> String {
        "swatch-board".into()
    }
    fn title(&self) -> String {
        "Swatch Board".into()
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.pick();
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        [16, 22, 32, 48, 64]
            .into_iter()
            .map(|size| icon(size, self.color.as_deref()))
            .collect()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Swatch Board".into(),
            description: self.message.clone().unwrap_or_else(|| {
                self.color
                    .clone()
                    .unwrap_or_else(|| "No color picked".into())
            }),
            ..Default::default()
        }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Pick Color".into(),
                activate: Box::new(|item: &mut Self| item.pick()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show Palette".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Board".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Configure".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|item: &mut Self| {
                    let _ = item.requests.send(Request::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn icon(size: i32, color: Option<&str>) -> ksni::Icon {
    let rgb = color
        .and_then(|s| u32::from_str_radix(s.strip_prefix('#')?, 16).ok())
        .unwrap_or(0x888888);
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let x = (x * 32) / size;
            let y = (y * 32) / size;
            let dot = (x - 25).pow(2) + (y - 25).pow(2);
            let diagonal = x + y;
            let pixel = if dot <= 36 {
                if dot >= 20 {
                    [255, 245, 245, 245]
                } else {
                    [255, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
                }
            } else if (22..=34).contains(&diagonal)
                && (x - y - 3).abs() <= 5
                && (5..=25).contains(&x)
            {
                if (x - y - 3).abs() >= 4 {
                    [255, 20, 20, 20]
                } else {
                    [255, 235, 235, 235]
                }
            } else if (x - 23).pow(2) + (y - 7).pow(2) <= 30 {
                [255, 70, 70, 70]
            } else {
                [0, 0, 0, 0]
            };
            data.extend_from_slice(&pixel);
        }
    }
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

pub async fn start(
    requests: mpsc::UnboundedSender<Request>,
) -> Result<ksni::Handle<Item>, ksni::Error> {
    Item {
        requests,
        color: None,
        message: None,
    }
    .spawn()
    .await
}
