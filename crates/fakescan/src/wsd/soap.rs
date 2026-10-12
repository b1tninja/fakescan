use std::io::{Cursor, Write};
use std::time::{Duration, Instant, SystemTime};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use uuid::Uuid;

pub use super::envelope::Envelope;

/// Canonical WS-Scan namespace (Windows WSDScan / WDP Scan Service).
pub const SCAN_NS: &str = "http://schemas.microsoft.com/windows/2006/01/wdp/scan";
/// Alternate namespace sane-airscan binds and writes (`2006/08`).
pub const SCAN_NS_AIRSCAN: &str = "http://schemas.microsoft.com/windows/2006/08/wdp/scan";
pub const WSA_NS: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing";
pub const SOAP_NS: &str = "http://www.w3.org/2003/05/soap-envelope";
/// WS-Eventing (WSDScan subscribes for scanner status / job events).
pub const WSE_NS: &str = "http://schemas.xmlsoap.org/ws/2004/08/eventing";

pub fn action(body: &str) -> Option<String> {
    Envelope::parse(body).action_uri(scan_ops(), WSE_NS, SCAN_NS)
}

const fn scan_ops() -> &'static [&'static str] {
    &[
        "GetScannerElements",
        "CreateScanJob",
        "RetrieveImage",
        "CancelJob",
        "ValidateScanTicket",
        "GetActiveJobs",
        "GetJobHistory",
        "GetJobElements",
    ]
}

pub fn eventing_action(short: &str) -> String {
    format!("{WSE_NS}/{short}")
}

/// Echo requested `wse:Expires` when it looks like xs:duration or UTC `xs:dateTime`;
/// else a long grant.
pub fn eventing_expires(body: &str) -> String {
    if let Some(raw) = extract_tagged(body, "Expires") {
        let t = raw.trim();
        if t.starts_with('P') || t.starts_with('p') {
            return esc(t);
        }
        // Absolute UTC dateTime (`…Z` or `…+00:00`) — echoed for Instant deadline enforcement.
        if t.contains('T') && (t.ends_with('Z') || t.ends_with("+00:00")) {
            return esc(t);
        }
    }
    "PT8H".into()
}

/// `wse:Identifier` from header `ReferenceParameters`, not a body `ScanIdentifier`.
pub fn eventing_identifier(body: &str) -> Option<String> {
    Envelope::parse(body).identifier
}

