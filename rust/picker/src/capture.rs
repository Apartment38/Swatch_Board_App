use std::{
    collections::{BTreeMap, BTreeSet},
    os::fd::OwnedFd,
    sync::{
        Arc, Condvar, Mutex,
        mpsc::{Receiver, TryRecvError},
    },
    time::{Duration, SystemTime},
};

use ashpd::desktop::screencast::Stream;
use pipewire as pw;
use pw::{properties::properties, spa};
use swatch_board_picker_core::{PixelPoint, Rgba8, RgbaFrame};

#[derive(Clone)]
pub struct CapturedFrame {
    pub stream_id: u32,
    pub stream_position: (i32, i32),
    pub stream_logical_size: (i32, i32),
    pub frame: Arc<RgbaFrame>,
    pub pointer: Option<PixelPoint>,
    /// Receipt of cursor metadata, separate from the age of the video image.
    pub cursor_updated_at: Option<SystemTime>,
    pub presentation_time_ns: Option<u64>,
    pub received_at: SystemTime,
}

/// Latest-only storage: a slow renderer never queues stale images.
#[derive(Clone, Default)]
pub struct CaptureFeed {
    shared: Arc<(Mutex<FeedState>, Condvar)>,
}

#[derive(Default)]
struct FeedState {
    frames: BTreeMap<u32, CapturedFrame>,
    authorized: Option<BTreeSet<u32>>,
    closed: bool,
    error: Option<String>,
}

impl CaptureFeed {
    pub fn new() -> Self {
        Self::default()
    }

    fn authorize(&self, ids: impl IntoIterator<Item = u32>) {
        let mut state = self.shared.0.lock().unwrap_or_else(|e| e.into_inner());
        let ids: BTreeSet<_> = ids.into_iter().collect();
        state.frames.retain(|id, _| ids.contains(id));
        state.authorized = Some(ids);
    }

    pub fn publish(&self, frame: CapturedFrame) {
        let (state, ready) = &*self.shared;
        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed
            || state
                .authorized
                .as_ref()
                .is_some_and(|ids| !ids.contains(&frame.stream_id))
        {
            return;
        }
        state.frames.insert(frame.stream_id, frame);
        ready.notify_all();
    }

    /// Wait without polling. A terminal close takes precedence over cached frames.
    pub fn wait_for_frame(&self, timeout: Duration) -> Result<CapturedFrame, String> {
        let (state, ready) = &*self.shared;
        let state = state.lock().unwrap_or_else(|e| e.into_inner());
        let (state, _) = ready
            .wait_timeout_while(state, timeout, |s| !s.closed && s.frames.is_empty())
            .unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err(state
                .error
                .clone()
                .unwrap_or_else(|| "PipeWire capture closed before a frame was available".into()));
        }
        state
            .frames
            .values()
            .next()
            .cloned()
            .ok_or_else(|| "timed out waiting for a PipeWire frame".into())
    }

    /// Terminal and idempotent; the first close reason is retained.
    pub fn close(&self, error: Option<String>) {
        let (state, ready) = &*self.shared;
        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
        if !state.closed {
            state.closed = true;
            state.error = error;
        }
        ready.notify_all();
    }

    pub fn failure(&self) -> Option<String> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .error
            .clone()
    }

    pub fn snapshot(&self) -> Vec<CapturedFrame> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .frames
            .values()
            .cloned()
            .collect()
    }
}

struct StreamState {
    stream_id: u32,
    stream_position: (i32, i32),
    stream_logical_size: (i32, i32),
    format: spa::param::video::VideoInfoRaw,
    cursor_meta_requested: bool,
    feed: CaptureFeed,
    connected: bool,
    latest: Option<CapturedFrame>,
}

fn cursor_pointer(
    cursor: &spa::buffer::meta::MetaCursor,
    width: u32,
    height: u32,
) -> Option<PixelPoint> {
    if !cursor.is_valid() || cursor.id() == 0 {
        return None;
    }
    // SPA position is the source-space pointer coordinate; hotspot only locates
    // the cursor bitmap and must not be added or subtracted here.
    let point = cursor.position();
    let x = u32::try_from(point.x).ok()?;
    let y = u32::try_from(point.y).ok()?;
    (x < width && y < height).then_some(PixelPoint { x, y })
}

