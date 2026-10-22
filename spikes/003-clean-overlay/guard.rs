//! Pure self-occlusion and genuine-presentation authority for the isolated probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub fn intersects(self, b: Self) -> bool {
        self.x < b.x + b.w && b.x < self.x + self.w && self.y < b.y + b.h && b.y < self.y + self.h
    }
}
#[derive(Default)]
pub struct Guard {
    pub history: Vec<Rect>,
    generation: u64,
    commit: u64,
    hidden: Option<(Ticket, Option<u64>)>,
    displayed: Option<(u64, [u8; 3])>,
}
impl Guard {
    pub fn contaminated(&self, source: Rect) -> bool {
        self.history.iter().any(|r| r.intersects(source))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub commit: u64,
    pub generation: u64,
    pub color: Option<[u8; 3]>,
}
impl Guard {
    pub fn visible(&mut self, rect: Rect, color: [u8; 3]) -> Result<Ticket, &'static str> {
        if self.hidden.is_some() {
            return Err("hide barrier pending");
        }
        if self.history.len() >= 128 {
            return Err("guard history exhausted");
        }
        self.commit += 1;
        self.history.push(rect);
        Ok(Ticket {
            commit: self.commit,
            generation: self.generation,
            color: Some(color),
        })
    }
    pub fn hide(&mut self) -> Result<Ticket, &'static str> {
        if self.hidden.is_some() {
            return Err("duplicate hide");
        }
        self.commit += 1;
        self.generation += 1;
        self.displayed = None;
        let t = Ticket {
            commit: self.commit,
            generation: self.generation,
            color: None,
        };
        self.hidden = Some((t, None));
        Ok(t)
    }
    pub fn presented(&mut self, t: Ticket, ns: u64) -> Result<bool, &'static str> {
        if t.generation != self.generation {
            return Ok(false);
        }
        if let Some((hidden, barrier)) = &mut self.hidden {
            if *hidden == t {
                *barrier = Some(ns);
                return Ok(true);
            }
            return Ok(false);
        }
        if let Some(color) = t.color {
            if self.displayed.is_none_or(|(c, _)| t.commit > c) {
                self.displayed = Some((t.commit, color));
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn presented_after_receipt(
        &mut self,
        t: Ticket,
        ns: u64,
        receipt_ns: u64,
    ) -> Result<bool, &'static str> {
        if receipt_ns < ns {
            return Err("presentation timestamp exceeds local monotonic receipt time");
        }
        self.presented(t, if t.color.is_none() { receipt_ns } else { ns })
    }
    pub fn clean_frame(&mut self, pts: Option<u64>) -> Result<bool, &'static str> {
        let Some((_, Some(barrier))) = self.hidden else {
            return Ok(false);
        };
        let pts = pts.ok_or("missing video PTS")?;
        if pts > barrier {
            self.history.clear();
            self.hidden = None;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn sample(&self) -> Option<[u8; 3]> {
        self.displayed.map(|(_, c)| c)
    }
    pub fn hiding(&self) -> bool {
        self.hidden.is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn r(x: i32) -> Rect {
        Rect {
            x,
            y: 0,
            w: 10,
            h: 10,
        }
    }
    #[test]
    fn hidden_barrier_requires_presented_hide_then_strictly_new_video() {
        let mut g = Guard::default();
        let v = g.visible(r(0), [1, 2, 3]).unwrap();
        assert_eq!(g.sample(), None, "queued is not displayed");
        assert!(g.presented(v, 50).unwrap());
        assert_eq!(g.sample(), Some([1, 2, 3]));
        let h = g.hide().unwrap();
        assert_eq!(g.sample(), None);
        assert!(
            !g.presented(v, 60).unwrap(),
            "late visible feedback must not resurrect sample"
        );
        assert!(g.visible(r(100), [4, 5, 6]).is_err());
        assert!(
            !g.clean_frame(Some(1000)).unwrap(),
            "PTS alone cannot prove hide"
        );
        assert!(g.presented(h, 100).unwrap());
        assert!(!g.clean_frame(Some(100)).unwrap());
        assert!(!g.clean_frame(Some(99)).unwrap());
        assert!(g.contaminated(r(0)));
        assert!(g.clean_frame(Some(101)).unwrap());
        assert!(!g.contaminated(r(0)));
        assert!(!g.presented(v, 102).unwrap());
        let next = g.visible(r(100), [4, 5, 6]).unwrap();
        assert!(g.presented(next, 103).unwrap());
        assert_eq!(g.sample(), Some([4, 5, 6]));
    }
    #[test]
    fn history_exhaustion_never_evicts_old_contamination() {
        let mut g = Guard::default();
        for _ in 0..128 {
            g.visible(r(0), [1, 2, 3]).unwrap();
        }
        assert!(g.visible(r(100), [1, 2, 3]).is_err());
        assert_eq!(g.history.len(), 128);
        assert!(g.contaminated(r(0)));
    }
    #[test]
    fn missing_pts_is_fail_closed() {
        let mut g = Guard::default();
        let h = g.hide().unwrap();
        g.presented(h, 100).unwrap();
        assert!(g.clean_frame(None).is_err());
        assert!(g.hiding());
    }
    #[test]
    fn receipt_cutoff_rejects_video_queued_after_presentation_but_before_receipt() {
        let mut g = Guard::default();
        let h = g.hide().unwrap();
        assert!(g.presented_after_receipt(h, 100, 120).unwrap());
        assert!(!g.clean_frame(Some(110)).unwrap());
        assert!(!g.clean_frame(Some(120)).unwrap());
        assert!(g.clean_frame(Some(121)).unwrap());
    }
    #[test]
    fn every_submitted_rectangle_remains_contaminating() {
        let g = Guard {
            history: vec![r(0), r(100)],
            ..Guard::default()
        };
        assert!(
            g.contaminated(r(0)),
            "old submitted lens remains unsafe even if never presented"
        );
        assert!(g.contaminated(r(100)));
        assert!(!g.contaminated(r(200)));
    }
}
