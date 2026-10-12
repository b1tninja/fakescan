use uuid::Uuid;

pub const SCAN_NS: &str = "http://schemas.microsoft.com/windows/2006/01/wdp/scan";
pub const WSA_NS: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing";
pub const SOAP_NS: &str = "http://www.w3.org/2003/05/soap-envelope";

pub fn action(body: &str) -> Option<String> {
    if let Some(v) = extract_tagged(body, "Action") {
        return Some(v);
    }
    for marker in [
        "GetScannerElements",
        "CreateScanJob",
        "RetrieveImage",
        "CancelJob",
        "ValidateScanTicket",
        "GetActiveJobs",
        "GetJobHistory",
        "GetJobElements",
    ] {
        if body.contains(&format!("{marker}Request")) || body.contains(&format!("/{marker}")) {
            return Some(format!("{SCAN_NS}/{marker}"));
        }
    }
    if body.contains("transfer/Get") || body.contains(":Get>") && body.contains("Transfer") {
        return Some("http://schemas.xmlsoap.org/ws/2004/09/transfer/Get".into());
    }
    None
}

pub fn message_id(body: &str) -> Option<String> {
    extract_tagged(body, "MessageID").or_else(|| extract_tagged(body, "MessageId"))
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

pub fn envelope(action_uri: &str, relates_to: Option<&str>, body_xml: &str) -> String {
    let msg = Uuid::new_v4();
    let relates = relates_to
        .map(|r| {
            let r = r.trim();
            let r = r.strip_prefix("urn:uuid:").unwrap_or(r);
            let r = r.strip_prefix("uuid:").unwrap_or(r);
            format!("    <wsa:RelatesTo>uuid:{r}</wsa:RelatesTo>\n")
        })
        .unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_NS}" xmlns:wsa="{WSA_NS}" xmlns:wscn="{SCAN_NS}">
  <soap:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsa:Action>{action_uri}</wsa:Action>
    <wsa:MessageID>uuid:{msg}</wsa:MessageID>
{relates}  </soap:Header>
  <soap:Body>
{body_xml}
  </soap:Body>
</soap:Envelope>"#
    )
}

pub fn fault(reason: &str) -> String {
    let reason = esc(reason);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_NS}" xmlns:wsa="{WSA_NS}">
  <soap:Header>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2004/08/addressing/fault</wsa:Action>
  </soap:Header>
  <soap:Body>
    <soap:Fault>
      <soap:Code><soap:Value>soap:Receiver</soap:Value></soap:Code>
      <soap:Reason><soap:Text xml:lang="en">{reason}</soap:Text></soap:Reason>
    </soap:Fault>
  </soap:Body>
</soap:Envelope>"#
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