impl StreamState {
    fn publish_update(
        &mut self,
        image: Option<RgbaFrame>,
        cursor: Option<&spa::buffer::meta::MetaCursor>,
        pts: Option<u64>,
        now: SystemTime,
    ) {
        let mut message = if let Some(image) = image {
            CapturedFrame {
                stream_id: self.stream_id,
                stream_position: self.stream_position,
                stream_logical_size: self.stream_logical_size,
                frame: Arc::new(image),
                pointer: self.latest.as_ref().and_then(|f| f.pointer),
                cursor_updated_at: self.latest.as_ref().and_then(|f| f.cursor_updated_at),
                presentation_time_ns: pts,
                received_at: now,
            }
        } else {
            // Metadata-only buffers cannot create pixels or refresh image age.
            let Some(previous) = &self.latest else {
                return;
            };
            if cursor.is_none() {
                return;
            }
            previous.clone()
        };
        if let Some(cursor) = cursor {
            message.pointer = cursor_pointer(cursor, message.frame.width, message.frame.height);
            message.cursor_updated_at = Some(now);
        } else {
            message.pointer = message
                .pointer
                .filter(|p| p.x < message.frame.width && p.y < message.frame.height);
        }
        self.latest = Some(message.clone());
        self.feed.publish(message);
    }
}

fn stream_failure(connected: &mut bool, state: &pw::stream::StreamState) -> Option<String> {
    use pw::stream::StreamState::*;
    match state {
        Error(error) => Some(error.clone()),
        Unconnected if *connected => Some("stream disconnected after connecting".into()),
        Paused | Streaming => {
            *connected = true;
            None
        }
        _ => None,
    }
}

fn stop_requested(stop: &Receiver<()>) -> bool {
    !matches!(stop.try_recv(), Err(TryRecvError::Empty))
}

/// Run PipeWire on its own thread: pipewire-rs uses thread-local Rc objects.
pub fn run(
    fd: OwnedFd,
    portal_streams: Vec<Stream>,
    feed: CaptureFeed,
    stop: Receiver<()>,
) -> Result<(), String> {
    let result =
        run_inner(fd, portal_streams, feed.clone(), stop).map_err(|error| error.to_string());
    feed.close(result.as_ref().err().cloned());
    result
}

fn run_inner(
    fd: OwnedFd,
    portal_streams: Vec<Stream>,
    feed: CaptureFeed,
    stop: Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    feed.authorize(portal_streams.iter().map(Stream::pipe_wire_node_id));
    if portal_streams.is_empty() {
        return Err("portal did not authorize any monitor streams".into());
    }
    if stop_requested(&stop) {
        return Ok(());
    }
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;
    let mut streams = Vec::with_capacity(portal_streams.len());
    let mut listeners = Vec::with_capacity(portal_streams.len());

    for (index, portal_stream) in portal_streams.iter().enumerate() {
        let node_id = portal_stream.pipe_wire_node_id();
        let (width, height) = portal_stream
            .size()
            .ok_or("portal monitor stream did not include dimensions")?;
        let position = portal_stream
            .position()
            .ok_or("portal monitor stream did not include position")?;
        if width <= 0 || height <= 0 {
            return Err("portal provided invalid stream dimensions".into());
        }
        let state = StreamState {
            stream_id: node_id,
            stream_position: position,
            stream_logical_size: (width, height),
            format: Default::default(),
            cursor_meta_requested: false,
            feed: feed.clone(),
            connected: false,
            latest: None,
        };
        let stream = pw::stream::StreamRc::new(
            core.clone(),
            &format!("swatch-board-screen-{index}"),
            properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )?;
        let listener = stream
            .add_local_listener_with_user_data(state)
            .state_changed(|_, data, old, new| {
                eprintln!(
                    "PipeWire stream {} changed from {old:?} to {new:?}",
                    data.stream_id
                );
                if let Some(error) = stream_failure(&mut data.connected, &new) {
                    data.feed.close(Some(format!(
                        "PipeWire stream {} failed: {error}",
                        data.stream_id
                    )));
                }
            })
            .param_changed(|stream, data, id, param| {
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
                } else {
                    eprintln!(
                        "PipeWire stream {} negotiated {:?} {}x{}",
                        data.stream_id,
                        data.format.format(),
                        data.format.size().width,
                        data.format.size().height
                    );
                    if !data.cursor_meta_requested {
                        match request_cursor_metadata(stream) {
                            Ok(()) => {
                                data.cursor_meta_requested = true;
                                eprintln!(
                                    "Requested PipeWire cursor metadata for stream {}",
                                    data.stream_id
                                );
                            }
                            Err(error) => eprintln!(
                                "Could not request PipeWire cursor metadata for stream {}: {error}",
                                data.stream_id
                            ),
                        }
                    }
                }
            })
            .process(|stream, data| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let frame = if let Some(plane) = buffer.datas_mut().first_mut() {
                    let chunk = plane.chunk();
                    let offset = chunk.offset() as usize;
                    let size = chunk.size() as usize;
                    let stride = chunk.stride();
                    if chunk.flags().contains(spa::buffer::ChunkFlags::CORRUPTED) {
                        return;
                    }
                    if size == 0 {
                        None // PipeWire can send cursor-only buffers.
                    } else {
                        let Some(plane_data) = plane.data() else {
                            return;
                        };
                        let Some(end) = offset.checked_add(size) else {
                            return;
                        };
                        let Some(bytes) = plane_data.get(offset..end) else {
                            return;
                        };
                        let Some(image) = decode_frame(
                            bytes,
                            data.format.size().width,
                            data.format.size().height,
                            stride,
                            data.format.format(),
                        ) else {
                            return;
                        };
                        Some(image)
                    }
                } else {
                    None
                };
                // pipewire-rs find_meta verifies the metadata allocation is at
                // least size_of::<T>(); never dereference bitmap offsets.
                let cursor = buffer.find_meta::<spa::buffer::meta::MetaCursor>();
                let presentation_time_ns = buffer
                    .find_meta::<spa::buffer::meta::MetaHeader>()
                    .and_then(|header| u64::try_from(header.pts()).ok());
                data.publish_update(frame, cursor, presentation_time_ns, SystemTime::now());
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
                Choice,
                Range,
                Rectangle,
                spa::utils::Rectangle {
                    width: width as u32,
                    height: height as u32,
                },
                spa::utils::Rectangle {
                    width: 1,
                    height: 1,
                },
                spa::utils::Rectangle {
                    width: 8192,
                    height: 8192,
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
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )?;
        listeners.push(listener);
        streams.push(stream);
    }

    loop {
        if let Some(error) = feed.failure() {
            return Err(error.into());
        }
        if stop_requested(&stop) {
            break;
        }
        mainloop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(100)));
    }
    Ok(())
}

