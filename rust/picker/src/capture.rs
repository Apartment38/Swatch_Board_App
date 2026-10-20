use std::{
    os::fd::OwnedFd,
    sync::mpsc::{Receiver, SyncSender},
    time::SystemTime,
};

use ashpd::desktop::screencast::Stream;
use colorboard_picker_core::{PixelPoint, Rgba8, RgbaFrame};
use pipewire as pw;
use pw::{properties::properties, spa};

pub struct CapturedFrame {
    pub stream_id: u32,
    pub frame: RgbaFrame,
    pub pointer: Option<PixelPoint>,
    pub presentation_time_ns: Option<u64>,
    pub received_at: SystemTime,
}

struct StreamState {
    stream_id: u32,
    format: spa::param::video::VideoInfoRaw,
    sender: SyncSender<CapturedFrame>,
}

/// Run PipeWire on its own thread: pipewire-rs uses thread-local Rc objects.
pub fn run(
    fd: OwnedFd,
    portal_streams: Vec<Stream>,
    sender: SyncSender<CapturedFrame>,
    stop: Receiver<()>,
) {
    if let Err(error) = run_inner(fd, portal_streams, sender, stop) {
        eprintln!("PipeWire capture stopped: {error}");
    }
}

fn run_inner(
    fd: OwnedFd,
    portal_streams: Vec<Stream>,
    sender: SyncSender<CapturedFrame>,
    stop: Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;
    let mut streams = Vec::with_capacity(portal_streams.len());

    for (index, portal_stream) in portal_streams.iter().enumerate() {
        let node_id = portal_stream.pipe_wire_node_id();
        let (width, height) = portal_stream
            .size()
            .ok_or("portal monitor stream did not include dimensions")?;
        if width <= 0 || height <= 0 {
            return Err("portal provided invalid stream dimensions".into());
        }
        let state = StreamState {
            stream_id: node_id,
            format: Default::default(),
            sender: sender.clone(),
        };
        let stream = pw::stream::StreamRc::new(
            core.clone(),
            &format!("colorboard-screen-{index}"),
            properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )?;
        let _listener = stream
            .add_local_listener_with_user_data(state)
            .param_changed(|_, data, id, param| {
                if id != pw::spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let Some(param) = param else { return };
                let Ok((media_type, media_subtype)) = spa::param::format_utils::parse_format(param)
                else {
                    return;
                };
                if media_type != spa::param::format::MediaType::Video
                    || media_subtype != spa::param::format::MediaSubtype::Raw
                {
                    return;
                }
                if data.format.parse(param).is_err() {
                    eprintln!(
                        "Ignoring invalid PipeWire video format for stream {}",
                        data.stream_id
                    );
                }
            })
            .process(|stream, data| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let Some(plane) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let chunk = plane.chunk();
                let offset = chunk.offset() as usize;
                let size = chunk.size() as usize;
                let stride = chunk.stride();
                if chunk.flags().contains(spa::buffer::ChunkFlags::CORRUPTED) {
                    return;
                }
                let Some(plane_data) = plane.data() else {
                    return;
                };
                let Some(end) = offset.checked_add(size) else {
                    return;
                };
                let Some(bytes) = plane_data.get(offset..end) else {
                    return;
                };
                let width = data.format.size().width;
                let height = data.format.size().height;
                let Some(frame) = decode_frame(bytes, width, height, stride, data.format.format())
                else {
                    return;
                };
                let pointer = buffer
                    .find_meta::<spa::buffer::meta::MetaCursor>()
                    .filter(|cursor| cursor.is_valid())
                    .map(|cursor| cursor.position())
                    .and_then(|point| {
                        (point.x >= 0 && point.y >= 0).then_some(PixelPoint {
                            x: point.x as u32,
                            y: point.y as u32,
                        })
                    });
                let presentation_time_ns = buffer
                    .find_meta::<spa::buffer::meta::MetaHeader>()
                    .and_then(|header| u64::try_from(header.pts()).ok());
                let message = CapturedFrame {
                    stream_id: data.stream_id,
                    frame,
                    pointer,
                    presentation_time_ns,
                    received_at: SystemTime::now(),
                };
                // Keep the newest frames when the renderer is behind.
                if let Err(std::sync::mpsc::TrySendError::Full(_)) = data.sender.try_send(message) {
                    // The bounded receiver is intentionally allowed to drop stale lens updates.
                }
            })
            .register()?;

        let format = spa::pod::object!(
            spa::utils::SpaTypes::ObjectParamFormat,
            spa::param::ParamType::EnumFormat,
            spa::pod::property!(
                spa::param::format::FormatProperties::MediaType,
                Id,
                spa::param::format::MediaType::Video
            ),
            spa::pod::property!(
                spa::param::format::FormatProperties::MediaSubtype,
                Id,
                spa::param::format::MediaSubtype::Raw
            ),
            spa::pod::property!(
                spa::param::format::FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                spa::param::video::VideoFormat::BGRA,
                spa::param::video::VideoFormat::BGRA,
                spa::param::video::VideoFormat::BGRx,
                spa::param::video::VideoFormat::RGBA,
                spa::param::video::VideoFormat::RGBx
            ),
            spa::pod::property!(
                spa::param::format::FormatProperties::VideoSize,
                Rectangle,
                spa::utils::Rectangle {
                    width: width as u32,
                    height: height as u32
                }
            )
        );
        let bytes = spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &spa::pod::Value::Object(format),
        )?
        .0
        .into_inner();
        let mut params = [spa::pod::Pod::from_bytes(&bytes)
            .ok_or("PipeWire rejected the requested video format pod")?];
        stream.connect(
            spa::utils::Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;
        streams.push(stream);
    }

    while stop.try_recv().is_err() {
        mainloop.loop_().iterate(pw::loop_::Timeout::Finite(
            std::time::Duration::from_millis(100),
        ));
    }
    Ok(())
}

fn decode_frame(
    bytes: &[u8],
    width: u32,
    height: u32,
    stride: i32,
    format: spa::param::video::VideoFormat,
) -> Option<RgbaFrame> {
    if width == 0 || height == 0 || stride <= 0 {
        return None;
    }
    let stride = stride as usize;
    if stride < (width as usize).checked_mul(4)?
        || bytes.len() < stride.checked_mul(height as usize)?
    {
        return None;
    }
    let bgra = matches!(
        format,
        spa::param::video::VideoFormat::BGRA | spa::param::video::VideoFormat::BGRx
    );
    let rgba = matches!(
        format,
        spa::param::video::VideoFormat::RGBA | spa::param::video::VideoFormat::RGBx
    );
    if !bgra && !rgba {
        return None;
    }
    let mut pixels = Vec::with_capacity((width as usize).checked_mul(height as usize)?);
    for row in bytes.chunks(stride).take(height as usize) {
        for px in row.chunks_exact(4).take(width as usize) {
            pixels.push(if bgra {
                Rgba8 {
                    r: px[2],
                    g: px[1],
                    b: px[0],
                    a: if format == spa::param::video::VideoFormat::BGRA {
                        px[3]
                    } else {
                        255
                    },
                }
            } else {
                Rgba8 {
                    r: px[0],
                    g: px[1],
                    b: px[2],
                    a: if format == spa::param::video::VideoFormat::RGBA {
                        px[3]
                    } else {
                        255
                    },
                }
            });
        }
    }
    RgbaFrame::new(width, height, pixels)
}
