//! Rust-owned selection with compositor-presented lens authority.
//! Authorized monitor streams are routed by pointer shield output; KWin's ScreenCast frames
//! may already be compositor-oriented, while raw transformed frames remain supported.
//! All known outputs are shielded; unauthorized/ambiguous outputs are rejected. Active deadline
//! 120s, presentation deadline 2s, clean-video deadline 3s. Success/cancel waits for all observed
//! button releases. A live geometry-failure case kept a held-button release off both fixtures;
//! untested failure stages still fail closed rather than claiming click safety.
use crate::native_guard::Sample;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Selected(Sample),
    Cancelled,
}
fn release_outcome(button: u32, sample: Option<Sample>) -> std::result::Result<Outcome, String> {
    match button {
        273 => Ok(Outcome::Cancelled),
        272 => sample
            .map(Outcome::Selected)
            .ok_or_else(|| "no compositor-presented native sample at left release".into()),
        _ => Err("unsupported confirmation button".into()),
    }
}
fn consume_release(
    pressed: &mut BTreeSet<(u32, u32)>,
    decision: &mut Option<std::result::Result<Outcome, String>>,
    pointer: u32,
    button: u32,
    sample: Option<Sample>,
) -> bool {
    let paired = pressed.remove(&(pointer, button));
    if paired && [LEFT, RIGHT].contains(&button) && decision.is_none() {
        *decision = Some(release_outcome(button, sample));
    }
    decision.is_some() && pressed.is_empty()
}

fn finish_outcome(
    decision: Option<std::result::Result<Outcome, String>>,
    held: bool,
) -> Result<Outcome> {
    if held {
        return Err("native timeout/error with buttons held: click safety NOT established".into());
    }
    decision
        .ok_or_else(|| "native selection timed out without a result".into())
        .and_then(|r| r.map_err(Into::into))
}

// A target output can still have an old cached image when the pointer arrives.
// During a hide transition, wait for fresh video under the existing hide deadline;
// never present or select that cached image.
fn frame_is_ready(
    age: Duration,
    awaiting_clean_frame: bool,
) -> std::result::Result<bool, &'static str> {
    if age <= Duration::from_secs(2) {
        Ok(true)
    } else if awaiting_clean_frame {
        Ok(false)
    } else {
        Err("stale capture image")
    }
}

fn validate_trigger(trigger: &str) -> std::result::Result<(), String> {
    if matches!(trigger.trim(), "Esc" | "Escape") {
        Ok(())
    } else {
        Err(format!(
            "effective global cancel trigger is not recognizable as bare Escape: {trigger:?}"
        ))
    }
}
#[derive(Clone, Debug)]
struct CaptureRoute {
    id: u32,
    origin: (i32, i32),
    portal_logical: (i32, i32),
    logical: (i32, i32),
    display_transform: OutputTransform,
    capture_transform: Option<OutputTransform>,
    physical: Option<(u32, u32)>,
}
fn route_for_output(
    routes: &[CaptureRoute],
    origin: (i32, i32),
    logical: (i32, i32),
) -> Result<Option<usize>> {
    let mut matching = routes
        .iter()
        .enumerate()
        .filter(|(_, r)| r.origin == origin);
    let Some((index, route)) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err("ambiguous authorized output streams".into());
    }
    if route.portal_logical.0 <= 0
        || route.portal_logical.1 <= 0
        || logical.0 <= 0
        || logical.1 <= 0
    {
        return Err("invalid portal/Wayland output geometry".into());
    }
    if route.portal_logical != logical {
        eprintln!(
            "PORTAL_GEOMETRY_DIFFERENCE stream={} portal={:?} wayland={logical:?}; using Wayland coordinates",
            route.id, route.portal_logical
        );
    }
    Ok(Some(index))
}

fn raw_dimensions(physical: (u32, u32), transform: OutputTransform) -> (u32, u32) {
    match transform {
        OutputTransform::Rotate90
        | OutputTransform::Rotate270
        | OutputTransform::Flipped90
        | OutputTransform::Flipped270 => (physical.1, physical.0),
        _ => physical,
    }
}

fn output_transform(transform: wl_output::Transform) -> OutputTransform {
    match transform {
        wl_output::Transform::Normal => OutputTransform::Normal,
        wl_output::Transform::_90 => OutputTransform::Rotate90,
        wl_output::Transform::_180 => OutputTransform::Rotate180,
        wl_output::Transform::_270 => OutputTransform::Rotate270,
        wl_output::Transform::Flipped => OutputTransform::Flipped,
        wl_output::Transform::Flipped90 => OutputTransform::Flipped90,
        wl_output::Transform::Flipped180 => OutputTransform::Flipped180,
        wl_output::Transform::Flipped270 => OutputTransform::Flipped270,
        _ => unreachable!("wl_output transform enum exhausted"),
    }
}
fn lens_geometry(
    logical: (i32, i32),
    physical: (u32, u32),
    transform: OutputTransform,
    radius: u32,
    zoom: u32,
) -> Result<(u32, u32)> {
    let (w, h) = raw_dimensions(physical, transform);
    if logical.0 <= 0 || logical.1 <= 0 || w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err("unsupported capture dimensions".into());
    }
    let sx = w as f64 / logical.0 as f64;
    let sy = h as f64 / logical.1 as f64;
    // Each logical axis may have rounded by one physical pixel.
    let tolerance = 1. / logical.0 as f64 + 1. / logical.1 as f64;
    if (sx - sy).abs() > tolerance || !(0.5..=8.).contains(&sx) || !(0.5..=8.).contains(&sy) {
        return Err("nonuniform/unsupported capture scale".into());
    }
    let required = radius
        .checked_mul(2)
        .and_then(|r| r.checked_add(1))
        .and_then(|r| r.checked_mul(zoom))
        .and_then(|r| r.checked_add(8))
        .ok_or("magnifier dimensions overflow")?;
    let scale = sx.max(sy);
    let logical_side = ((required as f64 / scale).ceil() as u32).max(160);
    let buffer_side = (logical_side as f64 * scale).ceil() as u32;
    if !(required..=4096).contains(&buffer_side) {
        return Err("physical lens buffer cannot fit magnifier".into());
    }
    Ok((logical_side, buffer_side))
}
fn choose_capture_transform(
    logical: (i32, i32),
    physical: (u32, u32),
    display_transform: OutputTransform,
    radius: u32,
    zoom: u32,
) -> Result<OutputTransform> {
    if lens_geometry(logical, physical, OutputTransform::Normal, radius, zoom).is_ok() {
        return Ok(OutputTransform::Normal);
    }
    lens_geometry(logical, physical, display_transform, radius, zoom)?;
    Ok(display_transform)
}
fn untransformed_point(
    point: PixelPoint,
    physical: (u32, u32),
    transform: OutputTransform,
) -> Option<PixelPoint> {
    if point.x >= physical.0 || point.y >= physical.1 {
        return None;
    }
    let (raw_w, raw_h) = raw_dimensions(physical, transform);
    let (x, y) = match transform {
        OutputTransform::Normal => (point.x, point.y),
        OutputTransform::Rotate90 => (point.y, raw_h.checked_sub(point.x + 1)?),
        OutputTransform::Rotate180 => (
            raw_w.checked_sub(point.x + 1)?,
            raw_h.checked_sub(point.y + 1)?,
        ),
        OutputTransform::Rotate270 => (raw_w.checked_sub(point.y + 1)?, point.x),
        OutputTransform::Flipped => (raw_w.checked_sub(point.x + 1)?, point.y),
        OutputTransform::Flipped90 => (
            raw_w.checked_sub(point.y + 1)?,
            raw_h.checked_sub(point.x + 1)?,
        ),
        OutputTransform::Flipped180 => (point.x, raw_h.checked_sub(point.y + 1)?),
        OutputTransform::Flipped270 => (point.y, point.x),
    };
    (x < raw_w && y < raw_h).then_some(PixelPoint { x, y })
}
fn source_rect(
    origin: (i32, i32),
    logical: (i32, i32),
    physical: (u32, u32),
    point: PixelPoint,
    transform: OutputTransform,
    radius: u32,
) -> Option<Rect> {
    let point = untransformed_point(point, physical, transform)?;
    let (raw_w, raw_h) = raw_dimensions(physical, transform);
    let sx = logical.0 as f64 / raw_w as f64;
    let sy = logical.1 as f64 / raw_h as f64;
    let x0 = i32::try_from((point.x.saturating_sub(radius) as f64 * sx).floor() as i64 - 2).ok()?;
    let y0 = i32::try_from((point.y.saturating_sub(radius) as f64 * sy).floor() as i64 - 2).ok()?;
    let x1 = i32::try_from(
        (point.x.saturating_add(radius).saturating_add(1).min(raw_w) as f64 * sx).ceil() as i64 + 2,
    )
    .ok()?;
    let y1 = i32::try_from(
        (point.y.saturating_add(radius).saturating_add(1).min(raw_h) as f64 * sy).ceil() as i64 + 2,
    )
    .ok()?;
    global_rect(origin, (x0, y0), (x1.checked_sub(x0)?, y1.checked_sub(y0)?))
}