/// `wse:Filter/@Dialect` from a Subscribe body when present.
///
/// DPWS prefers an Action filter; Windows may send scan-specific dialects.
/// Unknown dialects are ignored (not faulted) — callers log for Brother compare.
pub fn eventing_filter_dialect(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("filter") {
        let at = search + rel;
        // Element start: `<…Filter` / `<Filter` — not a closing tag or text.
        let before = body.as_bytes().get(at.wrapping_sub(1)).copied();
        if !matches!(before, Some(b'<') | Some(b':')) {
            search = at + 6;
            continue;
        }
        let after = &body[at..];
        let tag_end = after.find('>')?;
        let open_l = after[..tag_end].to_ascii_lowercase();
        for quote in ['"', '\''] {
            let key = format!("dialect={quote}");
            if let Some(d0) = open_l.find(&key) {
                // Attr name is ASCII — same byte offsets on the original open tag.
                let rest = &after[d0 + key.len()..tag_end];
                if let Some(d1) = rest.find(quote) {
                    let v = rest[..d1].trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        search = at + 6;
    }
    None
}

/// Parse `xs:duration` forms used for `wse:Expires` deadlines.
///
/// Supported:
/// - time-only `PT…` with units in order H, then M, then S (`PT1H`, `PT90M`,
///   `PT1H30M`, `PT1H30M10S`);
/// - day component `PnD` with optional time after `T` (`P1D`, `P1DT2H`).
///
/// Years/months (`PnY` / `PnM` before `T`) and absolute `xs:dateTime` return `None`.
/// Each accepted unit is optional, but at least one is required; a bare `T` with no
/// time unit is rejected.
pub fn parse_pt_duration(expires: &str) -> Option<Duration> {
    let t = expires.trim();
    if t.len() < 3 {
        return None;
    }
    let upper = t.to_ascii_uppercase();
    if !upper.starts_with('P') {
        return None;
    }
    let mut rest = &upper[1..];
    let mut total_secs: u64 = 0;
    let mut saw_unit = false;

    // Optional day before `T` (or alone for `P1D`).
    if !rest.starts_with('T') {
        if let Some((n, rem)) = take_pt_unit(rest, b'D') {
            total_secs = total_secs.checked_add(n.checked_mul(86400)?)?;
            rest = rem;
            saw_unit = true;
        } else if !rest.is_empty() {
            // Leftover before T (e.g. P1Y / P1M) — not a fixed Instant delta.
            return None;
        }
    }

    if let Some(after_t) = rest.strip_prefix('T') {
        rest = after_t;
        let mut saw_time = false;
        if let Some((n, rem)) = take_pt_unit(rest, b'H') {
            total_secs = total_secs.checked_add(n.checked_mul(3600)?)?;
            rest = rem;
            saw_time = true;
        }
        if let Some((n, rem)) = take_pt_unit(rest, b'M') {
            total_secs = total_secs.checked_add(n.checked_mul(60)?)?;
            rest = rem;
            saw_time = true;
        }
        if let Some((n, rem)) = take_pt_unit(rest, b'S') {
            total_secs = total_secs.checked_add(n)?;
            rest = rem;
            saw_time = true;
        }
        if !saw_time {
            return None;
        }
        saw_unit = true;
    }

    if !saw_unit || !rest.is_empty() {
        return None;
    }
    Some(Duration::from_secs(total_secs))
}

/// Digits followed by `unit`, or `None` if `s` does not start with that unit field.
fn take_pt_unit(s: &str, unit: u8) -> Option<(u64, &str)> {
    let digit_end = s.find(|c: char| !c.is_ascii_digit())?;
    if digit_end == 0 || s.as_bytes().get(digit_end) != Some(&unit) {
        return None;
    }
    let n: u64 = s[..digit_end].parse().ok()?;
    Some((n, &s[digit_end + 1..]))
}

/// `now + parse_pt_duration(expires)` when the Expires string is a supported
/// `xs:duration` (`PT…` or `PnD` / `PnDT…`).
pub fn pt_deadline_now(expires: &str) -> Option<Instant> {
    parse_pt_duration(expires).and_then(|d| Instant::now().checked_add(d))
}

/// Subscription deadline from `wse:Expires`: duration (`PT…` / `P1D` / `P1DT2H`) →
/// now+duration, else UTC `xs:dateTime` ending in `Z` or `+00:00`
/// (`YYYY-MM-DDTHH:MM:SS…`, fractional seconds stripped).
///
/// Absolute time maps to Instant via `SystemTime` delta from now. Past → `Instant::now()`
/// (already elapsed). Other offsets are not supported.
pub fn expires_deadline_now(expires: &str) -> Option<Instant> {
    if let Some(deadline) = pt_deadline_now(expires) {
        return Some(deadline);
    }
    let target = parse_xs_datetime_utc(expires)?;
    let now_st = SystemTime::now();
    match target.duration_since(now_st) {
        Ok(delta) => Instant::now().checked_add(delta),
        Err(_) => Some(Instant::now()),
    }
}

/// UTC `xs:dateTime` ending in `Z` or `+00:00`. Fractional seconds after `.` are stripped.
fn parse_xs_datetime_utc(expires: &str) -> Option<SystemTime> {
    let t = expires.trim();
    let mut core = if let Some(stripped) = t.strip_suffix('Z') {
        stripped
    } else {
        t.strip_suffix("+00:00")?
    };
    if let Some(dot) = core.rfind('.') {
        if core[..dot].contains('T') {
            core = &core[..dot];
        }
    }
    // YYYY-MM-DDTHH:MM:SS
    if core.len() != 19 || core.as_bytes().get(10) != Some(&b'T') {
        return None;
    }
    let b = core.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let y: i32 = core[0..4].parse().ok()?;
    let mo: u32 = core[5..7].parse().ok()?;
    let d: u32 = core[8..10].parse().ok()?;
    let h: u32 = core[11..13].parse().ok()?;
    let mi: u32 = core[14..16].parse().ok()?;
    let s: u32 = core[17..19].parse().ok()?;
    let unix = civil_to_unix_secs(y, mo, d, h, mi, s)?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(unix))
}

fn is_leap_year(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days+time since Unix epoch as seconds. Years before 1970 are rejected.
fn civil_to_unix_secs(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<u64> {
    if y < 1970 {
        return None;
    }
    let dim = days_in_month(y, mo);
    if mo < 1 || dim == 0 || d < 1 || d > dim || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let mut days: u64 = 0;
    for year in 1970..y {
        days += if is_leap_year(year) { 366 } else { 365 };
    }
    for month in 1..mo {
        days += u64::from(days_in_month(y, month));
    }
    days += u64::from(d - 1);
    days.checked_mul(86400)?
        .checked_add(u64::from(h) * 3600)?
        .checked_add(u64::from(mi) * 60)?
        .checked_add(u64::from(s))
}

/// ClientContext values from `wscn:ScanDestinations` in a Subscribe body (0..n).
pub fn scan_destination_contexts(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut idx = 0;
    while let Some(rel) = body[idx..].find("ClientContext") {
        let at = idx + rel;
        let before = body.as_bytes().get(at.wrapping_sub(1)).copied();
        let after = body.get(at + "ClientContext".len()..).unwrap_or("");
        if matches!(before, Some(b'<') | Some(b':')) && after.starts_with('>') {
            if let Some(end) = after[1..].find('<') {
                let value = after[1..1 + end].trim();
                if !value.is_empty() {
                    out.push(value.to_string());
                }
            }
        }
        idx = at + "ClientContext".len();
    }
    out
}

/// `wsa:Address` inside `wse:NotifyTo` from a Subscribe body (event sink URL).
pub fn notify_to_address(body: &str) -> Option<String> {
    let mut idx = 0;
    while let Some(rel) = body[idx..].find("NotifyTo") {
        let at = idx + rel;
        let before = body.as_bytes().get(at.wrapping_sub(1)).copied();
        let after = body.get(at + "NotifyTo".len()..).unwrap_or("");
        if !matches!(before, Some(b'<') | Some(b':')) {
            idx = at + "NotifyTo".len();
            continue;
        }
        let Some(gt) = after.find('>') else {
            idx = at + "NotifyTo".len();
            continue;
        };
        // Self-closing <wse:NotifyTo … />
        if after.as_bytes().get(gt.saturating_sub(1)) == Some(&b'/') {
            idx = at + "NotifyTo".len();
            continue;
        }
        let inner_start = at + "NotifyTo".len() + gt + 1;
        let rest = body.get(inner_start..)?;
        let Some(close_rel) = rest.find("NotifyTo>") else {
            idx = at + "NotifyTo".len();
            continue;
        };
        let region = &rest[..close_rel];
        if let Some(addr) = extract_tagged(region, "Address") {
            let t = addr.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        idx = inner_start + close_rel + "NotifyTo>".len();
    }
    None
}

/// Prefer the Action URI's scan namespace; fall back to xmlns on the envelope.
pub fn scan_ns_from_body(body: &str) -> &'static str {
    let env = Envelope::parse(body);
    let action = env
        .action
        .clone()
        .or_else(|| env.action_uri(scan_ops(), WSE_NS, SCAN_NS));
    if let Some(action) = action.as_deref() {
        if action.contains("/2006/08/wdp/scan") {
            return SCAN_NS_AIRSCAN;
        }
        if action.contains("/2006/01/wdp/scan") {
            return SCAN_NS;
        }
    }
    if env.xmlns.iter().any(|ns| ns.contains("/2006/08/wdp/scan")) {
        SCAN_NS_AIRSCAN
    } else {
        SCAN_NS
    }
}

pub fn message_id(body: &str) -> Option<String> {
    // RelatesTo is xs:anyURI. A bare UUID or a sentence is not one, so it is not copied.
    Envelope::parse(body)
        .message_id
        .filter(|id| super::envelope::absolute_iri(id))
}

/// Header problems in a reply this service wrote (SOAP 1.2 + WS-Addressing 2004/08).
pub fn reply_problems(xml: &str) -> Vec<String> {
    Envelope::parse(xml).reply_problems(super::envelope::ReplyRole::Reply)
}

/// Scan-schema problems in a request or a reply body.
pub fn body_problems(xml: &str) -> Vec<String> {
    super::scan_schema::body_problems(xml)
}

pub fn extract_tagged(xml: &str, local: &str) -> Option<String> {
    let mut idx = 0;
    while let Some(rel) = xml[idx..].find(local) {
        let at = idx + rel;
        let before = xml.as_bytes().get(at.wrapping_sub(1)).copied();
        let after = xml.get(at + local.len()..).unwrap_or("");
        if matches!(before, Some(b'<') | Some(b':')) && after.starts_with('>') {
            let value_start = &after[1..];
            if let Some(end) = value_start.find('<') {
                let value = value_start[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        idx = at + local.len();
    }
    None
}

pub fn envelope(
    scan_ns: &str,
    action_uri: &str,
    relates_to: Option<&str>,
    body_xml: &str,
) -> String {
    let msg = Uuid::new_v4();
    // Both sca: and wscn: bind the scan NS — Windows GSE requests use
    // sca:ScannerDescription and ElementData/@Name must resolve that prefix.
    write_envelope(
        &[("xmlns:wscn", scan_ns), ("xmlns:sca", scan_ns)],
        action_uri,
        &format!("uuid:{msg}"),
        relates_to,
        body_xml,
    )
}

fn write_envelope(
    extra_xmlns: &[(&str, &str)],
    action_uri: &str,
    message_id: &str,
    relates_to: Option<&str>,
    body_xml: &str,
) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)));
    let mut env = BytesStart::new("soap:Envelope");
    env.push_attribute(("xmlns:soap", SOAP_NS));
    env.push_attribute(("xmlns:wsa", WSA_NS));
    for (name, value) in extra_xmlns {
        env.push_attribute((*name, *value));
    }
    let _ = writer.write_event(Event::Start(env));
    let _ = writer.write_event(Event::Start(BytesStart::new("soap:Header")));
    write_text_el(
        &mut writer,
        "wsa:To",
        "http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous",
    );
    write_text_el(&mut writer, "wsa:Action", action_uri);
    write_text_el(&mut writer, "wsa:MessageID", message_id);
    if let Some(relates) = relates_to.map(str::trim).filter(|s| !s.is_empty()) {
        write_text_el(&mut writer, "wsa:RelatesTo", relates);
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("soap:Header")));
    let _ = writer.write_event(Event::Start(BytesStart::new("soap:Body")));
    // write_all advances the Cursor. extend_from_slice on the inner Vec does not,
    // so the closing tags would overwrite the start of the payload.
    let _ = writer.get_mut().write_all(body_xml.as_bytes());
    let _ = writer.write_event(Event::End(BytesEnd::new("soap:Body")));
    let _ = writer.write_event(Event::End(BytesEnd::new("soap:Envelope")));
    let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap_or_default();
    debug_assert!(
        Envelope::parse(&xml)
            .reply_problems(super::envelope::ReplyRole::Reply)
            .is_empty(),
        "{xml}"
    );
    xml
}

fn write_text_el(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, text: &str) {
    let _ = writer.write_event(Event::Start(BytesStart::new(name)));
    let _ = writer.write_event(Event::Text(BytesText::new(text)));
    let _ = writer.write_event(Event::End(BytesEnd::new(name)));
}

pub fn fault_with_code(subcode: Option<&str>, reason: &str, relates_to: Option<&str>) -> String {
    let reason = esc(reason);
    let sub = subcode
        .map(|c| {
            format!(
                r#"
        <soap:Subcode>
          <soap:Value>{c}</soap:Value>
        </soap:Subcode>"#
            )
        })
        .unwrap_or_default();
    let body = format!(
        r#"    <soap:Fault>
      <soap:Code>
        <soap:Value>soap:Sender</soap:Value>{sub}
      </soap:Code>
      <soap:Reason><soap:Text xml:lang="en">{reason}</soap:Text></soap:Reason>
    </soap:Fault>"#
    );
    envelope(
        SCAN_NS,
        "http://schemas.xmlsoap.org/ws/2004/08/addressing/fault",
        relates_to,
        &body,
    )
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn short_action(action_uri: &str) -> &str {
    action_uri.rsplit('/').next().unwrap_or(action_uri)
}

pub fn scan_action(scan_ns: &str, short: &str) -> String {
    format!("{scan_ns}/{short}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_airscan_ns_from_action() {
        let body = r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/08/wdp/scan/CreateScanJob</wsa:Action>"#;
        assert_eq!(scan_ns_from_body(body), SCAN_NS_AIRSCAN);
    }

    #[test]
    fn detects_windows_ns_from_action() {
        let body = r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/GetScannerElements</wsa:Action>"#;
        assert_eq!(scan_ns_from_body(body), SCAN_NS);
    }

    #[test]
    fn relates_to_echoes_verbatim() {
        let xml = envelope(
            SCAN_NS,
            &scan_action(SCAN_NS, "CancelJob"),
            Some("urn:uuid:abc"),
            "<x/>",
        );
        assert!(xml.contains("<wsa:RelatesTo>urn:uuid:abc</wsa:RelatesTo>"));
        assert!(xml.contains("<soap:Body><x/></soap:Body>"), "{xml}");
        assert!(reply_problems(&xml).is_empty(), "{xml}");
    }

    #[test]
    fn replies_mint_a_fresh_message_id() {
        let one = envelope(
            SCAN_NS,
            &scan_action(SCAN_NS, "CancelJobResponse"),
            None,
            "<x/>",
        );
        let two = envelope(
            SCAN_NS,
            &scan_action(SCAN_NS, "CancelJobResponse"),
            None,
            "<x/>",
        );
        let id = |xml: &str| Envelope::parse(xml).message_id.unwrap();
        assert_ne!(id(&one), id(&two));
        assert!(id(&one).starts_with("uuid:"));
        assert!(reply_problems(&one).is_empty(), "{one}");
    }

    #[test]
    fn bare_uuid_is_not_copied_to_relates_to() {
        let body = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing">
          <soap:Header><wsa:MessageID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</wsa:MessageID></soap:Header>
          <soap:Body><wscn:GetScannerElementsRequest/></soap:Body>
        </soap:Envelope>"#;
        assert!(message_id(body).is_none());
        let findings = Envelope::parse(body).request_findings();
        assert!(findings.iter().any(|f| f.field == "MessageID" && !f.reject));
    }

    #[test]
    fn short_action_falls_through_to_the_body_element() {
        let body = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Header><wsa:Action>GetScannerElements</wsa:Action></soap:Header>
          <soap:Body><wscn:GetScannerElementsRequest/></soap:Body>
        </soap:Envelope>"#;
        assert_eq!(
            action(body).as_deref(),
            Some("http://schemas.microsoft.com/windows/2006/01/wdp/scan/GetScannerElements")
        );
    }

    #[test]
    fn unknown_must_understand_header_is_rejected() {
        let body = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing">
          <soap:Header>
            <wsa:Action soap:mustUnderstand="true">http://schemas.microsoft.com/windows/2006/01/wdp/scan/GetScannerElements</wsa:Action>
            <ex:Trace soap:mustUnderstand="1">no</ex:Trace>
          </soap:Header>
          <soap:Body/>
        </soap:Envelope>"#;
        let findings = Envelope::parse(body).request_findings();
        assert!(findings
            .iter()
            .any(|f| f.field == "mustUnderstand" && f.reject));
        assert!(findings.iter().all(|f| f.field != "Action" || !f.reject));
    }

    #[test]
    fn envelope_declares_sca_and_wscn() {
        let xml = envelope(
            SCAN_NS,
            &scan_action(SCAN_NS, "GetScannerElementsResponse"),
            None,
            "<x/>",
        );
        assert!(
            xml.contains(&format!(r#"xmlns:sca="{SCAN_NS}""#)),
            "sca: must bind for ElementData Name=sca:…; got {xml}"
        );
        assert!(xml.contains(&format!(r#"xmlns:wscn="{SCAN_NS}""#)), "{xml}");
    }

    #[test]
    fn parses_header_and_keeps_body_markup() {
        let xml = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/08/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/08/wdp/scan/CreateScanJob</wsa:Action>
    <wsa:MessageID>urn:uuid:abcd</wsa:MessageID>
    <wsa:ReferenceParameters>
      <wse:Identifier>uuid:sub-9</wse:Identifier>
    </wsa:ReferenceParameters>
  </soap:Header>
  <soap:Body>
    <wscn:CreateScanJobRequest wscn:MustHonor="true">
      <wscn:ScanIdentifier>uuid:scan</wscn:ScanIdentifier>
      <wscn:Format>jfif</wscn:Format>
    </wscn:CreateScanJobRequest>
  </soap:Body>
</soap:Envelope>"#;
        let env = Envelope::parse(xml);
        assert_eq!(
            env.action.as_deref(),
            Some("http://schemas.microsoft.com/windows/2006/08/wdp/scan/CreateScanJob")
        );
        assert_eq!(env.message_id.as_deref(), Some("urn:uuid:abcd"));
        assert_eq!(env.identifier.as_deref(), Some("uuid:sub-9"));
        assert_eq!(env.body_name.as_deref(), Some("CreateScanJobRequest"));
        assert!(
            env.body.contains(r#"wscn:MustHonor="true""#),
            "{}",
            env.body
        );
        assert!(env.body.contains("<wscn:Format>jfif</wscn:Format>"));
        assert_eq!(scan_ns_from_body(xml), SCAN_NS_AIRSCAN);
    }

    #[test]
    fn infers_action_from_body_element_when_header_omits_it() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wse="http://schemas.xmlsoap.org/ws/2004/08/eventing">
  <soap:Header/>
  <soap:Body><wse:Subscribe><wse:Expires>PT1H</wse:Expires></wse:Subscribe></soap:Body>
</soap:Envelope>"#;
        assert_eq!(
            action(xml).as_deref(),
            Some("http://schemas.xmlsoap.org/ws/2004/08/eventing/Subscribe")
        );
    }

    #[test]
    fn reads_action_inside_a_mime_part() {
        let mime = "\
--bound\r\n\
Content-Type: application/soap+xml\r\n\
\r\n\
<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
<soap:Header><wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/RetrieveImage</wsa:Action>\
<wsa:MessageID>uuid:mime</wsa:MessageID></soap:Header>\
<soap:Body><wscn:RetrieveImageRequest/></soap:Body></soap:Envelope>\r\n\
--bound--\r\n";
        assert_eq!(
            action(mime).as_deref(),
            Some("http://schemas.microsoft.com/windows/2006/01/wdp/scan/RetrieveImage")
        );
        assert_eq!(message_id(mime).as_deref(), Some("uuid:mime"));
    }

    #[test]
    fn detects_subscribe_action() {
        let body =
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2004/08/eventing/Subscribe</wsa:Action>"#;
        assert_eq!(
            action(body).as_deref(),
            Some("http://schemas.xmlsoap.org/ws/2004/08/eventing/Subscribe")
        );
        assert_eq!(short_action(action(body).as_deref().unwrap()), "Subscribe");
    }

    #[test]
    fn eventing_filter_dialect_from_subscribe() {
        let action = r#"<wse:Filter Dialect="http://schemas.xmlsoap.org/ws/2006/02/devprof/Action">
  <wsdp:Actions>…</wsdp:Actions>
</wse:Filter>"#;
        assert_eq!(
            eventing_filter_dialect(action).as_deref(),
            Some("http://schemas.xmlsoap.org/ws/2006/02/devprof/Action")
        );
        let single = r#"<wse:Filter Dialect='urn:example:ScanEvents'/>"#;
        assert_eq!(
            eventing_filter_dialect(single).as_deref(),
            Some("urn:example:ScanEvents")
        );
        assert!(eventing_filter_dialect(r#"<wse:Expires>PT1H</wse:Expires>"#).is_none());
        assert!(eventing_filter_dialect(r#"Filter Dialect="x""#).is_none());
    }

    #[test]
    fn eventing_expires_prefers_duration() {
        let body = r#"<wse:Expires>PT1H</wse:Expires>"#;
        assert_eq!(eventing_expires(body), "PT1H");
        assert_eq!(eventing_expires("<x/>"), "PT8H");
        assert_eq!(
            eventing_expires(r#"<wse:Expires>2026-10-12T15:00:00Z</wse:Expires>"#),
            "2026-10-12T15:00:00Z"
        );
        assert_eq!(
            eventing_expires(r#"<wse:Expires>2026-10-12T15:00:00+00:00</wse:Expires>"#),
            "2026-10-12T15:00:00+00:00"
        );
    }

    #[test]
    fn eventing_identifier_from_reference_parameters() {
        let body = r#"
  <wsa:ReferenceParameters>
    <wse:Identifier>uuid:sub-1</wse:Identifier>
  </wsa:ReferenceParameters>
  <wse:Renew><wse:Expires>PT1H</wse:Expires></wse:Renew>"#;
        assert_eq!(eventing_identifier(body).as_deref(), Some("uuid:sub-1"));
        assert!(eventing_identifier("<wse:Renew/>").is_none());
        // ScanIdentifier must not match the eventing Identifier local name.
        assert!(
            eventing_identifier("<wscn:ScanIdentifier>uuid:scan</wscn:ScanIdentifier>").is_none()
        );
    }

    #[test]
    fn parse_pt_duration_simple_units() {
        assert_eq!(parse_pt_duration("PT1H"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_pt_duration("PT30M"), Some(Duration::from_secs(1800)));
        assert_eq!(parse_pt_duration("PT45S"), Some(Duration::from_secs(45)));
        assert_eq!(parse_pt_duration("pt2h"), Some(Duration::from_secs(7200)));
        assert!(parse_pt_duration("2026-01-01T00:00:00Z").is_none());
        assert!(parse_pt_duration("").is_none());
    }

    #[test]
    fn parse_pt_duration_compound() {
        assert_eq!(
            parse_pt_duration("PT1H30M"),
            Some(Duration::from_secs(5400))
        );
        assert_eq!(parse_pt_duration("PT90M"), Some(Duration::from_secs(5400)));
        assert_eq!(
            parse_pt_duration("PT1H30M10S"),
            Some(Duration::from_secs(5410))
        );
        assert_eq!(
            parse_pt_duration("PT1H10S"),
            Some(Duration::from_secs(3610))
        );
        // Wrong unit order / leftover text.
        assert!(parse_pt_duration("PT30M1H").is_none());
        assert!(parse_pt_duration("PT1S30M").is_none());
        assert!(parse_pt_duration("PT").is_none());
    }

    #[test]
    fn parse_pt_duration_day() {
        assert_eq!(parse_pt_duration("P1D"), Some(Duration::from_secs(86400)));
        assert_eq!(
            parse_pt_duration("P1DT2H"),
            Some(Duration::from_secs(86400 + 7200))
        );
        assert_eq!(
            parse_pt_duration("p1dt30m"),
            Some(Duration::from_secs(86400 + 1800))
        );
        assert_eq!(parse_pt_duration("P0D"), Some(Duration::from_secs(0)));
        // Years / months are not fixed Instant deltas.
        assert!(parse_pt_duration("P1Y").is_none());
        assert!(parse_pt_duration("P1M").is_none());
        assert!(parse_pt_duration("P1YT2H").is_none());
        // Bare T / leftover after day.
        assert!(parse_pt_duration("P1DT").is_none());
        assert!(parse_pt_duration("P1D2H").is_none());
        assert!(parse_pt_duration("P").is_none());
    }

    #[test]
    fn expires_deadline_now_p1d() {
        let before = Instant::now();
        let deadline = expires_deadline_now("P1D").expect("P1D");
        let after = Instant::now();
        let lo = before + Duration::from_secs(86400);
        let hi = after + Duration::from_secs(86400);
        assert!(
            deadline >= lo && deadline <= hi,
            "{deadline:?} not in {lo:?}..{hi:?}"
        );
    }

    #[test]
    fn expires_deadline_now_p1dt2h() {
        let before = Instant::now();
        let deadline = expires_deadline_now("P1DT2H").expect("P1DT2H");
        let after = Instant::now();
        let secs = 86400 + 7200;
        let lo = before + Duration::from_secs(secs);
        let hi = after + Duration::from_secs(secs);
        assert!(
            deadline >= lo && deadline <= hi,
            "{deadline:?} not in {lo:?}..{hi:?}"
        );
    }

    #[test]
    fn pt_deadline_now_roughly_one_hour() {
        let before = Instant::now();
        let deadline = pt_deadline_now("PT1H").expect("PT1H");
        let after = Instant::now();
        let lo = before + Duration::from_secs(3600);
        let hi = after + Duration::from_secs(3600);
        assert!(
            deadline >= lo && deadline <= hi,
            "{deadline:?} not in {lo:?}..{hi:?}"
        );
        assert!(pt_deadline_now("2026-01-01T00:00:00Z").is_none());
    }

    /// Format Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (test helper; mirrors parse rules).
    fn unix_secs_to_xs_z(secs: u64) -> String {
        let mut days = secs / 86400;
        let tod = secs % 86400;
        let h = tod / 3600;
        let mi = (tod % 3600) / 60;
        let s = tod % 60;
        let mut y = 1970i32;
        loop {
            let diy = if is_leap_year(y) { 366u64 } else { 365 };
            if days < diy {
                break;
            }
            days -= diy;
            y += 1;
        }
        let mut mo = 1u32;
        loop {
            let dim = u64::from(days_in_month(y, mo));
            if days < dim {
                break;
            }
            days -= dim;
            mo += 1;
        }
        let d = days as u32 + 1;
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    }

    #[test]
    fn expires_deadline_now_pt1h() {
        let before = Instant::now();
        let deadline = expires_deadline_now("PT1H").expect("PT1H");
        let after = Instant::now();
        let lo = before + Duration::from_secs(3600);
        let hi = after + Duration::from_secs(3600);
        assert!(
            deadline >= lo && deadline <= hi,
            "{deadline:?} not in {lo:?}..{hi:?}"
        );
    }

    #[test]
    fn expires_deadline_now_pt1h30m() {
        let before = Instant::now();
        let deadline = expires_deadline_now("PT1H30M").expect("PT1H30M");
        let after = Instant::now();
        let lo = before + Duration::from_secs(5400);
        let hi = after + Duration::from_secs(5400);
        assert!(
            deadline >= lo && deadline <= hi,
            "{deadline:?} not in {lo:?}..{hi:?}"
        );
    }

    #[test]
    fn expires_deadline_now_future_datetime() {
        let now_unix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("after epoch")
            .as_secs();
        let expires = unix_secs_to_xs_z(now_unix + 60);
        let before = Instant::now();
        let deadline = expires_deadline_now(&expires).expect("future Z");
        let after = Instant::now();
        // ~60s ahead; allow a little slack for scheduling / second boundaries.
        let lo = before + Duration::from_secs(58);
        let hi = after + Duration::from_secs(62);
        assert!(
            deadline >= lo && deadline <= hi,
            "expires={expires} deadline={deadline:?} not in {lo:?}..{hi:?}"
        );
        // Fractional seconds stripped.
        let with_frac = format!("{}.5Z", expires.trim_end_matches('Z'));
        assert!(expires_deadline_now(&with_frac).is_some());
        // Non-UTC offsets rejected.
        assert!(expires_deadline_now("2026-10-12T15:00:00+01:00").is_none());
        assert!(expires_deadline_now("2026-10-12T15:00:00-00:00").is_none());
    }

    #[test]
    fn expires_deadline_now_future_datetime_plus_00_00() {
        let now_unix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("after epoch")
            .as_secs();
        let expires_z = unix_secs_to_xs_z(now_unix + 60);
        let expires = format!("{}+00:00", expires_z.trim_end_matches('Z'));
        let before = Instant::now();
        let deadline = expires_deadline_now(&expires).expect("future +00:00");
        let after = Instant::now();
        let lo = before + Duration::from_secs(58);
        let hi = after + Duration::from_secs(62);
        assert!(
            deadline >= lo && deadline <= hi,
            "expires={expires} deadline={deadline:?} not in {lo:?}..{hi:?}"
        );
        let with_frac = format!("{}.5+00:00", expires_z.trim_end_matches('Z'));
        assert!(expires_deadline_now(&with_frac).is_some());
    }

    #[test]
    fn expires_deadline_now_past_datetime() {
        let deadline = expires_deadline_now("2000-01-01T00:00:00Z").expect("past Z");
        assert!(
            deadline <= Instant::now(),
            "past dateTime must be already elapsed: {deadline:?}"
        );
    }

    #[test]
    fn scan_destination_contexts_extracts() {
        let body = r#"
          <wscn:ScanDestinations>
            <wscn:ScanDestination>
              <wscn:ClientDisplayName>Lab PC</wscn:ClientDisplayName>
              <wscn:ClientContext>ctx-abc</wscn:ClientContext>
            </wscn:ScanDestination>
          </wscn:ScanDestinations>"#;
        assert_eq!(scan_destination_contexts(body), vec!["ctx-abc".to_string()]);
    }

    #[test]
    fn notify_to_address_extracts_from_subscribe() {
        let body = r#"
    <wse:Subscribe>
      <wse:Delivery>
        <wse:NotifyTo>
          <wsa:Address>http://127.0.0.1:9/sink</wsa:Address>
        </wse:NotifyTo>
      </wse:Delivery>
      <wscn:ScanDestinations>
        <wscn:ScanDestination>
          <wscn:ClientContext>ctx-lab</wscn:ClientContext>
        </wscn:ScanDestination>
      </wscn:ScanDestinations>
    </wse:Subscribe>"#;
        assert_eq!(
            notify_to_address(body).as_deref(),
            Some("http://127.0.0.1:9/sink")
        );
        assert!(notify_to_address("<wse:Subscribe/>").is_none());
    }
}