fn request_cursor_metadata(stream: &pw::stream::Stream) -> Result<(), Box<dyn std::error::Error>> {
    let cursor_meta_min = std::mem::size_of::<spa::sys::spa_meta_cursor>();
    let cursor_meta_default =
        cursor_meta_min + std::mem::size_of::<spa::sys::spa_meta_bitmap>() + 64 * 64 * 4;
    let cursor_meta_max =
        cursor_meta_min + std::mem::size_of::<spa::sys::spa_meta_bitmap>() + 256 * 256 * 4;
    let cursor_meta = meta_param(
        spa::sys::SPA_META_Cursor,
        spa::pod::Value::Choice(spa::pod::ChoiceValue::Int(spa::utils::Choice(
            spa::utils::ChoiceFlags::empty(),
            spa::utils::ChoiceEnum::Range {
                default: cursor_meta_default as i32,
                min: cursor_meta_min as i32,
                max: cursor_meta_max as i32,
            },
        ))),
    )?;
    let header_meta = meta_param(
        spa::sys::SPA_META_Header,
        spa::pod::Value::Int(std::mem::size_of::<spa::sys::spa_meta_header>() as i32),
    )?;
    let mut params = [
        spa::pod::Pod::from_bytes(&cursor_meta).ok_or("PipeWire rejected cursor metadata pod")?,
        spa::pod::Pod::from_bytes(&header_meta).ok_or("PipeWire rejected header metadata pod")?,
    ];
    stream.update_params(&mut params)?;
    Ok(())
}

fn meta_param(
    meta_type: u32,
    size: spa::pod::Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamMeta.as_raw(),
        id: spa::param::ParamType::Meta.as_raw(),
        properties: vec![
            spa::pod::Property::new(
                spa::sys::SPA_PARAM_META_type,
                spa::pod::Value::Id(spa::utils::Id(meta_type)),
            ),
            spa::pod::Property::new(spa::sys::SPA_PARAM_META_size, size),
        ],
    };
    Ok(spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(metadata),
    )?
    .0
    .into_inner())
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

#[cfg(test)]
mod tests {
    use super::*;
    use spa::param::video::VideoFormat;

