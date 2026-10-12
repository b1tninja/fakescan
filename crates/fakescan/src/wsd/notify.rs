//! ScanAvailableEvent delivery to a WS-Eventing NotifyTo sink.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::warn;
use uuid::Uuid;

use super::soap::{self, SOAP_NS, WSA_NS};

/// WSDL ScanAvailableEvent Action namespace (`2006/08`).
pub const SCAN_AVAILABLE_NS: &str = soap::SCAN_NS_AIRSCAN;

/// Default HTTP timeout for NotifyTo POSTs (lab sinks may be down).
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(3);

/// Build a SOAP 1.2 ScanAvailableEvent envelope (WSDL Action under `scan_ns`).
pub fn scan_available_event_xml(
    scan_ns: &str,
    notify_to: &str,
    scan_identifier: &str,
    client_context: &str,
) -> String {
    let action = soap::scan_action(scan_ns, "ScanAvailableEvent");
    let msg = Uuid::new_v4();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="{SOAP_NS}" xmlns:wsa="{WSA_NS}" xmlns:wscn="{scan_ns}">
  <soap:Header>
    <wsa:To>{}</wsa:To>
    <wsa:Action>{action}</wsa:Action>
    <wsa:MessageID>uuid:{msg}</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:ScanAvailableEvent>
      <wscn:ClientContext>{}</wscn:ClientContext>
      <wscn:ScanIdentifier>{}</wscn:ScanIdentifier>
    </wscn:ScanAvailableEvent>
  </soap:Body>
</soap:Envelope>"#,
        soap::esc(notify_to.trim()),
        soap::esc(client_context),
        soap::esc(scan_identifier),
    )
}

/// POST ScanAvailableEvent to `notify_to`. Best-effort: logs and returns Err on failure.
pub async fn deliver_scan_available(
    notify_to: &str,
    scan_ns: &str,
    scan_identifier: &str,
    client_context: &str,
) -> Result<(), String> {
    let xml = scan_available_event_xml(scan_ns, notify_to, scan_identifier, client_context);
    match post_soap_event(notify_to, "ScanAvailableEvent", &xml).await {
        Ok(()) => Ok(()),
        Err(e) => {
            warn!(
                %notify_to,
                %scan_identifier,
                error = %e,
                "ScanAvailableEvent NotifyTo delivery failed"
            );
            Err(e)
        }
    }
}

/// Generic NotifyTo SOAP POST (ScanAvailableEvent, JobStatusEvent, JobEndStateEvent, …).
pub async fn post_soap_event(url: &str, _action: &str, body: &str) -> Result<(), String> {
    let (host, port, path) = parse_http_url(url)?;
    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/soap+xml; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len(),
    );
    let connect = TcpStream::connect((host.as_str(), port));
    let mut stream = timeout(NOTIFY_TIMEOUT, connect)
        .await
        .map_err(|_| format!("connect timeout to {host}:{port}"))?
        .map_err(|e| format!("connect {host}:{port}: {e}"))?;
    timeout(NOTIFY_TIMEOUT, stream.write_all(req.as_bytes()))
        .await
        .map_err(|_| "write timeout".to_string())?
        .map_err(|e| format!("write: {e}"))?;
    let mut buf = vec![0u8; 1024];
    let n = timeout(NOTIFY_TIMEOUT, stream.read(&mut buf))
        .await
        .map_err(|_| "read timeout".to_string())?
        .map_err(|e| format!("read: {e}"))?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    let status_line = head.lines().next().unwrap_or("");
    // Sinks may close without a status line; treat as delivered.
    if status_line.is_empty() {
        return Ok(());
    }
    match http_status_code(status_line) {
        Some(code) if (200..300).contains(&code) => Ok(()),
        Some(code) => Err(format!("NotifyTo HTTP {code}: {status_line}")),
        None => Ok(()),
    }
}

fn http_status_code(status_line: &str) -> Option<u16> {
    // HTTP/1.1 200 OK
    let mut parts = status_line.split_whitespace();
    let _proto = parts.next()?;
    parts.next()?.parse().ok()
}

fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("NotifyTo must be http:// (got {url})"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err("NotifyTo host missing".into());
    }
    let (host, port) = if let Some(i) = authority.rfind(':') {
        let host = &authority[..i];
        let port: u16 = authority[i + 1..]
            .parse()
            .map_err(|_| format!("bad NotifyTo port in {authority}"))?;
        (host.to_string(), port)
    } else {
        (authority.to_string(), 80)
    };
    let path = if path.is_empty() { "/" } else { path };
    Ok((host, port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wsd::soap::SCAN_NS_AIRSCAN;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[test]
    fn builds_scan_available_event_shape() {
        let xml = scan_available_event_xml(
            SCAN_NS_AIRSCAN,
            "http://127.0.0.1:9/sink",
            "uuid:scan-1",
            "ctx-lab",
        );
        assert!(
            xml.contains(&format!(
                "<wsa:Action>{}/ScanAvailableEvent</wsa:Action>",
                SCAN_NS_AIRSCAN
            )),
            "{xml}"
        );
        assert!(
            xml.contains("<wsa:To>http://127.0.0.1:9/sink</wsa:To>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ClientContext>ctx-lab</wscn:ClientContext>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScanIdentifier>uuid:scan-1</wscn:ScanIdentifier>"),
            "{xml}"
        );
        assert!(xml.contains("<wscn:ScanAvailableEvent>"), "{xml}");
    }

    #[test]
    fn echoes_empty_client_context_element() {
        let xml = scan_available_event_xml(SCAN_NS_AIRSCAN, "http://x/s", "uuid:1", "");
        assert!(
            xml.contains("<wscn:ClientContext></wscn:ClientContext>")
                || xml.contains("<wscn:ClientContext/>"),
            "{xml}"
        );
        assert!(xml.contains("ScanIdentifier"), "{xml}");
    }

    #[test]
    fn parse_http_url_splits_host_port_path() {
        assert_eq!(
            parse_http_url("http://127.0.0.1:9999/evt").unwrap(),
            ("127.0.0.1".into(), 9999, "/evt".into())
        );
        assert_eq!(
            parse_http_url("http://example.com/a").unwrap(),
            ("example.com".into(), 80, "/a".into())
        );
        assert!(parse_http_url("https://x/y").is_err());
    }

    #[tokio::test]
    async fn deliver_posts_soap_to_local_sink() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().unwrap();
        let got = Arc::new(Mutex::new(None::<String>));
        let got2 = got.clone();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.expect("read");
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            *got2.lock().await = Some(req);
            let _ = sock
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
        });

        let url = format!("http://{addr}/sink");
        deliver_scan_available(&url, SCAN_NS_AIRSCAN, "uuid:push-1", "ctx-a")
            .await
            .expect("deliver");

        let req = got.lock().await.clone().expect("request captured");
        assert!(req.starts_with("POST /sink HTTP/1.1"), "method/path: {req}");
        assert!(
            req.contains("Content-Type: application/soap+xml"),
            "content-type: {req}"
        );
        assert!(
            req.contains("<wscn:ScanIdentifier>uuid:push-1</wscn:ScanIdentifier>"),
            "{req}"
        );
        assert!(
            req.contains("<wscn:ClientContext>ctx-a</wscn:ClientContext>"),
            "{req}"
        );
        assert!(
            req.contains(&format!("{SCAN_NS_AIRSCAN}/ScanAvailableEvent")),
            "{req}"
        );
    }
}
