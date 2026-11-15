//! Fail-closed authority for compositor-presented native magnifier commits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub fn intersects(self, b: Self) -> bool {
        i64::from(self.x) < i64::from(b.x) + i64::from(b.w)
            && i64::from(b.x) < i64::from(self.x) + i64::from(self.w)
            && i64::from(self.y) < i64::from(b.y) + i64::from(b.h)
            && i64::from(b.y) < i64::from(self.y) + i64::from(self.h)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub stream_id: u32,
    pub source: (u32, u32),
    pub color: [u8; 3],
    pub video_pts: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub commit: u64,
    pub generation: u64,
    pub sample: Option<Sample>,
}
#[derive(Default)]
pub struct Guard {
    pub history: Vec<Rect>,
    generation: u64,
    commit: u64,
    hidden: Option<(Ticket, Option<u64>)>,
    displayed: Option<(u64, Sample)>,
    expected_stream: Option<u32>,
}
impl Guard {
    // Retargeting never acknowledges a hide or discards contamination history.
    pub fn expect_stream(&mut self, stream_id: u32) {
        if self.expected_stream != Some(stream_id) {
            self.displayed = None;
            self.expected_stream = Some(stream_id);
        }
    }
    pub fn clean_stream_frame(
        &mut self,
        stream_id: u32,
        pts: Option<u64>,
    ) -> Result<bool, &'static str> {
        if self.expected_stream != Some(stream_id) {
            return Ok(false);
        }
        self.clean_frame(pts)
    }
    pub fn visible(&mut self, rect: Rect, sample: Sample) -> Result<Ticket, &'static str> {
        if self
            .expected_stream
            .is_some_and(|id| id != sample.stream_id)
        {
            return Err("sample is not from active authorized stream");
        }
        if self.hidden.is_some() {
            return Err("hide barrier pending");
        }
        // Identical rectangles cover exactly the same contamination; never evict
        // a distinct submitted footprint, including commits never presented.
        if !self.history.contains(&rect) {
            if self.history.len() >= 128 {
                return Err("submitted rectangle history exhausted");
            }
            self.history.push(rect);
        }
        self.commit = self.commit.checked_add(1).ok_or("commit overflow")?;
        Ok(Ticket {
            commit: self.commit,
            generation: self.generation,
            sample: Some(sample),
        })
    }
    pub fn presented_after_receipt(
        &mut self,
        ticket: Ticket,
        ns: u64,
        receipt: u64,
    ) -> Result<bool, &'static str> {
        if receipt < ns {
            return Err("presentation timestamp exceeds monotonic receipt");
        }
        if ticket.generation != self.generation {
            return Ok(false);
        }
        if let Some((hidden, cutoff)) = &mut self.hidden {
            if *hidden == ticket {
                *cutoff = Some(receipt);
                return Ok(true);
            }
            return Ok(false);
        }
        if let Some(sample) = ticket.sample {
            if self
                .expected_stream
                .is_some_and(|id| id != sample.stream_id)
            {
                return Ok(false);
            }
            if self
                .displayed
                .is_none_or(|(commit, _)| ticket.commit > commit)
            {
                self.displayed = Some((ticket.commit, sample));
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn hide(&mut self) -> Result<Ticket, &'static str> {
        if self.hidden.is_some() {
            return Err("duplicate hide");
        }
        self.displayed = None;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("generation overflow")?;
        self.commit = self.commit.checked_add(1).ok_or("commit overflow")?;
        let ticket = Ticket {
            commit: self.commit,
            generation: self.generation,
            sample: None,
        };
        self.hidden = Some((ticket, None));
        Ok(ticket)
    }
    pub fn clean_frame(&mut self, pts: Option<u64>) -> Result<bool, &'static str> {
        let Some((_, Some(cutoff))) = self.hidden else {
            return Ok(false);
        };
        if pts.ok_or("missing video PTS")? <= cutoff {
            return Ok(false);
        }
        self.history.clear();
        self.hidden = None;
        Ok(true)
    }
    pub fn hiding(&self) -> bool {
        self.hidden.is_some()
    }
    pub fn contaminated(&self, source: Rect) -> bool {
        self.history.iter().any(|r| r.intersects(source))
    }
    pub fn sample(&self) -> Option<Sample> {
        self.displayed.map(|(_, s)| s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: i32) -> Rect {
        Rect {
            x,
            y: 0,
            w: 10,
            h: 10,
        }
    }
    fn sample() -> Sample {
        Sample {
            stream_id: 7,
            source: (4, 5),
            color: [1, 2, 3],
            video_pts: 9,
        }
    }
    #[test]
    fn hide_invalidates_late_feedback_and_requires_strict_post_receipt_video() {
        let mut g = Guard::default();
        let old = g.visible(rect(0), sample()).unwrap();
        g.presented_after_receipt(old, 10, 11).unwrap();
        let queued = g.visible(rect(100), sample()).unwrap();
        assert!(g.contaminated(rect(0)) && g.contaminated(rect(100)));
        let hide = g.hide().unwrap();
        assert!(g.hiding());
        assert_eq!(g.sample(), None);
        assert!(!g.presented_after_receipt(queued, 12, 13).unwrap());
        assert!(g.visible(rect(200), sample()).is_err());
        assert!(!g.clean_frame(Some(500)).unwrap());
        g.presented_after_receipt(hide, 100, 120).unwrap();
        assert!(g.clean_frame(None).is_err());
        assert!(!g.clean_frame(Some(110)).unwrap());
        assert!(!g.clean_frame(Some(120)).unwrap());
        assert!(g.clean_frame(Some(121)).unwrap());
        assert!(!g.contaminated(rect(0)));
        assert!(!g.presented_after_receipt(old, 122, 123).unwrap());
        assert_eq!(g.sample(), None);
    }
    #[test]
    fn retarget_cannot_reauthorize_old_stream_feedback_even_before_hide_submission() {
        let mut g = Guard::default();
        g.expect_stream(7);
        let old = g.visible(rect(0), sample()).unwrap();
        g.expect_stream(42);
        assert!(!g.presented_after_receipt(old, 10, 11).unwrap());
        assert_eq!(g.sample(), None);
    }
    #[test]
    fn output_switch_requires_target_stream_video_not_other_stream_or_metadata() {
        let mut g = Guard::default();
        let old = g.visible(rect(0), sample()).unwrap();
        g.presented_after_receipt(old, 10, 11).unwrap();
        let hide = g.hide().unwrap();
        g.expect_stream(42);
        g.presented_after_receipt(hide, 100, 120).unwrap();
        assert!(!g.clean_stream_frame(7, Some(200)).unwrap());
        assert!(!g.clean_stream_frame(42, Some(120)).unwrap());
        assert!(g.contaminated(rect(0)));
        assert!(g.clean_stream_frame(42, Some(121)).unwrap());
        assert!(!g.presented_after_receipt(old, 130, 140).unwrap());
        assert!(g.visible(rect(0), sample()).is_err());
        assert_eq!(g.sample(), None);
        let target = Sample {
            stream_id: 42,
            ..sample()
        };
        let ticket = g.visible(rect(100), target).unwrap();
        g.presented_after_receipt(ticket, 150, 160).unwrap();
        assert_eq!(g.sample(), Some(target));
    }
    #[test]
    fn repeated_identical_footprint_does_not_exhaust_history() {
        let mut g = Guard::default();
        for _ in 0..1000 {
            g.visible(rect(0), sample()).unwrap();
        }
        assert_eq!(g.history.len(), 1);
        assert!(g.contaminated(rect(0)));
    }
    #[test]
    fn history_never_evicts_unpresented_rectangles_at_bound() {
        let mut g = Guard::default();
        for x in 0..128 {
            g.visible(rect(x * 20), sample()).unwrap();
        }
        assert!(g.visible(rect(5000), sample()).is_err());
        assert!(g.contaminated(rect(0)));
        assert_eq!(g.history.len(), 128);
    }
    #[test]
    fn only_genuine_presented_commit_authorizes_exact_sample_identity() {
        let mut guard = Guard::default();
        let ticket = guard.visible(rect(0), sample()).unwrap();
        assert_eq!(guard.sample(), None);
        assert!(guard.presented_after_receipt(ticket, 100, 120).unwrap());
        assert_eq!(guard.sample(), Some(sample()));
    }
}