fn global_rect(origin: (i32, i32), local: (i32, i32), size: (i32, i32)) -> Option<Rect> {
    let x = origin.0.checked_add(local.0)?;
    let y = origin.1.checked_add(local.1)?;
    x.checked_add(size.0)?;
    y.checked_add(size.1)?;
    Some(Rect {
        x,
        y,
        w: size.0,
        h: size.1,
    })
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    #[test]
    fn portal_geometry_difference_uses_wayland_geometry() {
        let routes = vec![CaptureRoute {
            id: 9,
            origin: (1024, 0),
            portal_logical: (585, 439),
            logical: (585, 439),
            display_transform: OutputTransform::Normal,
            capture_transform: Some(OutputTransform::Normal),
            physical: Some((585, 439)),
        }];
        assert_eq!(
            route_for_output(&routes, (1024, 0), (335, 251)).unwrap(),
            Some(0)
        );
        assert!(route_for_output(&routes, (1024, 0), (0, 251)).is_err());
    }
    fn routes() -> Vec<CaptureRoute> {
        vec![
            CaptureRoute {
                id: 40,
                origin: (0, 0),
                portal_logical: (1024, 768),
                logical: (1024, 768),
                display_transform: OutputTransform::Normal,
                capture_transform: None,
                physical: None,
            },
            CaptureRoute {
                id: 3,
                origin: (-1024, -100),
                portal_logical: (1024, 768),
                logical: (1024, 768),
                display_transform: OutputTransform::Normal,
                capture_transform: None,
                physical: None,
            },
        ]
    }
    #[test]
    fn rapid_output_retarget_preserves_hide_ticket_and_latest_target_barrier() {
        let mut guard = Guard::default();
        guard.expect_stream(40);
        let a = Sample {
            stream_id: 40,
            source: (500, 100),
            color: [0, 0, 255],
            video_pts: 90,
        };
        let visible = guard
            .visible(
                Rect {
                    x: -900,
                    y: -100,
                    w: 160,
                    h: 160,
                },
                a,
            )
            .unwrap();
        guard.presented_after_receipt(visible, 100, 110).unwrap();
        let hide = guard.hide().unwrap();
        guard.expect_stream(3);
        guard.expect_stream(40);
        assert!(release_outcome(LEFT, guard.sample()).is_err());
        assert_eq!(
            release_outcome(RIGHT, guard.sample()),
            Ok(Outcome::Cancelled)
        );
        assert!(!guard.clean_stream_frame(40, Some(1000)).unwrap());
        assert!(guard.contaminated(Rect {
            x: -900,
            y: -100,
            w: 1,
            h: 1
        }));
        guard.presented_after_receipt(hide, 200, 230).unwrap();
        assert!(!guard.clean_stream_frame(3, Some(1000)).unwrap());
        assert!(!guard.clean_stream_frame(40, Some(230)).unwrap());
        assert!(guard.clean_stream_frame(40, Some(231)).unwrap());
        assert!(!guard.presented_after_receipt(visible, 240, 250).unwrap());
        assert_eq!(guard.sample(), None);
        let next = guard
            .visible(
                Rect {
                    x: -700,
                    y: -100,
                    w: 160,
                    h: 160,
                },
                a,
            )
            .unwrap();
        guard.presented_after_receipt(next, 260, 270).unwrap();
        let hide = guard.hide().unwrap();
        guard.expect_stream(3);
        guard.presented_after_receipt(hide, 300, 330).unwrap();
        // Returning to another output never reuses its cached pre-hide video.
        assert!(!guard.clean_stream_frame(3, Some(231)).unwrap());
        assert!(guard.clean_stream_frame(3, Some(331)).unwrap());
    }
    #[test]
    fn protected_neighborhood_and_pixels_use_target_scale_with_negative_origin() {
        let point = logical_pixel(
            (100.5, 50.5),
            (1024, 768),
            (1792, 1344),
            OutputTransform::Normal,
        )
        .unwrap();
        assert_eq!(point, PixelPoint { x: 175, y: 88 });
        let rect = source_rect(
            (-1024, -200),
            (1024, 768),
            (1792, 1344),
            point,
            OutputTransform::Normal,
            5,
        )
        .unwrap();
        assert_eq!(
            rect,
            Rect {
                x: -929,
                y: -155,
                w: 11,
                h: 11
            }
        );
        let rotated = logical_pixel(
            (100.5, 50.5),
            (1024, 768),
            (1344, 1792),
            OutputTransform::Rotate90,
        )
        .unwrap();
        assert_eq!(rotated, PixelPoint { x: 1255, y: 175 });
        assert_eq!(
            source_rect(
                (-1024, -200),
                (1024, 768),
                (1344, 1792),
                rotated,
                OutputTransform::Rotate90,
                5,
            )
            .unwrap(),
            rect
        );
        let edge = source_rect(
            (-1024, -200),
            (1024, 768),
            (1792, 1344),
            PixelPoint { x: 0, y: 0 },
            OutputTransform::Normal,
            5,
        )
        .unwrap();
        assert_eq!((edge.x, edge.y), (-1026, -202));
        for position in [(-0.1, 0.), (1024., 0.), (f64::NAN, 0.)] {
            assert!(
                logical_pixel(position, (1024, 768), (1792, 1344), OutputTransform::Normal)
                    .is_none()
            );
        }
    }
    #[test]
    fn lens_geometry_uses_actual_target_frame_at_fractional_scales() {
        for (physical, expected) in [
            ((1024, 768), (160, 160)),
            ((1280, 960), (160, 200)),
            ((1536, 1152), (160, 240)),
            ((1792, 1344), (160, 280)),
            ((2048, 1536), (160, 320)),
        ] {
            assert_eq!(
                lens_geometry((1024, 768), physical, OutputTransform::Normal, 5, 12,).unwrap(),
                expected
            );
        }
        assert!(lens_geometry((585, 439), (1024, 768), OutputTransform::Normal, 5, 12).is_ok());
        assert!(lens_geometry((1024, 768), (768, 1024), OutputTransform::Rotate90, 5, 12).is_ok());
        assert!(lens_geometry((1024, 768), (1024, 1000), OutputTransform::Normal, 5, 12).is_err());
        assert!(lens_geometry((0, 768), (1024, 768), OutputTransform::Normal, 5, 12).is_err());
        assert!(lens_geometry((1024, 768), (0, 768), OutputTransform::Normal, 5, 12).is_err());
        assert!(lens_geometry((1024, 768), (20000, 768), OutputTransform::Normal, 5, 12).is_err());
        assert!(
            lens_geometry(
                (1024, 768),
                (1024, 768),
                OutputTransform::Normal,
                u32::MAX,
                12
            )
            .is_err()
        );
        assert_eq!(
            global_rect((-1024, -200), (20, 30), (160, 160)).unwrap(),
            Rect {
                x: -1004,
                y: -170,
                w: 160,
                h: 160
            }
        );
        assert!(global_rect((i32::MAX, 0), (1, 0), (160, 160)).is_none());
    }
    #[test]
    fn pointer_output_routes_by_authorized_origin_not_sorted_stream_id() {
        let r = routes();
        assert_eq!(
            route_for_output(&r, (-1024, -100), (1024, 768)).unwrap(),
            Some(1)
        );
        assert_eq!(route_for_output(&r, (0, 0), (1024, 768)).unwrap(), Some(0));
        assert_eq!(route_for_output(&r, (1024, 0), (1024, 768)).unwrap(), None);
        assert_eq!(
            route_for_output(&r, (-1024, -100), (1000, 768)).unwrap(),
            Some(1)
        );
        let mut duplicate = r.clone();
        duplicate.push(r[1].clone());
        assert!(route_for_output(&duplicate, (-1024, -100), (1024, 768)).is_err());
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    #[test]
    fn timeout_with_observed_buttons_held_never_returns_a_successful_decision() {
        assert!(finish_outcome(Some(Ok(Outcome::Cancelled)), true).is_err());
        assert!(finish_outcome(None, false).is_err());
        assert_eq!(
            finish_outcome(Some(Ok(Outcome::Cancelled)), false).unwrap(),
            Outcome::Cancelled
        );
    }
    #[test]
    fn paired_right_release_cancels_but_waits_for_every_other_observed_button() {
        let mut pressed = BTreeSet::from([(1, 273), (1, 274)]);
        let mut decision = None;
        assert!(!consume_release(&mut pressed, &mut decision, 1, 273, None));
        assert_eq!(decision, Some(Ok(Outcome::Cancelled)));
        assert!(consume_release(&mut pressed, &mut decision, 1, 274, None));
    }
    #[test]
    fn unpaired_release_cannot_confirm_or_cancel() {
        let mut pressed = BTreeSet::new();
        let mut decision = None;
        assert!(!consume_release(&mut pressed, &mut decision, 1, 272, None));
        assert_eq!(decision, None);
    }
    #[test]
    fn left_without_presented_sample_is_error_not_cancelled_success() {
        assert!(release_outcome(272, None).is_err());
    }
    #[test]
    fn changed_empty_or_ambiguous_escape_assignment_fails_closed() {
        assert!(validate_trigger("Esc").is_ok());
        assert!(validate_trigger("Escape").is_ok());
        for trigger in ["", "Ctrl+Esc", "Alt+Escape", "unknown"] {
            assert!(validate_trigger(trigger).is_err(), "accepted {trigger}");
        }
    }

    #[test]
    fn stale_target_waits_only_during_clean_frame_transition() {
        let stale = Duration::from_secs(3);
        assert_eq!(frame_is_ready(stale, true), Ok(false));
        assert_eq!(frame_is_ready(Duration::from_millis(100), true), Ok(true));
        assert_eq!(frame_is_ready(stale, false), Err("stale capture image"));
    }

    #[test]
    fn future_feedback_does_not_authorize_a_sample_before_its_timestamp() {
        let mut guard = Guard::default();
        let sample = Sample {
            stream_id: 1,
            source: (4, 5),
            color: [1, 2, 3],
            video_pts: 50,
        };
        guard.expect_stream(1);
        let ticket = guard
            .visible(
                Rect {
                    x: 0,
                    y: 0,
                    w: 10,
                    h: 10,
                },
                sample,
            )
            .unwrap();
        let mut queue = vec![(
            Feedback {
                ticket,
                source: None,
                video_pts: Some(50),
            },
            100,
        )];
        assert!(take_due_feedback(&mut queue, 99).is_none());
        assert_eq!(guard.sample(), None);
        let (feedback, timestamp) = take_due_feedback(&mut queue, 100).unwrap();
        assert!(
            guard
                .presented_after_receipt(feedback.ticket, timestamp, 100)
                .unwrap()
        );
        assert_eq!(guard.sample(), Some(sample));
    }

    #[test]
    fn future_hide_feedback_cannot_clear_contamination_early() {
        let mut guard = Guard::default();
        let sample = Sample {
            stream_id: 1,
            source: (4, 5),
            color: [1, 2, 3],
            video_pts: 50,
        };
        guard.expect_stream(1);
        let rect = Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        guard.visible(rect, sample).unwrap();
        let hide = guard.hide().unwrap();
        let mut queue = vec![(
            Feedback {
                ticket: hide,
                source: None,
                video_pts: None,
            },
            100,
        )];
        assert!(take_due_feedback(&mut queue, 99).is_none());
        assert!(!guard.clean_stream_frame(1, Some(101)).unwrap());
        assert!(guard.contaminated(rect));
        let (feedback, timestamp) = take_due_feedback(&mut queue, 110).unwrap();
        assert!(
            guard
                .presented_after_receipt(feedback.ticket, timestamp, 110)
                .unwrap()
        );
        assert!(!guard.clean_stream_frame(1, Some(110)).unwrap());
        assert!(guard.clean_stream_frame(1, Some(111)).unwrap());
    }
}

use crate::{
    capture,
    native_guard::{Guard, Rect, Ticket},
    options::PickerOptions,
    portal,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
        protocols::wp::{
            presentation_time::client::{wp_presentation, wp_presentation_feedback},
            viewporter::client::{wp_viewport, wp_viewporter},
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};
use std::{
    collections::BTreeSet,
    error::Error,
    sync::mpsc,
    time::{Duration, Instant},
};
use swatch_board_picker_core::{
    OutputGeometry, OutputTransform, PixelPoint, Point, Rgba8, place_lens, render_magnifier,
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

fn monotonic_ns() -> Result<u64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one initialized timespec through a valid pointer.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let secs = u64::try_from(ts.tv_sec)?;
    let nanos = u64::try_from(ts.tv_nsec)?;
    if nanos >= 1_000_000_000 {
        return Err("invalid monotonic nanoseconds".into());
    }
    secs.checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .ok_or_else(|| "monotonic timestamp overflow".into())
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const LEFT: u32 = 272;
const RIGHT: u32 = 273;
macro_rules! log { ($($arg:tt)*) => {{ eprintln!($($arg)*); }} }

async fn close_after<T>(
    result: Result<T>,
    close: impl std::future::Future<Output = std::result::Result<(), String>>,
) -> Result<T> {
    match (result, close.await) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Err(error), Err(close_error)) => {
            Err(format!("{error}; session cleanup also failed: {close_error}").into())
        }
    }
}

#[cfg(test)]
mod closure_tests {
    use super::*;
    #[tokio::test]
    async fn startup_denial_still_closes_shortcut_session_and_reports_both_errors() {
        let closed = std::cell::Cell::new(false);
        let result = close_after::<()>(Err("binding denied".into()), async {
            closed.set(true);
            Err("close failed".into())
        })
        .await;
        assert!(closed.get());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("binding denied") && error.contains("close failed"));
    }
}