    fn sample(stream_id: u32, red: u8) -> CapturedFrame {
        CapturedFrame {
            stream_id,
            stream_position: (0, 0),
            stream_logical_size: (1, 1),
            frame: std::sync::Arc::new(
                decode_frame(&[red, 0, 0, 255], 1, 1, 4, VideoFormat::RGBA).unwrap(),
            ),
            pointer: None,
            cursor_updated_at: None,
            presentation_time_ns: None,
            received_at: SystemTime::now(),
        }
    }

    #[test]
    fn feed_replaces_backlog_with_latest_frame_per_stream() {
        let feed = CaptureFeed::new();
        for red in 0..100 {
            feed.publish(sample(9, red));
        }
        feed.publish(sample(2, 42));
        let frames = feed.snapshot();
        assert_eq!(
            frames.iter().map(|f| f.stream_id).collect::<Vec<_>>(),
            vec![2, 9]
        );
        assert_eq!(frames[1].frame.pixels[0].r, 99);
        assert!(std::sync::Arc::ptr_eq(
            &frames[1].frame,
            &feed.snapshot()[1].frame
        ));
    }

    #[test]
    fn feed_wait_wakes_for_publication_and_close() {
        let feed = CaptureFeed::new();
        assert!(
            feed.wait_for_frame(Duration::ZERO)
                .err()
                .unwrap()
                .contains("timed out")
        );
        let worker_feed = feed.clone();
        let worker = std::thread::spawn(move || worker_feed.wait_for_frame(Duration::from_secs(2)));
        feed.publish(sample(1, 33));
        assert_eq!(worker.join().unwrap().unwrap().frame.pixels[0].r, 33);
        feed.close(Some("capture disconnected".into()));
        assert_eq!(feed.failure().as_deref(), Some("capture disconnected"));
        assert_eq!(
            feed.wait_for_frame(Duration::from_secs(2)).err().as_deref(),
            Some("capture disconnected")
        );
        feed.publish(sample(2, 0));
        assert_eq!(feed.snapshot().len(), 1);
        feed.close(None);
        assert_eq!(feed.failure().as_deref(), Some("capture disconnected"));

        let closed = CaptureFeed::new();
        let worker_feed = closed.clone();
        let worker = std::thread::spawn(move || worker_feed.wait_for_frame(Duration::from_secs(2)));
        closed.close(None);
        assert!(worker.join().unwrap().err().unwrap().contains("closed"));
        assert!(closed.failure().is_none());
    }

    #[test]
    fn feed_only_accepts_authorized_streams_during_capture() {
        let feed = CaptureFeed::new();
        feed.authorize([1, 2]);
        feed.publish(sample(3, 0));
        feed.publish(sample(1, 1));
        assert_eq!(feed.snapshot().len(), 1);
        assert_eq!(feed.snapshot()[0].stream_id, 1);
    }

    #[test]
    fn terminal_stream_states_report_failure_without_initial_disconnect_false_alarm() {
        let mut connected = false;
        assert!(stream_failure(&mut connected, &pw::stream::StreamState::Unconnected).is_none());
        assert!(stream_failure(&mut connected, &pw::stream::StreamState::Connecting).is_none());
        assert!(stream_failure(&mut connected, &pw::stream::StreamState::Paused).is_none());
        assert!(connected);
        assert!(
            stream_failure(&mut connected, &pw::stream::StreamState::Unconnected)
                .unwrap()
                .contains("disconnected")
        );
        assert_eq!(
            stream_failure(&mut false, &pw::stream::StreamState::Error("denied".into())).as_deref(),
            Some("denied")
        );
    }

    #[test]
    fn stop_request_or_dropped_sender_ends_capture() {
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(!stop_requested(&rx));
        tx.send(()).unwrap();
        assert!(stop_requested(&rx));
        drop(tx);
        assert!(stop_requested(&rx));
    }

    #[test]
    fn cursor_metadata_requires_valid_id_and_frame_bounds_not_hotspot() {
        let mut raw: spa::sys::spa_meta_cursor = unsafe { std::mem::zeroed() };
        raw.position = spa::utils::Point { x: 3, y: 2 };
        raw.hotspot = spa::utils::Point { x: 100, y: 100 };
        let read = |raw: &spa::sys::spa_meta_cursor| {
            // MetaCursor is repr(transparent) over spa_meta_cursor in libspa 0.10.1.
            cursor_pointer(
                unsafe { &*(raw as *const _ as *const spa::buffer::meta::MetaCursor) },
                4,
                3,
            )
        };
        assert_eq!(read(&raw), None);
        raw.id = 1;
        assert_eq!(read(&raw), Some(PixelPoint { x: 3, y: 2 }));
        for (x, y) in [(-1, 0), (0, -1), (4, 0), (0, 3), (i32::MAX, i32::MAX)] {
            raw.position = spa::utils::Point { x, y };
            assert_eq!(read(&raw), None);
        }
    }

