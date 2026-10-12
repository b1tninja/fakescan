//! GetScannerElements response builders from a capability profile.

use crate::image_gen::{LETTER_H_THOU, LETTER_W_THOU};
use crate::profile::Profile;
use crate::wsd::soap;

pub fn requested_names(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut idx = 0;
    while let Some(rel) = body[idx..].find("Name>") {
        let at = idx + rel;
        let before = body.as_bytes().get(at.wrapping_sub(1)).copied();
        // `<…:Name>` or `<Name>` — not ElementData Name= attribute
        if matches!(before, Some(b'<') | Some(b':')) {
            let after = &body[at + "Name>".len()..];
            if let Some(end) = after.find('<') {
                let v = after[..end].trim();
                if !v.is_empty() && !v.contains('=') {
                    names.push(v.to_string());
                }
            }
        }
        idx = at + 5;
    }
    names
}

pub fn get_scanner_elements(profile: &Profile, feeder_ready: bool, body: &str) -> String {
    let names = requested_names(body);
    let want_all = names.is_empty();
    let mut parts = String::new();

    let want = |suffix: &str| {
        want_all
            || names
                .iter()
                .any(|n| n.ends_with(suffix) || n == suffix || n.ends_with(&format!(":{suffix}")))
    };

    if want("ScannerDescription") {
        parts.push_str(&element_data(
            "wscn:ScannerDescription",
            &format!(
                r#"        <wscn:ScannerDescription>
          <wscn:ScannerName xml:lang="en-US">{label}</wscn:ScannerName>
          <wscn:ScannerInfo xml:lang="en-US">fakescan profile {id}</wscn:ScannerInfo>
          <wscn:ScannerLocation xml:lang="en-US">lab</wscn:ScannerLocation>
        </wscn:ScannerDescription>"#,
                label = soap::esc(&profile.label),
                id = soap::esc(&profile.id),
            ),
        ));
    }
    if want("ScannerStatus") {
        // Keep Idle so clients will still CreateScanJob; reason hints feeder emptiness.
        let reason = if feeder_ready {
            "None"
        } else {
            "AttentionRequired"
        };
        parts.push_str(&element_data(
            "wscn:ScannerStatus",
            &format!(
                r#"        <wscn:ScannerStatus>
          <wscn:ScannerCurrentTime>{}</wscn:ScannerCurrentTime>
          <wscn:ScannerState>Idle</wscn:ScannerState>
          <wscn:ScannerStateReasons>
            <wscn:ScannerStateReason>{reason}</wscn:ScannerStateReason>
          </wscn:ScannerStateReasons>
        </wscn:ScannerStatus>"#,
                now_iso(),
            ),
        ));
    }
    if want("ScannerConfiguration") {
        parts.push_str(&element_data(
            "wscn:ScannerConfiguration",
            &scanner_configuration(profile),
        ));
    }
    if want("DefaultScanTicket") {
        parts.push_str(&element_data(
            "wscn:DefaultScanTicket",
            &default_scan_ticket(profile),
        ));
    }

    if parts.is_empty() {
        // Unknown names → Valid=false stubs + full config so clients still progress.
        for n in &names {
            parts.push_str(&format!(
                r#"      <wscn:ElementData wscn:Name="{}" wscn:Valid="false"/>
"#,
                soap::esc(n)
            ));
        }
        parts.push_str(&element_data(
            "wscn:ScannerConfiguration",
            &scanner_configuration(profile),
        ));
    }

    format!(
        r#"    <wscn:GetScannerElementsResponse>
      <wscn:ScannerElements>
{parts}      </wscn:ScannerElements>
    </wscn:GetScannerElementsResponse>"#
    )
}

fn element_data(name: &str, inner: &str) -> String {
    format!(
        r#"      <wscn:ElementData wscn:Name="{name}" wscn:Valid="true">
{inner}
      </wscn:ElementData>
"#
    )
}