const SHORTCUT_ID: &str = "cancel-native-pick";

/// No normal window or keyboard object. Authorization precedes all input shields.
pub async fn pick(options: PickerOptions) -> Result<Outcome> {
    ashpd::register_host_app("org.apartment38.SwatchBoard".parse()?)
        .await
        .map_err(|error| format!("app identity registration failed: {error}; install share/applications/org.apartment38.SwatchBoard.desktop through normal packaging"))?;
    let capture = portal::begin_capture().await.map_err(|error| format!(
        "ScreenCast setup failed: {error}; check manual portal authorization and that org.apartment38.SwatchBoard.desktop is installed by normal packaging (project artifact: share/applications/org.apartment38.SwatchBoard.desktop)"
    ))?;
    eprintln!("NATIVE_STARTUP_AUTHORIZED monotonic_ns={}", monotonic_ns()?);
    let result = async {
        let shortcuts = ashpd::desktop::global_shortcuts::GlobalShortcuts::new().await.map_err(|error| format!(
            "GlobalShortcuts unavailable: {error}; this path requires portal support and the packaged org.apartment38.SwatchBoard.desktop entry"
        ))?;
        let session = shortcuts.create_session(Default::default()).await.map_err(|error| format!(
            "native GlobalShortcuts session failed: {error}; ensure org.apartment38.SwatchBoard.desktop is installed and authorize cancellation manually"
        ))?;
        // Everything fallible after create_session is inside this block. ashpd Drop
        // does not close a session, including denied/changed bind responses.
        let result = async {
            use futures_util::StreamExt;
            let encoded = zbus::zvariant::to_bytes(
                zbus::zvariant::serialized::Context::new_dbus(zbus::zvariant::LE, 0), &session,
            )?;
            let (path, _) = encoded.deserialize::<zbus::zvariant::ObjectPath<'_>>()?;
            let path = path.to_string();
            let activated = shortcuts.receive_activated().await?;
            futures_util::pin_mut!(activated);
            let shortcut = ashpd::desktop::global_shortcuts::NewShortcut::new(
                SHORTCUT_ID, "Cancel active Swatch Board native color selection",
            ).preferred_trigger(Some("Escape"));
            let response = shortcuts.bind_shortcuts(&session, &[shortcut], None, Default::default())
                .await.map_err(|error| format!("native Escape bind request failed: {error}; ensure org.apartment38.SwatchBoard.desktop is installed and authorize the requested bare Escape manually"))?
                .response().map_err(|error| format!("native Escape authorization denied/failed: {error}; no shield was created; install the packaged org.apartment38.SwatchBoard.desktop if desktop registration is required"))?;
            let bound = response.shortcuts().iter().find(|s| s.id() == SHORTCUT_ID)
                .ok_or("portal did not bind native cancellation; no input shield was created")?;
            validate_trigger(bound.trigger_description())?;
            let (stop_tx, stop_rx) = mpsc::channel();
            let mut worker = tokio::task::spawn_blocking(move || {
                native_worker(capture.pipewire_fd, capture.streams, options, stop_rx)
            });
            let result = loop {
                tokio::select! {
                    biased;
                    event = activated.next() => {
                        match event {
                            Some(event) if event.session_handle().as_str() == path && event.shortcut_id() == SHORTCUT_ID => {
                                let _ = stop_tx.send(());
                                break worker.await.map_err(|error| format!("native worker failed: {error}"))?;
                            }
                            Some(_) => continue,
                            None => {
                                let _ = stop_tx.send(());
                                let stopped = worker.await;
                                return Err(format!("global cancellation stream ended; native worker stop={stopped:?}").into());
                            }
                        }
                    }
                    result = &mut worker => break result.map_err(|error| format!("native worker failed: {error}"))?,
                }
            };
            result.map_err(Into::into)
        }.await;
        close_after(result, async {
            match session.close().await {
                Ok(()) => eprintln!("GlobalShortcuts session closed"),
                Err(error) if crate::portal::session_is_already_closed(&error) => {
                    eprintln!("GlobalShortcuts session already closed")
                }
                Err(error) => {
                    return Err(format!("GlobalShortcuts session close failed: {error}"))
                }
            }
            Ok(())
        }).await
    }.await;
    close_after(result, async {
        match capture.session.close().await {
            Ok(()) => eprintln!("ScreenCast session closed"),
            Err(error) if crate::portal::session_is_already_closed(&error) => {
                eprintln!("ScreenCast session already closed")
            }
            Err(error) => return Err(format!("ScreenCast session close failed: {error}")),
        }
        drop(capture.portal);
        Ok(())
    })
    .await
}