    #[test]
    fn cursor_only_update_reuses_image_without_refreshing_video_age() {
        let feed = CaptureFeed::new();
        let mut state = StreamState {
            stream_id: 1,
            stream_position: (0, 0),
            stream_logical_size: (1, 1),
            format: Default::default(),
            cursor_meta_requested: false,
            feed: feed.clone(),
            connected: false,
            latest: None,
        };
        let first_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
        let later = first_time + Duration::from_secs(5);
        let image = decode_frame(&[1, 2, 3, 255], 1, 1, 4, VideoFormat::RGBA).unwrap();
        state.publish_update(Some(image), None, Some(100), first_time);
        let first = feed.snapshot().remove(0);
        let mut raw: spa::sys::spa_meta_cursor = unsafe { std::mem::zeroed() };
        raw.id = 1;
        let cursor = unsafe { &*(&raw as *const _ as *const spa::buffer::meta::MetaCursor) };
        state.publish_update(None, Some(cursor), Some(200), later);
        let next = feed.snapshot().remove(0);
        assert!(Arc::ptr_eq(&first.frame, &next.frame));
        assert_eq!(next.received_at, first_time);
        assert_eq!(next.presentation_time_ns, Some(100));
        assert_eq!(next.cursor_updated_at, Some(later));
        assert_eq!(next.pointer, Some(PixelPoint { x: 0, y: 0 }));
        raw.id = 0;
        let cursor = unsafe { &*(&raw as *const _ as *const spa::buffer::meta::MetaCursor) };
        state.publish_update(None, Some(cursor), None, later);
        assert_eq!(feed.snapshot()[0].pointer, None);
    }

    #[test]
    fn decodes_all_negotiated_channel_orders_and_alpha() {
        for (format, bytes, alpha) in [
            (VideoFormat::BGRA, [30, 20, 10, 40], 40),
            (VideoFormat::BGRx, [30, 20, 10, 0], 255),
            (VideoFormat::RGBA, [10, 20, 30, 40], 40),
            (VideoFormat::RGBx, [10, 20, 30, 0], 255),
        ] {
            let frame = decode_frame(&bytes, 1, 1, 4, format).unwrap();
            assert_eq!(
                frame.pixels,
                vec![Rgba8 {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: alpha
                }],
                "format {format:?}"
            );
        }
    }

    #[test]
    fn skips_stride_padding_between_rows() {
        let bytes = [1, 2, 3, 255, 99, 99, 99, 99, 4, 5, 6, 255, 88, 88, 88, 88];
        let frame = decode_frame(&bytes, 1, 2, 8, VideoFormat::RGBA).unwrap();
        assert_eq!(
            frame.pixels,
            vec![
                Rgba8 {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255
                },
                Rgba8 {
                    r: 4,
                    g: 5,
                    b: 6,
                    a: 255
                },
            ]
        );
    }

    #[test]
    fn rejects_malformed_dimensions_strides_and_buffers() {
        for (width, height, stride, bytes) in [
            (0, 1, 4, &[0; 4][..]),
            (1, 0, 4, &[0; 4][..]),
            (1, 1, 0, &[0; 4][..]),
            (1, 1, -4, &[0; 4][..]),
            (2, 1, 4, &[0; 8][..]),
            (1, 2, 4, &[0; 7][..]),
            (u32::MAX, u32::MAX, i32::MAX, &[][..]),
        ] {
            assert!(decode_frame(bytes, width, height, stride, VideoFormat::RGBA).is_none());
        }
    }

    #[test]
    fn rejects_unnegotiated_pixel_formats() {
        assert!(decode_frame(&[0; 4], 1, 1, 4, VideoFormat::RGB).is_none());
    }

    #[test]
    fn decodes_consecutive_frames_without_reusing_previous_pixels() {
        let first = decode_frame(&[255, 0, 0, 255], 1, 1, 4, VideoFormat::RGBA).unwrap();
        let next = decode_frame(&[0, 0, 255, 255], 1, 1, 4, VideoFormat::RGBA).unwrap();
        assert_eq!(
            first.pixels[0],
            Rgba8 {
                r: 255,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(
            next.pixels[0],
            Rgba8 {
                r: 0,
                g: 0,
                b: 255,
                a: 255
            }
        );
    }
}