fn scanner_configuration(profile: &Profile) -> String {
    let formats = profile
        .formats
        .iter()
        .map(|f| {
            let v = match f.as_str() {
                "jpeg" => "jfif",
                "tiff" => "tiff-single-uncompressed",
                "bmp" => "dib",
                other => other,
            };
            format!("            <wscn:FormatValue>{v}</wscn:FormatValue>\n")
        })
        .collect::<String>();

    let widths = profile
        .dpi
        .iter()
        .map(|d| format!("              <wscn:Width>{d}</wscn:Width>\n"))
        .collect::<String>();
    let heights = profile
        .dpi
        .iter()
        .map(|d| format!("              <wscn:Height>{d}</wscn:Height>\n"))
        .collect::<String>();

    let (ox, oy) = match &profile.optical {
        Some(o) => (o.x_dpi, o.y_dpi),
        None => (
            *profile.dpi.last().unwrap_or(&300),
            *profile.dpi.last().unwrap_or(&300),
        ),
    };

    let has_feeder = profile.sources.iter().any(|s| s == "feeder");
    let has_glass = profile.sources.iter().any(|s| s == "glass");

    let platen = if has_glass {
        format!(
            r#"        <wscn:Platen>
          <wscn:PlatenOpticalResolution>
            <wscn:Width>{ox}</wscn:Width>
            <wscn:Height>{oy}</wscn:Height>
          </wscn:PlatenOpticalResolution>
          <wscn:PlatenResolutions>
            <wscn:Widths>
{widths}            </wscn:Widths>
            <wscn:Heights>
{heights}            </wscn:Heights>
          </wscn:PlatenResolutions>
          <wscn:PlatenColor>
            <wscn:ColorEntry>RGB24</wscn:ColorEntry>
            <wscn:ColorEntry>Grayscale8</wscn:ColorEntry>
          </wscn:PlatenColor>
          <wscn:PlatenMinimumSize>
            <wscn:Width>1000</wscn:Width>
            <wscn:Height>1000</wscn:Height>
          </wscn:PlatenMinimumSize>
          <wscn:PlatenMaximumSize>
            <wscn:Width>{LETTER_W_THOU}</wscn:Width>
            <wscn:Height>{LETTER_H_THOU}</wscn:Height>
          </wscn:PlatenMaximumSize>
        </wscn:Platen>
"#
        )
    } else {
        String::new()
    };

    let adf = if has_feeder {
        format!(
            r#"        <wscn:ADF>
          <wscn:ADFSupportsDuplex>false</wscn:ADFSupportsDuplex>
          <wscn:ADFFront>
            <wscn:ADFOpticalResolution>
              <wscn:Width>{ox}</wscn:Width>
              <wscn:Height>{oy}</wscn:Height>
            </wscn:ADFOpticalResolution>
            <wscn:ADFResolutions>
              <wscn:Widths>
{widths}              </wscn:Widths>
              <wscn:Heights>
{heights}              </wscn:Heights>
            </wscn:ADFResolutions>
            <wscn:ADFColor>
              <wscn:ColorEntry>RGB24</wscn:ColorEntry>
              <wscn:ColorEntry>Grayscale8</wscn:ColorEntry>
            </wscn:ADFColor>
            <wscn:ADFMinimumSize>
              <wscn:Width>4000</wscn:Width>
              <wscn:Height>6000</wscn:Height>
            </wscn:ADFMinimumSize>
            <wscn:ADFMaximumSize>
              <wscn:Width>{LETTER_W_THOU}</wscn:Width>
              <wscn:Height>{LETTER_H_THOU}</wscn:Height>
            </wscn:ADFMaximumSize>
          </wscn:ADFFront>
        </wscn:ADF>
"#
        )
    } else {
        String::new()
    };

    format!(
        r#"        <wscn:ScannerConfiguration>
          <wscn:DeviceSettings>
            <wscn:FormatsSupported>
{formats}            </wscn:FormatsSupported>
            <wscn:CompressionQualityFactorSupported>
              <wscn:MinValue>1</wscn:MinValue>
              <wscn:MaxValue>100</wscn:MaxValue>
            </wscn:CompressionQualityFactorSupported>
            <wscn:ContentTypesSupported>
              <wscn:ContentTypeValue>Auto</wscn:ContentTypeValue>
              <wscn:ContentTypeValue>Text</wscn:ContentTypeValue>
              <wscn:ContentTypeValue>Photo</wscn:ContentTypeValue>
            </wscn:ContentTypesSupported>
            <wscn:DocumentSizeAutoDetectSupported>true</wscn:DocumentSizeAutoDetectSupported>
            <wscn:AutoExposureSupported>false</wscn:AutoExposureSupported>
            <wscn:BrightnessSupported>false</wscn:BrightnessSupported>
            <wscn:ContrastSupported>false</wscn:ContrastSupported>
            <wscn:RotationsSupported>
              <wscn:RotationValue>0</wscn:RotationValue>
            </wscn:RotationsSupported>
          </wscn:DeviceSettings>
{platen}{adf}        </wscn:ScannerConfiguration>"#
    )
}

fn now_iso() -> String {
    // Clients only need a parseable timestamp; wall-clock accuracy is irrelevant.
    "2026-01-01T00:00:00Z".into()
}

fn default_scan_ticket(profile: &Profile) -> String {
    let dpi = profile
        .dpi
        .iter()
        .find(|&&d| d == 200)
        .copied()
        .unwrap_or(profile.dpi.first().copied().unwrap_or(200));
    let format = match profile.formats.first().map(|s| s.as_str()) {
        Some("tiff") => "tiff-single-uncompressed",
        Some("bmp") => "dib",
        _ => "jfif",
    };
    format!(
        r#"        <wscn:DefaultScanTicket>
          <wscn:JobDescription>
            <wscn:JobName>fakescan</wscn:JobName>
            <wscn:JobOriginatingUserName>fakescan</wscn:JobOriginatingUserName>
          </wscn:JobDescription>
          <wscn:DocumentParameters>
            <wscn:Format>{format}</wscn:Format>
            <wscn:CompressionQualityFactor>80</wscn:CompressionQualityFactor>
            <wscn:InputSource>Platen</wscn:InputSource>
            <wscn:ContentType>Auto</wscn:ContentType>
            <wscn:InputSize>
              <wscn:DocumentSizeAutoDetect>true</wscn:DocumentSizeAutoDetect>
            </wscn:InputSize>
            <wscn:MediaSides>
              <wscn:MediaFront>
                <wscn:ColorProcessing>RGB24</wscn:ColorProcessing>
                <wscn:Resolution>
                  <wscn:Width>{dpi}</wscn:Width>
                  <wscn:Height>{dpi}</wscn:Height>
                </wscn:Resolution>
              </wscn:MediaFront>
            </wscn:MediaSides>
          </wscn:DocumentParameters>
        </wscn:DefaultScanTicket>"#
    )
}
