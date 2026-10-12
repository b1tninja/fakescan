//! Ordered tester scenarios: one pattern + knobs + what to check.

use serde::Serialize;

use crate::image_gen::PatternId;

/// One step in the recommended scan-tool smoke progression.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Scenario {
    /// Stable id for `POST /v1/scenario` (`order`, `qr`, …).
    pub id: &'static str,
    /// 1-based step in [`PROGRESSION`].
    pub step: u8,
    pub of: u8,
    pub pattern: PatternId,
    /// Short banner text drawn on the page (human-visible).
    pub title: &'static str,
    /// What the tester should verify after acquire.
    pub check: &'static str,
    /// Feeder page count to preload (0 = glass-friendly empty ADF).
    pub pages: u32,
    pub feeder_ready: bool,
    /// Optional forced duplex face (`front` / `back`).
    pub side: Option<&'static str>,
    /// Optional sheet order (`forward` / `reverse`).
    pub order: Option<&'static str>,
}

impl Scenario {
    pub fn pattern_id(self) -> &'static str {
        self.pattern.as_str()
    }
}

/// Recommended order for people exercising pdfcraft / WIA / airscan against fakescan.
pub const PROGRESSION: &[Scenario] = &[
    Scenario {
        id: "order",
        step: 1,
        of: 7,
        pattern: PatternId::PageId,
        title: "1/7 PAGE ORDER",
        check: "Multi-page feeder: pages arrive 0,1,2… (QR page= / COM fakescan-page:). Bars match count.",
        pages: 3,
        feeder_ready: true,
        side: None,
        order: None,
    },
    Scenario {
        id: "qr",
        step: 2,
        of: 7,
        pattern: PatternId::Qr,
        title: "2/7 QR DECODE",
        check: "Decode QR with zbar/zxing/rqrr — no OCR. Payload fakescan|v1|page=…|pat=qr.",
        pages: 1,
        feeder_ready: true,
        side: None,
        order: None,
    },
    Scenario {
        id: "tone",
        step: 3,
        of: 7,
        pattern: PatternId::Wedges,
        title: "3/7 TONE WEDGES",
        check: "Gray steps visible. Later: threshold/brightness should change which wedges survive.",
        pages: 1,
        feeder_ready: false,
        side: None,
        order: None,
    },
    Scenario {
        id: "color",
        step: 4,
        of: 7,
        pattern: PatternId::Patches,
        title: "4/7 COLOR PATCHES",
        check: "RGB vs gray vs bw acquire: patches stay separable only in color modes.",
        pages: 1,
        feeder_ready: false,
        side: None,
        order: None,
    },
    Scenario {
        id: "orient",
        step: 5,
        of: 7,
        pattern: PatternId::SlantF,
        title: "5/7 ORIENTATION",
        check: "Big F is top-left. Rotate/deskew/mirror settings should move it predictably.",
        pages: 1,
        feeder_ready: false,
        side: None,
        order: None,
    },
    Scenario {
        id: "crop",
        step: 6,
        of: 7,
        pattern: PatternId::Corners,
        title: "6/7 CORNERS CROP",
        check: "All four L-marks present on full page; missing marks mean crop/region wrong.",
        pages: 1,
        feeder_ready: false,
        side: None,
        order: None,
    },
    Scenario {
        id: "duplex",
        step: 7,
        of: 7,
        pattern: PatternId::DuplexPair,
        title: "7/7 DUPLEX REVERSE",
        check: "Acquire fronts, then POST /v1/pages {count:3,side:back,order:reverse}. Interleave → F0,B0…",
        pages: 3,
        feeder_ready: true,
        side: Some("front"),
        order: None,
    },
];

/// Lab knobs outside the smoke walk (POST `/v1/scenario` still accepts these ids).
/// Pair `sticky_empty` with a sticky profile (`brother-usb`) so status keeps FEED_READY.
pub const LAB: &[Scenario] = &[Scenario {
    id: "sticky_empty",
    step: 0,
    of: 0,
    pattern: PatternId::PageId,
    title: "STICKY EMPTY ADF",
    check: "On brother-usb: sticky_ready+feeder_ready+pages=0; ADF Create faults; Platen RetrieveImage OK; tray still sticky.",
    pages: 0,
    feeder_ready: true,
    side: None,
    order: None,
}];

pub fn by_id(id: &str) -> Option<&'static Scenario> {
    let key = id.trim().to_ascii_lowercase();
    PROGRESSION.iter().chain(LAB.iter()).find(|s| s.id == key)
}

pub fn by_step(step: u8) -> Option<&'static Scenario> {
    PROGRESSION.iter().find(|s| s.step == step)
}

pub fn next_after(id: &str) -> Option<&'static Scenario> {
    let key = id.trim().to_ascii_lowercase();
    // Lab ids (of == 0) are outside the smoke walk.
    let cur = PROGRESSION.iter().find(|s| s.id == key)?;
    by_step(cur.step + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progression_is_contiguous_1_to_n() {
        assert_eq!(PROGRESSION.len(), 7);
        for (i, s) in PROGRESSION.iter().enumerate() {
            assert_eq!(s.step as usize, i + 1);
            assert_eq!(s.of as usize, PROGRESSION.len());
            assert!(by_id(s.id).is_some());
        }
        assert_eq!(next_after("order").unwrap().id, "qr");
        assert!(next_after("duplex").is_none());
    }

    #[test]
    fn sticky_empty_lab_scenario_is_findable() {
        let s = by_id("sticky_empty").expect("lab id");
        assert_eq!(s.pages, 0);
        assert!(s.feeder_ready);
        assert_eq!(s.step, 0);
        assert!(next_after("sticky_empty").is_none());
    }
}