fn native_worker(
    fd: std::os::fd::OwnedFd,
    streams: Vec<ashpd::desktop::screencast::Stream>,
    options: PickerOptions,
    stop: mpsc::Receiver<()>,
) -> std::result::Result<Outcome, String> {
    let routes = (|| -> Result<Vec<CaptureRoute>> {
        if streams.is_empty() || streams.len() > 32 {
            return Err("authorized stream count must be 1..=32".into());
        }
        let mut ids = BTreeSet::new();
        streams
            .iter()
            .map(|s| {
                let id = s.pipe_wire_node_id();
                if !ids.insert(id) {
                    return Err("duplicate authorized stream ID".into());
                }
                let logical = s.size().ok_or("authorized stream missing logical size")?;
                if logical.0 <= 0 || logical.1 <= 0 || logical.0 > 65536 || logical.1 > 65536 {
                    return Err("unsupported authorized stream geometry".into());
                }
                Ok(CaptureRoute {
                    id,
                    origin: s.position().ok_or("authorized stream missing position")?,
                    portal_logical: logical,
                    logical,
                    display_transform: OutputTransform::Normal,
                    capture_transform: None,
                    physical: None,
                })
            })
            .collect()
    })()
    .map_err(|e| e.to_string())?;
    for route in &routes {
        eprintln!(
            "CAPTURE_ROUTE stream={} origin={:?} portal_logical={:?}",
            route.id, route.origin, route.portal_logical
        );
    }
    let feed = capture::CaptureFeed::new();
    let capture_feed = feed.clone();
    let (capture_stop, capture_rx) = mpsc::channel();
    let capture_thread = std::thread::Builder::new()
        .name("swatch-native-pipewire".into())
        .spawn(move || {
            let result = capture::run(
                fd,
                streams,
                capture_feed.clone(),
                capture_rx,
                Some(Duration::from_millis(75)),
            );
            capture_feed.close(Some(
                result
                    .as_ref()
                    .err()
                    .cloned()
                    .unwrap_or_else(|| "capture worker stopped".into()),
            ));
            result
        })
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<Outcome> {
        let startup = Instant::now();
        let _first = loop {
            match stop.try_recv() {
                Ok(()) => return Ok(Outcome::Cancelled),
                Err(mpsc::TryRecvError::Disconnected) => return Err("native coordinator disconnected".into()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            match feed.wait_for_frame(Duration::from_millis(100)) {
                Ok(first) => break first,
                Err(error) if capture_thread.is_finished() || feed.failure().is_some() || startup.elapsed() >= Duration::from_secs(10) => return Err(error.into()),
                Err(_) => {}
            }
        };
        match stop.try_recv() {
            Ok(()) => return Ok(Outcome::Cancelled),
            Err(mpsc::TryRecvError::Disconnected) => return Err("native coordinator disconnected".into()),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        eprintln!("Selection active: left release confirms the presented lens; right release/Escape cancels");
        run_overlay(feed, routes, stop, options)
    })().map_err(|e| e.to_string());
    let _ = capture_stop.send(());
    let joined = capture_thread.join();
    match (result, joined) {
        (Ok(outcome), Ok(Ok(()))) => Ok(outcome),
        (Err(error), Ok(Ok(()))) => Err(error),
        (result, joined) => Err(format!(
            "native result={result:?}; capture cleanup={joined:?}"
        )),
    }
}

struct Shield {
    output: wl_output::WlOutput,
    route: Option<usize>,
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    origin: (i32, i32),
    size: (i32, i32),
    configured: bool,
}
struct Lens {
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    configured: bool,
    buffer_side: u32,
    logical_side: u32,
}
#[derive(Clone, Debug)]
struct Feedback {
    ticket: Ticket,
    source: Option<PixelPoint>,
    video_pts: Option<u64>,
}

fn take_due_feedback(queue: &mut Vec<(Feedback, u64)>, now_ns: u64) -> Option<(Feedback, u64)> {
    queue
        .iter()
        .position(|(_, timestamp)| *timestamp <= now_ns)
        .map(|index| queue.swap_remove(index))
}
struct App {
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    shm: Shm,
    compositor: CompositorState,
    shell: LayerShell,
    viewporter: wp_viewporter::WpViewporter,
    presentation: wp_presentation::WpPresentation,
    clock_id: Option<u32>,
    shields: Vec<Shield>,
    lens: Option<Lens>,
    selected_output: Option<usize>,
    routes: Vec<CaptureRoute>,
    pointers: Vec<(wl_seat::WlSeat, wl_pointer::WlPointer)>,
    active: Option<(u32, wl_surface::WlSurface)>,
    position: (f64, f64),
    pool: SlotPool,
    shield_buffers: Vec<Buffer>,
    lens_buffers: Vec<Buffer>,
    pressed: BTreeSet<(u32, u32)>,
    selected: bool,
    outcome: Option<std::result::Result<Outcome, String>>,
    options: PickerOptions,
    guard: Guard,
    pending: Option<(u64, Instant)>,
    deferred_feedback: Vec<(Feedback, u64)>,
    hide_started: Option<Instant>,
    last_submit: Instant,
    quit: bool,
    failure: Option<String>,
}

fn run_overlay(
    feed: capture::CaptureFeed,
    routes: Vec<CaptureRoute>,
    stop: mpsc::Receiver<()>,
    options: PickerOptions,
) -> Result<Outcome> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;
    let mut app = App {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        pool: SlotPool::new(1024 * 1024, &shm)?,
        shell: LayerShell::bind(&globals, &qh)?,
        viewporter: globals.bind(&qh, 1..=1, ())?,
        presentation: globals.bind(&qh, 1..=1, ())?,
        clock_id: None,
        compositor,
        shm,
        shields: vec![],
        lens: None,
        selected_output: None,
        routes,
        pointers: vec![],
        active: None,
        position: (0., 0.),
        shield_buffers: vec![],
        lens_buffers: vec![],
        pressed: BTreeSet::new(),
        selected: false,
        outcome: None,
        options,
        guard: Guard::default(),
        pending: None,
        deferred_feedback: Vec::new(),
        hide_started: None,
        last_submit: Instant::now() - Duration::from_secs(1),
        quit: false,
        failure: None,
    };
    let result = (|| -> Result<Outcome> {
        queue.roundtrip(&mut app)?;
        queue.roundtrip(&mut app)?;
        if app.clock_id != Some(1) {
            return Err("wp_presentation must announce Linux CLOCK_MONOTONIC (1)".into());
        }
        app.create_surfaces(&qh)?;
        let mut event_loop: EventLoop<App> = EventLoop::try_new()?;
        WaylandSource::new(conn.clone(), queue).insert(event_loop.handle())?;
        let start = Instant::now();
        while !app.quit && start.elapsed() < Duration::from_secs(120) {
            match stop.try_recv() {
                Ok(()) => {
                    app.selected = true;
                    app.outcome = Some(Ok(Outcome::Cancelled));
                    if app.pressed.is_empty() {
                        app.quit = true;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("native coordinator disconnected".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            event_loop.dispatch(Duration::from_millis(16), &mut app)?;
            if app.quit {
                break;
            }
            // wait_for_frame checks terminal closure before returning cached pixels.
            let _ = feed.wait_for_frame(Duration::ZERO)?;

            app.tick(&feed, &qh)?;
            conn.flush()?;
        }
        if !app.pressed.is_empty() {
            log!(
                "WARNING bounded timeout/error with buttons held: click safety NOT established held={:?}",
                app.pressed
            );
        }
        log!(
            "END reason={} selected={} history={}",
            if app.quit {
                "release_or_error"
            } else {
                "120s_timeout"
            },
            app.selected,
            app.guard.history.len()
        );
        if let Some(error) = &app.failure {
            return Err(error.clone().into());
        }
        finish_outcome(app.outcome.take(), !app.pressed.is_empty())
    })();
    app.cleanup();
    let flushed = conn.flush();
    log!(
        "WAYLAND_CLEANUP flush={flushed:?} result={:?}",
        result.as_ref().err().map(|e| e.to_string())
    );
    match (result, flushed) {
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("Wayland cleanup flush failed: {error}").into()),
        (Err(error), Err(flush_error)) => {
            Err(format!("{error}; cleanup flush failed: {flush_error}").into())
        }
    }
}

fn paint_color_value(canvas: &mut [u8], color: Rgba8, surface_size: u32) {
    fn hex(value: u8) -> u8 {
        match value {
            0..=9 => b'0' + value,
            _ => b'A' + value - 10,
        }
    }
    let value = [
        b'#',
        hex(color.r >> 4),
        hex(color.r & 0x0f),
        hex(color.g >> 4),
        hex(color.g & 0x0f),
        hex(color.b >> 4),
        hex(color.b & 0x0f),
    ];
    let scale = 2usize;
    let advance = 4 * scale;
    let text_width = value.len() * advance;
    let Some(start_x) = (surface_size as usize)
        .checked_sub(text_width)
        .map(|width| width / 2)
    else {
        return;
    };
    let start_y = surface_size as usize - 14;
    for y in start_y.saturating_sub(2)..(start_y + 5 * scale + 2).min(surface_size as usize) {
        for x in start_x.saturating_sub(2)..(start_x + text_width + 2).min(surface_size as usize) {
            let offset = (y * surface_size as usize + x) * 4;
            canvas[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    for (index, character) in value.iter().copied().enumerate() {
        for (row, bits) in glyph(character as char).iter().enumerate() {
            for column in 0..3 {
                if bits & (1 << (2 - column)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = start_x + index * advance + column * scale + dx;
                        let y = start_y + row * scale + dy;
                        let offset = (y * surface_size as usize + x) * 4;
                        canvas[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                    }
                }
            }
        }
    }
}

fn glyph(character: char) -> [u8; 5] {
    match character {
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        _ => [0; 5],
    }
}

impl App {
    // Feedback may describe display-path latency with a timestamp later than
    // receipt. Do not authorize the sample or hide barrier before that time.
    fn accept_presentation(&mut self, data: &Feedback, ns: u64, receipt_ns: u64) {
        if self.pending.is_some_and(|(c, _)| c == data.ticket.commit) {
            self.pending = None;
        }
        match self
            .guard
            .presented_after_receipt(data.ticket, ns, receipt_ns)
        {
            Ok(true) => {
                if let Some(sample) = data.ticket.sample {
                    let c = sample.color;
                    log!(
                        "LENS_PRESENTED commit={} color=#{:02X}{:02X}{:02X} source={:?} video_pts={:?} presented_ns={ns} generation={}",
                        data.ticket.commit,
                        c[0],
                        c[1],
                        c[2],
                        data.source,
                        data.video_pts,
                        data.ticket.generation
                    );
                } else {
                    log!(
                        "GUARD_HIDE_PRESENTED barrier_ns={receipt_ns} presented_ns={ns} commit={} generation={}",
                        data.ticket.commit,
                        data.ticket.generation
                    );
                }
            }
            Ok(false) => log!(
                "PRESENTATION_IGNORED commit={} stale_generation_or_hidden=true",
                data.ticket.commit
            ),
            Err(error) => self.fail(error),
        }
    }

    fn process_deferred_feedback(&mut self, now_ns: u64) {
        while let Some((data, presented_ns)) =
            take_due_feedback(&mut self.deferred_feedback, now_ns)
        {
            self.accept_presentation(&data, presented_ns, now_ns);
            if self.quit {
                return;
            }
        }
    }

    // Called in the pointer callback as well as tick: events in one dispatch batch
    // cannot confirm a sample invalidated by motion before the next timer tick.
    fn active_output(&self) -> Option<usize> {
        let (_, surface) = self.active.as_ref()?;
        self.shields
            .iter()
            .position(|s| s.layer.wl_surface() == surface)
    }
    fn begin_hide(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        if self.guard.hiding() {
            return Ok(());
        }
        if !self.lens.as_ref().is_some_and(|l| l.configured) {
            return Err("output transition before lens configured".into());
        }
        let ticket = self.guard.hide()?;
        self.hide_started = Some(Instant::now());
        self.submit(
            qh,
            Feedback {
                ticket,
                source: None,
                video_pts: None,
            },
            None,
            None,
        )?;
        log!(
            "GUARD_HIDE_SUBMIT commit={} generation={} mapped=true transparent_buffer=true",
            ticket.commit,
            ticket.generation
        );
        Ok(())
    }
    fn invalidate_pointer_source(&mut self, qh: &QueueHandle<Self>) {
        if self.selected {
            return;
        }
        let result = (|| -> Result<()> {
            let Some(index) = self.active_output() else {
                if self.active.is_some() {
                    return Err("pointer on unknown shield".into());
                }
                return Ok(());
            };
            let shield = &self.shields[index];
            let route = &self.routes[shield.route.ok_or("pointer is on unauthorized output")?];
            self.guard.expect_stream(route.id);
            if self.selected_output.is_some_and(|old| old != index) {
                return self.begin_hide(qh);
            }
            if self.guard.hiding() || self.guard.history.is_empty() {
                return Ok(());
            }
            let physical = route.physical.ok_or("missing active capture geometry")?;
            let transform = route
                .capture_transform
                .ok_or("missing active capture transform")?;
            let point = logical_pixel(self.position, shield.size, physical, transform)
                .ok_or("invalid shield pointer position")?;
            let protected = source_rect(
                shield.origin,
                shield.size,
                physical,
                point,
                transform,
                self.options.radius,
            )
            .ok_or("protected source geometry overflow")?;
            if self.guard.contaminated(protected) {
                self.begin_hide(qh)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(error.to_string());
        }
    }
    fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        log!("FAIL_CLOSED {message}");
        self.failure = Some(message);
        self.quit = true;
    }
    fn create_surfaces(&mut self, qh: &QueueHandle<Self>) -> Result<()> {
        let outputs: Vec<_> = self.outputs.outputs().collect();
        if outputs.len() > 32 {
            return Err("more than 32 outputs unsupported".into());
        }
        if outputs.is_empty() {
            return Err("no outputs".into());
        }
        let mut matches = BTreeSet::new();
        for output in outputs {
            let info = self.outputs.info(&output).ok_or("missing output info")?;
            let origin = info
                .logical_position
                .ok_or("logical output origin required")?;
            let size = info
                .logical_size
                .filter(|s| s.0 > 0 && s.1 > 0)
                .ok_or("logical output size required")?;
            if !(-1_000_000..=1_000_000).contains(&origin.0)
                || !(-1_000_000..=1_000_000).contains(&origin.1)
                || size.0 > 65536
                || size.1 > 65536
            {
                return Err("unsupported output coordinate bounds".into());
            }
            let route = route_for_output(&self.routes, origin, size)?;
            if let Some(route) = route {
                if !matches.insert(route) {
                    return Err("authorized stream matches ambiguous outputs".into());
                }
                self.routes[route].logical = size;
                self.routes[route].display_transform = output_transform(info.transform);
            }
            log!(
                "OUTPUT name={:?} origin={origin:?} logical={size:?} transform={:?}",
                info.name,
                info.transform
            );
            let surface = self.compositor.create_surface(qh);
            let viewport = self.viewporter.get_viewport(&surface, qh, ());
            viewport.set_destination(size.0, size.1);
            let layer = self.shell.create_layer_surface(
                qh,
                surface,
                Layer::Overlay,
                Some("swatch-board-native-shield"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_size(size.0 as u32, size.1 as u32);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.wl_surface().set_input_region(None); // NULL = full infinite input region.
            layer.commit();
            self.shields.push(Shield {
                output,
                route,
                layer,
                viewport,
                origin,
                size,
                configured: false,
            });
        }
        if matches.len() != self.routes.len() {
            return Err("authorized stream missing matching Wayland output".into());
        }
        Ok(())
    }
    fn create_lens(
        &mut self,
        qh: &QueueHandle<Self>,
        index: usize,
        physical: (u32, u32),
    ) -> Result<()> {
        let s = &self.shields[index];
        let transform = self.routes[s.route.ok_or("lens output is unauthorized")?]
            .capture_transform
            .ok_or("lens capture transform is unavailable")?;
        let (logical_side, buffer_side) = lens_geometry(
            s.size,
            physical,
            transform,
            self.options.radius,
            self.options.zoom,
        )?;
        let surface = self.compositor.create_surface(qh);
        let viewport = self.viewporter.get_viewport(&surface, qh, ());
        viewport.set_destination(logical_side as i32, logical_side as i32);
        let layer = self.shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("swatch-board-native-lens"),
            Some(&s.output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_size(logical_side, logical_side);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let empty = Region::new(&self.compositor)?;
        layer.wl_surface().set_input_region(Some(empty.wl_region()));
        layer.commit();
        self.selected_output = Some(index);
        self.lens = Some(Lens {
            layer,
            viewport,
            configured: false,
            buffer_side,
            logical_side,
        });
        log!(
            "LENS_REQUESTED normal_layer=true cursor_role=false keyboard=None empty_input=true buffer={buffer_side}"
        );
        Ok(())
    }
    fn tick(&mut self, feed: &capture::CaptureFeed, qh: &QueueHandle<Self>) -> Result<()> {
        if !self.deferred_feedback.is_empty() {
            self.process_deferred_feedback(monotonic_ns()?);
            if self.quit {
                return Ok(());
            }
        }
        if self
            .pending
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(2))
        {
            return Err("presentation feedback timeout".into());
        }
        if self
            .hide_started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(3))
        {
            return Err("post-hide clean video timeout".into());
        }
        if !self.shields.iter().all(|s| s.configured) || self.selected {
            return Ok(());
        }
        let frames = feed.snapshot();
        // Check every received frame against portal authorization; never choose by vector order.
        for f in &frames {
            let route = self
                .routes
                .iter_mut()
                .find(|r| r.id == f.stream_id)
                .ok_or("unauthorized capture frame")?;
            if f.stream_position != route.origin || f.stream_logical_size != route.portal_logical {
                return Err("capture geometry/identity changed".into());
            }
            let physical = (f.frame.width, f.frame.height);
            let transform = choose_capture_transform(
                route.logical,
                physical,
                route.display_transform,
                self.options.radius,
                self.options.zoom,
            )?;
            if route.capture_transform.is_some_and(|old| old != transform) {
                return Err("capture orientation changed".into());
            }
            route.capture_transform = Some(transform);
            if route.physical.is_some_and(|old| old != physical) {
                return Err("capture dimensions changed".into());
            }
            route.physical = Some(physical);
        }
        let Some(index) = self.active_output() else {
            return Ok(());
        }; // NEVER cursor metadata fallback.
        let route_index = self.shields[index]
            .route
            .ok_or("pointer is on unauthorized output")?;
        let id = self.routes[route_index].id;
        self.guard.expect_stream(id);
        if self.selected_output.is_some_and(|old| old != index) && !self.guard.hiding() {
            self.begin_hide(qh)?;
            return Ok(());
        }
        let Some(f) = frames.iter().find(|f| f.stream_id == id) else {
            // No pixels for this authorized target yet; no stale other-output fallback.
            return Ok(());
        };
        let pts = f
            .presentation_time_ns
            .ok_or("video PTS missing; fail closed")?;
        if pts > monotonic_ns()? {
            return Err("video PTS exceeds local CLOCK_MONOTONIC; incompatible clock".into());
        }
        let age = f
            .received_at
            .elapsed()
            .map_err(|_| "capture image receipt is in the future")?;
        if !frame_is_ready(age, self.guard.hiding())? {
            return Ok(());
        }
        if self.guard.hiding() {
            if !self.guard.clean_stream_frame(id, Some(pts))? {
                return Ok(());
            }
            log!("GUARD_CLEAN_FRAME pts_ns={pts} stream={id}");
            self.hide_started = None;
            if self.selected_output != Some(index) {
                // Old transparent commit was genuinely presented before this target video.
                // Only now may the old output-bound surface be destroyed/replaced.
                self.destroy_lens();
            }
        }
        if self.lens.is_none() {
            self.create_lens(qh, index, (f.frame.width, f.frame.height))?;
            return Ok(());
        }
        if !self.lens.as_ref().is_some_and(|l| l.configured) {
            return Ok(());
        }
        let s = &self.shields[index];
        let transform = self.routes[route_index]
            .capture_transform
            .ok_or("capture transform is unavailable")?;
        let physical = (f.frame.width, f.frame.height);
        let point = logical_pixel(self.position, s.size, physical, transform)
            .ok_or("invalid shield pointer position")?;
        let (raw_w, raw_h) = raw_dimensions(physical, transform);
        let sx = s.size.0 as f64 / raw_w as f64;
        let sy = s.size.1 as f64 / raw_h as f64;
        let protected = source_rect(
            s.origin,
            s.size,
            physical,
            point,
            transform,
            self.options.radius,
        )
        .ok_or("protected source geometry overflow")?;
        if self.guard.contaminated(protected) {
            self.begin_hide(qh)?;
            return Ok(());
        }
        if self.pending.is_some() || self.last_submit.elapsed() < Duration::from_millis(100) {
            return Ok(());
        }
        let logical_point = PixelPoint {
            x: self.position.0.floor() as u32,
            y: self.position.1.floor() as u32,
        };
        let side = self.lens.as_ref().ok_or("lens missing")?.logical_side;
        let gap = (self.options.radius as f64 * sx.max(sy)).ceil() as u32 + 8;
        let pos = place_lens(
            (s.size.0 as u32, s.size.1 as u32),
            logical_point,
            (side, side),
            gap,
        )
        .ok_or("cannot place lens outside source neighborhood")?;
        let rect = global_rect(
            s.origin,
            (i32::try_from(pos.x)?, i32::try_from(pos.y)?),
            (i32::try_from(side)?, i32::try_from(side)?),
        )
        .ok_or("lens global geometry overflow")?;
        if rect.intersects(protected) {
            return Err("new lens intersects protected neighborhood".into());
        }
        let color = f.frame.pixel(point).ok_or("source pixel missing")?;
        let color = [color.r, color.g, color.b];
        if self.options.diagnostics {
            eprintln!(
                "NATIVE_DIAGNOSTIC stream={} source=({},{}) color={color:?} frame_age_ms={:?} cursor_age_ms={:?} video_pts={pts}",
                f.stream_id,
                point.x,
                point.y,
                f.received_at.elapsed().ok().map(|d| d.as_millis()),
                f.cursor_updated_at
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_millis())
            );
        }
        let magnifier = render_magnifier(&f.frame, point, self.options.radius, self.options.zoom)
            .ok_or("magnifier core rejected source")?;
        let ticket = self.guard.visible(
            rect,
            Sample {
                stream_id: f.stream_id,
                source: (point.x, point.y),
                color,
                video_pts: pts,
            },
        )?;
        self.submit(
            qh,
            Feedback {
                ticket,
                source: Some(point),
                video_pts: Some(pts),
            },
            Some(pos),
            Some(&magnifier),
        )?;
        self.last_submit = Instant::now();
        log!(
            "LENS_SUBMIT commit={} rect=({},{},{},{}) source=({},{}) color=#{:02X}{:02X}{:02X} pts_ns={} generation={}",
            ticket.commit,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            point.x,
            point.y,
            color[0],
            color[1],
            color[2],
            pts,
            ticket.generation
        );
        Ok(())
    }
    fn submit(
        &mut self,
        qh: &QueueHandle<Self>,
        data: Feedback,
        pos: Option<PixelPoint>,
        magnifier: Option<&swatch_board_picker_core::Magnifier>,
    ) -> Result<()> {
        let lens = self.lens.as_ref().ok_or("lens missing")?;
        // Released buffers can be dropped; active slots are never overwritten.
        self.lens_buffers
            .retain(|b| b.canvas(&mut self.pool).is_none());
        if self.lens_buffers.len() >= 16 {
            return Err("active lens buffer bound exhausted".into());
        }
        let side = lens.buffer_side as usize;
        let (buffer, canvas) = self.pool.create_buffer(
            side as i32,
            side as i32,
            (side * 4) as i32,
            wl_shm::Format::Argb8888,
        )?;
        canvas.fill(0);
        if let Some(m) = magnifier {
            // Opaque background; real core pixels are drawn without interpolation in buffer space.
            for pixel in canvas.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[24, 24, 24, 255]);
            }
            let ox = (side - m.width as usize) / 2;
            let oy = (side - m.height as usize) / 2;
            for y in 0..m.height as usize {
                for x in 0..m.width as usize {
                    let c = m.pixels[y * m.width as usize + x];
                    let i = ((y + oy) * side + x + ox) * 4;
                    canvas[i..i + 4].copy_from_slice(&[c.b, c.g, c.r, 255]);
                }
            }
            // Thin center-pixel outline; do not overwrite its actual center color.
            let cx = ox + self.options.radius as usize * self.options.zoom as usize;
            let cy = oy + self.options.radius as usize * self.options.zoom as usize;
            let zoom = self.options.zoom as usize;
            for y in 0..zoom {
                for x in 0..zoom {
                    let edge = x.min(y).min(zoom - 1 - x).min(zoom - 1 - y);
                    let level = match edge {
                        0 => 255,
                        1 if zoom >= 6 => 0,
                        _ => continue,
                    };
                    let i = ((cy + y) * side + cx + x) * 4;
                    canvas[i..i + 4].copy_from_slice(&[level, level, level, 255]);
                }
            }
            let center = m.pixels[m.center.y as usize * m.width as usize + m.center.x as usize];
            paint_color_value(canvas, center, lens.buffer_side);
        }
        if let Some(p) = pos {
            lens.layer.set_margin(p.y as i32, 0, 0, p.x as i32);
        }
        buffer.attach_to(lens.layer.wl_surface())?;
        lens.layer
            .wl_surface()
            .damage_buffer(0, 0, side as i32, side as i32);
        self.presentation
            .feedback(lens.layer.wl_surface(), qh, data.clone());
        lens.layer.commit();
        self.pending = Some((data.ticket.commit, Instant::now()));
        self.lens_buffers.push(buffer);
        Ok(())
    }
    fn destroy_lens(&mut self) {
        if let Some(lens) = self.lens.take() {
            lens.layer.wl_surface().attach(None, 0, 0);
            lens.layer.commit();
            lens.viewport.destroy();
            lens.layer.wl_surface().destroy();
        }
        self.selected_output = None;
    }
    fn cleanup(&mut self) {
        if !self.pressed.is_empty() {
            log!(
                "WARNING CLEANUP held_buttons={:?}; bounded exit is not proof of click safety",
                self.pressed
            );
        }
        for (_, p) in self.pointers.drain(..) {
            if p.version() >= 3 {
                p.release();
            }
        }
        self.destroy_lens();
        let count = self.shields.len();
        for s in self.shields.drain(..) {
            s.layer.wl_surface().attach(None, 0, 0);
            s.layer.commit();
            s.viewport.destroy();
            s.layer.wl_surface().destroy();
        }
        self.shield_buffers.clear();
        self.lens_buffers.clear();
        log!(
            "CLEANUP shields={count} lens_destroyed=true pointers_released=true keyboard_objects=0"
        );
    }
}

fn logical_pixel(
    p: (f64, f64),
    logical: (i32, i32),
    physical: (u32, u32),
    transform: OutputTransform,
) -> Option<PixelPoint> {
    if logical.0 <= 0 || logical.1 <= 0 {
        return None;
    }
    let (raw_w, raw_h) = raw_dimensions(physical, transform);
    let sx = raw_w as f64 / logical.0 as f64;
    let sy = raw_h as f64 / logical.1 as f64;
    let tolerance = 1. / logical.0 as f64 + 1. / logical.1 as f64;
    if (sx - sy).abs() > tolerance {
        return None;
    }
    OutputGeometry {
        logical_origin: Point { x: 0., y: 0. },
        logical_size: Point {
            x: logical.0 as f64,
            y: logical.1 as f64,
        },
        scale: sx.max(sy),
        stream_width: physical.0,
        stream_height: physical.1,
        transform,
    }
    .desktop_to_stream(Point { x: p.0, y: p.1 })
}
impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let id = pointer.id().protocol_id();
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                if app.active.as_ref().is_some_and(|(other, _)| *other != id) {
                    app.fail("multiple active pointers unsupported");
                    return;
                }
                app.active = Some((id, surface));
                app.position = (surface_x, surface_y);
                app.invalidate_pointer_source(qh);
                log!("POINTER_ENTER id={id} local=({surface_x},{surface_y}) source=wl_pointer");
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if app.active.as_ref().is_some_and(|(active, _)| *active == id) {
                    app.position = (surface_x, surface_y);
                    app.invalidate_pointer_source(qh);
                }
            }
            wl_pointer::Event::Leave { .. } => {
                if app.active.as_ref().is_some_and(|(active, _)| *active == id) {
                    app.active = None;
                }
            }
            wl_pointer::Event::Button { button, state, .. } => {
                let key = (id, button);
                match state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        app.pressed.insert(key);
                    }
                    WEnum::Value(wl_pointer::ButtonState::Released) => {
                        if button == LEFT {
                            app.invalidate_pointer_source(qh);
                        }
                        let sample = app
                            .active
                            .as_ref()
                            .filter(|(active_id, surf)| {
                                *active_id == id
                                    && app
                                        .selected_output
                                        .is_some_and(|i| surf == app.shields[i].layer.wl_surface())
                                    && !app.guard.hiding()
                            })
                            .and_then(|_| app.guard.sample());
                        // Never destroy on Press; consume all observed paired releases.
                        app.quit |=
                            consume_release(&mut app.pressed, &mut app.outcome, id, button, sample);
                        app.selected = app.outcome.is_some();
                        log!(
                            "NATIVE_RELEASE button={button} outcome={:?} held={:?}",
                            app.outcome,
                            app.pressed
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<wp_presentation::WpPresentation, ()> for App {
    fn event(
        app: &mut Self,
        _: &wp_presentation::WpPresentation,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            app.clock_id = Some(clk_id);
            log!("PRESENTATION_CLOCK clock_id={clk_id} required_CLOCK_MONOTONIC=1");
            if clk_id != 1 {
                app.fail("incomparable presentation clock");
            }
        }
    }
}
impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, Feedback> for App {
    fn event(
        app: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        data: &Feedback,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                ..
            } => {
                if app.clock_id != Some(1) {
                    app.fail("presentation clock unverified");
                    return;
                }
                let seconds = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
                let Some(ns) = seconds
                    .checked_mul(1_000_000_000)
                    .and_then(|s| s.checked_add(tv_nsec as u64))
                else {
                    app.fail("presentation timestamp overflow");
                    return;
                };
                if tv_nsec >= 1_000_000_000 {
                    app.fail("invalid presentation nanoseconds");
                    return;
                }
                let receipt_ns = match monotonic_ns() {
                    Ok(t) => t,
                    Err(e) => {
                        app.fail(e.to_string());
                        return;
                    }
                };
                if ns > receipt_ns {
                    // Keep the existing two-second feedback bound, and never
                    // authorize a timestamp that is still in the future.
                    if ns - receipt_ns > Duration::from_secs(2).as_nanos() as u64
                        || app.deferred_feedback.len() >= 128
                    {
                        app.fail("presentation timestamp too far in the future");
                        return;
                    }
                    log!(
                        "PRESENTATION_DEFERRED commit={} delta_ns={}",
                        data.ticket.commit,
                        ns - receipt_ns
                    );
                    app.deferred_feedback.push((data.clone(), ns));
                    return;
                }
                app.accept_presentation(data, ns, receipt_ns);
            }
            wp_presentation_feedback::Event::Discarded => {
                if app.pending.is_some_and(|(c, _)| c == data.ticket.commit) {
                    app.pending = None;
                }
                log!(
                    "PRESENTATION_DISCARDED commit={} hidden={}",
                    data.ticket.commit,
                    data.ticket.sample.is_none()
                );
                // A discarded hide is never sufficient to clear contamination.
                if data.ticket.sample.is_none() {
                    app.fail("hide presentation discarded");
                }
            }
            _ => {}
        }
    }
}
impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer {
            let p = seat.get_pointer(qh, ());
            self.pointers.push((seat, p));
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer {
            self.fail("pointer capability removed");
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        self.fail("seat removed");
    }
}
impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.fail("layer closed");
    }
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(lens) = self.lens.as_mut().filter(|l| l.layer == *layer) {
            if (c.new_size.0 != 0 && c.new_size.0 != lens.logical_side)
                || (c.new_size.1 != 0 && c.new_size.1 != lens.logical_side)
            {
                self.fail("lens configure geometry mismatch");
            } else {
                lens.configured = true;
            }
            return;
        }
        let Some(index) = self.shields.iter().position(|s| s.layer == *layer) else {
            self.fail("unknown configured layer");
            return;
        };
        let s = &self.shields[index];
        if (c.new_size.0 != 0 && c.new_size.0 != s.size.0 as u32)
            || (c.new_size.1 != 0 && c.new_size.1 != s.size.1 as u32)
        {
            self.fail("shield does not cover output");
            return;
        }
        if s.configured {
            return;
        }
        match self.pool.create_buffer(1, 1, 4, wl_shm::Format::Argb8888) {
            Ok((b, canvas)) => {
                canvas.fill(0);
                if let Err(e) = b.attach_to(layer.wl_surface()) {
                    self.fail(e.to_string());
                    return;
                }
                layer.wl_surface().damage_buffer(0, 0, 1, 1);
                layer.commit();
                self.shield_buffers.push(b);
                self.shields[index].configured = true;
                log!("SHIELD_MAPPED output={index} transparent=true full_input=true keyboard=None");
            }
            Err(e) => self.fail(e.to_string()),
        }
    }
}
impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.fail("output added");
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if !self.shields.is_empty() {
            self.fail("output changed");
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.fail("output removed");
    }
}
impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {} // NEVER presentation authority.
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}
impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}
delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);
wayland_client::delegate_noop!(App: ignore wp_viewporter::WpViewporter);
wayland_client::delegate_noop!(App: ignore wp_viewport::WpViewport);
