//! Minimal WS-Discovery (UDP multicast) for a fake scanner.
//!
//! Windows looks for WSD devices on `239.255.255.250:3702`. We answer Probe
//! with a ProbeMatch that points at our HTTP Scan Service XAddrs.
//!
//! This is enough for discovery traffic; full install still needs a working
//! WS-Scan SOAP endpoint (see `wsd.rs`).

use std::net::{Ipv4Addr, SocketAddr};

use anyhow::{Context, Result};
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};
use uuid::Uuid;

const MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const PORT: u16 = 3702;

/// WSD Scan device type Windows expects for network scanners.
const SCANNER_TYPE: &str =
    "http://schemas.microsoft.com/windows/2006/08/wdp/scan/ScannerServiceType";

pub struct DiscoveryConfig {
    pub endpoint: String,
    pub device_uuid: Uuid,
    /// Reserved for ThisDevice metadata once WS-Transfer Get is implemented.
    #[allow(dead_code)]
    pub friendly_name: String,
}

pub async fn run(cfg: DiscoveryConfig) -> Result<()> {
    let local = SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT));
    let sock = UdpSocket::bind(local)
        .await
        .with_context(|| format!("bind WS-Discovery {local}"))?;
    sock.set_multicast_loop_v4(true)?;
    sock.join_multicast_v4(MULTICAST, Ipv4Addr::UNSPECIFIED)
        .context("join 239.255.255.250")?;

    info!(
        endpoint = %cfg.endpoint,
        uuid = %cfg.device_uuid,
        "WS-Discovery listening on {}:{}",
        MULTICAST,
        PORT
    );

    // Announce once so local tools see us without waiting for Probe.
    let hello = build_hello(&cfg);
    let _ = sock
        .send_to(hello.as_bytes(), SocketAddr::from((MULTICAST, PORT)))
        .await;

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        let text = String::from_utf8_lossy(&buf[..n]);
        if !text.contains("Probe") {
            debug!(%peer, bytes = n, "discovery datagram (ignored)");
            continue;
        }
        // Answer general Device probes and scanner-specific ones.
        if text.contains("ScannerServiceType")
            || text.contains("wsdp:Device")
            || text.contains(":Device")
        {
            let relates = extract_message_id(&text).unwrap_or_else(Uuid::new_v4);
            let match_xml = build_probe_match(&cfg, relates);
            if let Err(e) = sock.send_to(match_xml.as_bytes(), peer).await {
                warn!(%peer, error = %e, "ProbeMatch send failed");
            } else {
                info!(%peer, "answered Probe with ProbeMatch");
            }
        }
    }
}

fn extract_message_id(probe: &str) -> Option<Uuid> {
    for tag in ["MessageID>", "MessageId>"] {
        if let Some(i) = probe.find(tag) {
            let rest = &probe[i + tag.len()..];
            let end = rest.find('<')?;
            let raw = rest[..end].trim().trim_start_matches("urn:uuid:");
            return Uuid::parse_str(raw).ok();
        }
    }
    None
}

fn build_hello(cfg: &DiscoveryConfig) -> String {
    let msg_id = Uuid::new_v4();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
               xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
               xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
               xmlns:wsdp="http://schemas.xmlsoap.org/ws/2006/02/devprof"
               xmlns:pub="http://schemas.microsoft.com/windows/pub/2005/07">
  <soap:Header>
    <wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Hello</wsa:Action>
    <wsa:MessageID>urn:uuid:{msg_id}</wsa:MessageID>
    <wsd:AppSequence InstanceId="1" MessageNumber="1"/>
  </soap:Header>
  <soap:Body>
    <wsd:Hello>
      <wsa:EndpointReference>
        <wsa:Address>urn:uuid:{uuid}</wsa:Address>
      </wsa:EndpointReference>
      <wsd:Types>wsdp:Device {scan_type}</wsd:Types>
      <wsd:XAddrs>{xaddrs}</wsd:XAddrs>
      <wsd:MetadataVersion>1</wsd:MetadataVersion>
    </wsd:Hello>
  </soap:Body>
</soap:Envelope>"#,
        msg_id = msg_id,
        uuid = cfg.device_uuid,
        scan_type = SCANNER_TYPE,
        xaddrs = cfg.endpoint,
    )
}

fn build_probe_match(cfg: &DiscoveryConfig, relates: Uuid) -> String {
    let msg_id = Uuid::new_v4();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
               xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
               xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
               xmlns:wsdp="http://schemas.xmlsoap.org/ws/2006/02/devprof">
  <soap:Header>
    <wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/ProbeMatches</wsa:Action>
    <wsa:MessageID>urn:uuid:{msg_id}</wsa:MessageID>
    <wsa:RelatesTo>urn:uuid:{relates}</wsa:RelatesTo>
    <wsd:AppSequence InstanceId="1" MessageNumber="2"/>
  </soap:Header>
  <soap:Body>
    <wsd:ProbeMatches>
      <wsd:ProbeMatch>
        <wsa:EndpointReference>
          <wsa:Address>urn:uuid:{uuid}</wsa:Address>
        </wsa:EndpointReference>
        <wsd:Types>wsdp:Device {scan_type}</wsd:Types>
        <wsd:XAddrs>{xaddrs}</wsd:XAddrs>
        <wsd:MetadataVersion>1</wsd:MetadataVersion>
      </wsd:ProbeMatch>
    </wsd:ProbeMatches>
  </soap:Body>
</soap:Envelope>"#,
        msg_id = msg_id,
        relates = relates,
        uuid = cfg.device_uuid,
        scan_type = SCANNER_TYPE,
        xaddrs = cfg.endpoint,
    )
}
