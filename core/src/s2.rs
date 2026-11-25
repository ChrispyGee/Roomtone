//! Which speakers can move to S2? Read-only: it looks at each speaker's own description (model,
//! hardware version, which software generation it runs) and compares the model with Sonos's
//! published list of products that can't run S2.
//!
//! Sonos's list is by product and generation, and a few products share one model number across
//! generations (the Connect and the Connect:Amp: "first generation can't, second generation,
//! from 2015, can"). Those come out as "check", never as a guess either way.

use roxmltree::Document;

/// What a speaker says about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    pub model_name: String,
    pub model_number: String,
    pub hardware: String,
    /// The software generation the speaker runs: 1 for S1, 2 for S2 (0 if it doesn't say).
    pub software_generation: u32,
}

/// Read `/xml/device_description.xml`.
pub fn parse_description(xml: &str) -> Option<Details> {
    let doc = Document::parse(xml).ok()?;
    let text = |name: &str| doc.descendants().find(|n| n.tag_name().name() == name).and_then(|n| n.text()).map(|t| t.trim().to_string()).unwrap_or_default();
    let d =
        Details { model_name: text("modelName"), model_number: text("modelNumber"), hardware: text("hardwareVersion"), software_generation: text("swGen").parse().unwrap_or(0) };
    (!d.model_number.is_empty() || !d.model_name.is_empty()).then_some(d)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Already runs S2.
    AlreadyS2,
    /// Not on Sonos's list of products that can't run S2.
    Can,
    /// On the list: this product can't run S2.
    Cannot,
    /// The product has a generation that can and one that can't, with the same model number.
    Check,
}

/// Model numbers of products Sonos says can't run S2, whatever their hardware version:
/// Zone Players (ZP80, ZP100), the controllers (CR100, CR200) and the Bridge (BR100).
const CANNOT: &[&str] = &["ZP80", "ZP100", "CR100", "CR200", "BR100"];
/// Model numbers shared by a first generation that can't run S2 and a second that can:
/// the Connect (ZP90), the Connect:Amp (ZP120) and the Play:5 (S5).
const CHECK: &[&str] = &["ZP90", "ZP120", "S5"];

pub fn assess(d: &Details) -> Verdict {
    if d.software_generation >= 2 {
        return Verdict::AlreadyS2;
    }
    let model = d.model_number.trim().to_uppercase();
    if CANNOT.contains(&model.as_str()) {
        Verdict::Cannot
    } else if CHECK.contains(&model.as_str()) {
        Verdict::Check
    } else {
        Verdict::Can
    }
}

/// One room in the report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomReport {
    pub room: String,
    pub details: Details,
    pub verdict: Verdict,
}

/// How a whole system looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub already: usize,
    pub can: usize,
    pub cannot: usize,
    pub check: usize,
}

pub fn summarize(rooms: &[RoomReport]) -> Summary {
    let mut s = Summary::default();
    for r in rooms {
        match r.verdict {
            Verdict::AlreadyS2 => s.already += 1,
            Verdict::Can => s.can += 1,
            Verdict::Cannot => s.cannot += 1,
            Verdict::Check => s.check += 1,
        }
    }
    s
}

#[cfg(test)]
mod unit {
    use super::*;

    /// The shape of a real S1 speaker's description (a Connect:Amp), trimmed.
    const REAL: &str = r#"<?xml version="1.0"?><root xmlns="urn:schemas-upnp-org:device-1-0"><device><deviceType>urn:schemas-upnp-org:device:ZonePlayer:1</deviceType><friendlyName>10.0.0.5 - Sonos Connect:Amp</friendlyName><manufacturer>Sonos, Inc.</manufacturer><modelNumber>ZP120</modelNumber><modelName>Sonos Connect:Amp</modelName><softwareVersion>57.23-74170</softwareVersion><hardwareVersion>1.16.3.1-2.0</hardwareVersion><swGen>1</swGen><minCompatibleVersion>56.0-00000</minCompatibleVersion></device></root>"#;

    fn d(model: &str, sw: u32) -> Details {
        Details { model_number: model.into(), software_generation: sw, ..Default::default() }
    }

    #[test]
    fn a_real_description_is_read() {
        let got = parse_description(REAL).unwrap();
        assert_eq!((got.model_number.as_str(), got.model_name.as_str(), got.hardware.as_str(), got.software_generation), ("ZP120", "Sonos Connect:Amp", "1.16.3.1-2.0", 1));
        assert!(parse_description("<root/>").is_none());
        assert!(parse_description("not xml").is_none());
    }

    #[test]
    fn verdicts() {
        assert_eq!(assess(&d("ZP100", 1)), Verdict::Cannot);
        assert_eq!(assess(&d("CR200", 1)), Verdict::Cannot);
        assert_eq!(assess(&d("BR100", 1)), Verdict::Cannot);
        assert_eq!(assess(&d("zp120", 1)), Verdict::Check, "case doesn't matter, and the generation can't be told from the model");
        assert_eq!(assess(&d("ZP90", 1)), Verdict::Check);
        assert_eq!(assess(&d("S1", 1)), Verdict::Can, "a Play:1 isn't on the list");
        assert_eq!(assess(&d("S12", 1)), Verdict::Can);
        assert_eq!(assess(&d("ZP120", 2)), Verdict::AlreadyS2, "running S2 already beats the model");
        assert_eq!(assess(&Details::default()), Verdict::Can, "no model said: not on the list, so not claimed");
    }

    #[test]
    fn the_system_summary_counts() {
        let r = |v| RoomReport { room: "x".into(), details: Details::default(), verdict: v };
        let s = summarize(&[r(Verdict::Can), r(Verdict::Can), r(Verdict::Cannot), r(Verdict::Check), r(Verdict::AlreadyS2)]);
        assert_eq!(s, Summary { already: 1, can: 2, cannot: 1, check: 1 });
        assert_eq!(summarize(&[]), Summary::default());
    }
}
