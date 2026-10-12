//! GetScannerElements response builders from a capability profile.

use crate::image_gen::{LETTER_H_THOU, LETTER_W_THOU};
use crate::profile::Profile;
use crate::state::{encodable_formats, ConditionHistoryEntry, ScanJob};
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

pub struct ScannerView<'a> {
    pub profile: &'a Profile,
    pub jam: bool,
    pub cover_open: bool,
    /// Double-feed / multi-pick (`MultipleFeedError`).
    pub multi_feed: bool,
    /// Lamp failure (`LampError`).
    pub lamp_error: bool,
    /// Device busy — Status `Processing` even with no job.
    pub busy: bool,
    /// Sticky offline (AttentionRequired + Create/Retrieve refuse).
    pub offline: bool,
    /// Lab sticky: `Calibrating` reason + DeviceCondition Id=5.
    pub calibrating: bool,
    /// Lab sticky: `InterlockOpen` Id=6.
    pub interlock_open: bool,
    /// Lab sticky: `InternalStorageFull` Id=7.
    pub storage_full: bool,
    /// Lab sticky: `Paused` reason only (Processing).
    pub paused: bool,
    /// Lab sticky: Condition `LampWarning` Id=8 + reason `AttentionRequired`.
    pub lamp_warning: bool,
    /// Transient / lab: `LampWarming` reason.
    pub lamp_warming: bool,
    /// Feeder sensor (for derived `InputTrayEmpty` when ADF is configured).
    pub feeder_ready: bool,
    pub pages_remaining: u32,
    /// True while a CreateScanJob is waiting for RetrieveImage.
    pub scanning: bool,
    /// Cleared sticky faults for optional `ConditionHistory` (omit when empty).
    pub condition_history: &'a [ConditionHistoryEntry],
}

pub fn get_scanner_elements(view: ScannerView<'_>, body: &str) -> String {
    let names = requested_names(body);
    let want_all = names.is_empty();
    let mut parts = String::new();

    // Echo the client's QName (e.g. sca:ScannerDescription) on ElementData/@Name —
    // WIA asks with the 2006/08 prefix and rejects a mismatched Name.
    let client_name = |suffix: &str| -> String {
        names
            .iter()
            .find(|n| n.ends_with(suffix) || *n == suffix || n.ends_with(&format!(":{suffix}")))
            .cloned()
            .unwrap_or_else(|| format!("wscn:{suffix}"))
    };
    let want = |suffix: &str| {
        want_all
            || names
                .iter()
                .any(|n| n.ends_with(suffix) || n == suffix || n.ends_with(&format!(":{suffix}")))
    };

    if want("ScannerDescription") {
        parts.push_str(&element_data(
            &client_name("ScannerDescription"),
            &scanner_description(view.profile),
        ));
    }
    if want("ScannerStatus") {
        parts.push_str(&element_data(
            &client_name("ScannerStatus"),
            &scanner_status(&view),
        ));
    }
    if want("ScannerConfiguration") {
        parts.push_str(&element_data(
            &client_name("ScannerConfiguration"),
            &scanner_configuration(view.profile),
        ));
    }
    if want("DefaultScanTicket") {
        parts.push_str(&element_data(
            &client_name("DefaultScanTicket"),
            &default_scan_ticket(view.profile),
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
            &scanner_configuration(view.profile),
        ));
    }

    format!(
        r#"    <wscn:GetScannerElementsResponse>
      <wscn:ScannerElements>
{parts}      </wscn:ScannerElements>
    </wscn:GetScannerElementsResponse>"#
    )
}

pub fn get_job_elements(job: &ScanJob, body: &str) -> String {
    let names = requested_names(body);
    let want_all = names.is_empty();
    let want = |suffix: &str| {
        want_all
            || names
                .iter()
                .any(|n| n.ends_with(suffix) || n == suffix || n.ends_with(&format!(":{suffix}")))
    };

    let mut parts = String::new();
    if want("JobStatus") || want_all {
        parts.push_str(&element_data("wscn:JobStatus", &job_status_xml(job)));
    }
    if want("JobToken") || want_all {
        // JobElementsEntry is JobStatus, ScanTicket, or Document. JobToken is not one of them.
        parts.push_str(
            r#"      <wscn:ElementData Name="wscn:JobToken" Valid="false">
      </wscn:ElementData>
"#,
        );
    }
    // Documents / DocumentFinalParameters: Format, dpi, pages_left from ScanJob.
    // Documents is the WKV name; DocumentFinalParameters is accepted as a lab alias.
    if want("Documents") || want("DocumentFinalParameters") || want_all {
        let name = if want("Documents") && !want_all {
            "wscn:Documents"
        } else if want("DocumentFinalParameters") && !want("Documents") && !want_all {
            "wscn:DocumentFinalParameters"
        } else {
            "wscn:Documents"
        };
        let inner = if name.ends_with("DocumentFinalParameters") {
            document_final_parameters_xml(job)
        } else {
            format!(
                r#"        <wscn:Documents>
{}
        </wscn:Documents>"#,
                document_final_parameters_xml(job)
            )
        };
        parts.push_str(&element_data(name, &inner));
    }

    if parts.is_empty() {
        for n in &names {
            parts.push_str(&format!(
                r#"      <wscn:ElementData wscn:Name="{}" wscn:Valid="false"/>
"#,
                soap::esc(n)
            ));
        }
        // Always include status so clients can progress.
        parts.push_str(&element_data("wscn:JobStatus", &job_status_xml(job)));
    }

    format!(
        r#"    <wscn:GetJobElementsResponse>
      <wscn:JobElements>
{parts}      </wscn:JobElements>
    </wscn:GetJobElementsResponse>"#
    )
}

fn job_status_xml(job: &ScanJob) -> String {
    // JobStatusType: JobId, JobState, JobStateReasons, ScansCompleted. `xs:any` is another namespace.
    format!(
        r#"        <wscn:JobStatus>
          <wscn:JobId>{id}</wscn:JobId>
          <wscn:JobState>{state}</wscn:JobState>
          <wscn:JobStateReasons>
            <wscn:JobStateReason>{reason}</wscn:JobStateReason>
          </wscn:JobStateReasons>
          <wscn:ScansCompleted>{done}</wscn:ScansCompleted>
        </wscn:JobStatus>"#,
        id = job.id,
        state = job.job_state(),
        reason = soap::esc(&job.state_reason),
        done = job.page_index,
    )
}

fn document_final_parameters_xml(job: &ScanJob) -> String {
    format!(
        r#"        <wscn:DocumentFinalParameters>
          <wscn:Format>{fmt}</wscn:Format>
          <wscn:ImagesToTransfer>{left}</wscn:ImagesToTransfer>
          <wscn:InputSource>{src}</wscn:InputSource>
          <wscn:MediaSides>
            <wscn:MediaFront>
              <wscn:Resolution>
                <wscn:Width>{dpi}</wscn:Width>
                <wscn:Height>{dpi}</wscn:Height>
              </wscn:Resolution>
            </wscn:MediaFront>
          </wscn:MediaSides>
        </wscn:DocumentFinalParameters>"#,
        fmt = soap::esc(&job.format),
        left = job.pages_left,
        src = soap::esc(&job.input_source),
        dpi = job.dpi,
    )
}

fn scanner_status(view: &ScannerView<'_>) -> String {
    let jam = view.jam;
    let cover_open = view.cover_open;
    let multi_feed = view.multi_feed;
    let lamp_error = view.lamp_error;
    let busy = view.busy;
    let offline = view.offline;
    let calibrating = view.calibrating;
    let interlock_open = view.interlock_open;
    let storage_full = view.storage_full;
    let paused = view.paused;
    let lamp_warning = view.lamp_warning;
    let lamp_warming = view.lamp_warming;
    let scanning = view.scanning;
    let condition_history = view.condition_history;

    let mut reasons = Vec::new();
    if jam {
        reasons.push("MediaJam");
    }
    if cover_open {
        reasons.push("CoverOpen");
    }
    if multi_feed {
        reasons.push("MultipleFeedError");
    }
    if lamp_error {
        reasons.push("LampError");
    }
    if calibrating {
        reasons.push("Calibrating");
    }
    if interlock_open {
        reasons.push("InterlockOpen");
    }
    if storage_full {
        reasons.push("InternalStorageFull");
    }
    if lamp_warming {
        reasons.push("LampWarming");
    }
    if paused {
        reasons.push("Paused");
    }
    if offline || lamp_warning {
        reasons.push("AttentionRequired");
    }
    // Empty ADF is Idle/None — AttentionRequired means human intervention and
    // must not be used for feeder_ready=false (glass acquire stays online).
    // Busy alone keeps reason None with state Processing.
    if reasons.is_empty() {
        reasons.push("None");
    }
    // After CreateScanJob, WIA polls Status — Idle here aborts before RetrieveImage.
    let state = if jam
        || cover_open
        || multi_feed
        || lamp_error
        || offline
        || interlock_open
        || storage_full
        || lamp_warning
    {
        "Stopped"
    } else if scanning || busy || calibrating || paused || lamp_warming {
        "Processing"
    } else {
        "Idle"
    };
    let reason_xml = reasons
        .iter()
        .map(|r| format!("            <wscn:ScannerStateReason>{r}</wscn:ScannerStateReason>\n"))
        .collect::<String>();
    // ConditionName enum has no AttentionRequired / Paused / LampWarming — those stay reasons only.
    // Stable Ids: 1 MediaJam, 2 CoverOpen, 3 MultipleFeedError, 4 LampError, 5 Calibrating,
    // 6 InterlockOpen, 7 InternalStorageFull, 8 LampWarning, 9 InputTrayEmpty (derived).
    let time = now_iso();
    let mut conditions = String::new();
    if jam {
        conditions.push_str(&device_condition(1, &time, "MediaJam", "ADF", "Critical"));
    }
    if cover_open {
        conditions.push_str(&device_condition(
            2,
            &time,
            "CoverOpen",
            "Platen",
            "Critical",
        ));
    }
    if multi_feed {
        conditions.push_str(&device_condition(
            3,
            &time,
            "MultipleFeedError",
            "ADF",
            "Critical",
        ));
    }
    if lamp_error {
        conditions.push_str(&device_condition(
            4,
            &time,
            "LampError",
            "Platen",
            "Critical",
        ));
    }
    if calibrating {
        conditions.push_str(&device_condition(
            5,
            &time,
            "Calibrating",
            "Platen",
            "Informational",
        ));
    }
    if interlock_open {
        conditions.push_str(&device_condition(
            6,
            &time,
            "InterlockOpen",
            "MediaPath",
            "Critical",
        ));
    }
    if storage_full {
        conditions.push_str(&device_condition(
            7,
            &time,
            "InternalStorageFull",
            "Platen",
            "Critical",
        ));
    }
    if lamp_warning {
        conditions.push_str(&device_condition(
            8,
            &time,
            "LampWarning",
            "Platen",
            "Warning",
        ));
    }
    let has_feeder = view.profile.sources.iter().any(|s| s == "feeder");
    let input_tray_empty = has_feeder && !view.feeder_ready && view.pages_remaining == 0;
    if input_tray_empty {
        // Derived only — do not emit AttentionRequired for empty ADF (Brother WIA).
        conditions.push_str(&device_condition(
            9,
            &time,
            "InputTrayEmpty",
            "ADF",
            "Warning",
        ));
    }
    let active = if conditions.is_empty() {
        "          <wscn:ActiveConditions/>\n".to_string()
    } else {
        format!(
            "          <wscn:ActiveConditions>\n{conditions}          </wscn:ActiveConditions>\n"
        )
    };
    // ConditionHistory is minOccurs=0 — omit the element when the ring is empty.
    let history = if condition_history.is_empty() {
        String::new()
    } else {
        let mut entries = String::new();
        for e in condition_history {
            entries.push_str(&condition_history_entry(e));
        }
        format!("          <wscn:ConditionHistory>\n{entries}          </wscn:ConditionHistory>\n")
    };
    format!(
        r#"        <wscn:ScannerStatus>
          <wscn:ScannerCurrentTime>{time}</wscn:ScannerCurrentTime>
          <wscn:ScannerState>{state}</wscn:ScannerState>
          <wscn:ScannerStateReasons>
{reason_xml}          </wscn:ScannerStateReasons>
{active}{history}        </wscn:ScannerStatus>"#
    )
}

fn device_condition(id: u32, time: &str, name: &str, component: &str, severity: &str) -> String {
    format!(
        r#"            <wscn:DeviceCondition Id="{id}">
              <wscn:Time>{time}</wscn:Time>
              <wscn:Name>{name}</wscn:Name>
              <wscn:Component>{component}</wscn:Component>
              <wscn:Severity>{severity}</wscn:Severity>
            </wscn:DeviceCondition>
"#
    )
}

fn condition_history_entry(e: &ConditionHistoryEntry) -> String {
    format!(
        r#"            <wscn:ConditionHistoryEntry Id="{id}">
              <wscn:Time>{time}</wscn:Time>
              <wscn:Name>{name}</wscn:Name>
              <wscn:Component>{component}</wscn:Component>
              <wscn:Severity>{severity}</wscn:Severity>
              <wscn:ClearTime>{clear}</wscn:ClearTime>
            </wscn:ConditionHistoryEntry>
"#,
        id = e.id,
        time = soap::esc(&e.time_iso),
        name = soap::esc(e.name),
        component = soap::esc(e.component),
        severity = soap::esc(e.severity),
        clear = soap::esc(&e.clear_time_iso),
    )
}

fn element_data(name: &str, inner: &str) -> String {
    // Live wsdscdrv: unprefixed Name/Valid + client QName (sca:…). Prefixed
    // wscn:Name stops Connect after Description (no Ticket/Config).
    format!(
        r#"      <wscn:ElementData Name="{name}" Valid="true">
{inner}
      </wscn:ElementData>
"#
    )
}

/// WSD `ColorEntry` list from profile `colors` (canonical wsd_token order).
/// Empty allow-list → Brother-shaped BW → Gray → RGB.
fn color_entries_xml(profile: &Profile, indent: &str) -> String {
    use crate::effects::ColorMode;
    let tokens: Vec<String> = if profile.colors.is_empty() {
        vec![
            ColorMode::Bw1.wsd_token().to_string(),
            ColorMode::Gray8.wsd_token().to_string(),
            ColorMode::Rgb24.wsd_token().to_string(),
        ]
    } else {
        // Preserve profile order; canonicalize via ColorMode so Gray8 → Grayscale8.
        let mut out = Vec::new();
        for raw in &profile.colors {
            let tok = ColorMode::parse(raw).wsd_token().to_string();
            if !out.iter().any(|t| t == &tok) {
                out.push(tok);
            }
        }
        out
    };
    tokens
        .into_iter()
        .map(|t| format!("{indent}<wscn:ColorEntry>{t}</wscn:ColorEntry>\n"))
        .collect()
}

fn default_color_processing(profile: &Profile) -> &'static str {
    use crate::effects::ColorMode;
    if let Some(c) = profile.colors.first() {
        ColorMode::parse(c).wsd_token()
    } else {
        ColorMode::Rgb24.wsd_token()
    }
}

fn scanner_configuration(profile: &Profile) -> String {
    // Shape matches a live Brother MFC-9330CDW GetScannerElements capture
    // (`D:\temp\brother-gse-config.xml`): 0/1 booleans, Auto-only content type,
    // CompressionQuality 100–100, Scaling 100–100, color order BW→Gray→RGB,
    // min size 556, platen max height LETTER_H_THOU (11700).
    let formats = encodable_formats(profile)
        .into_iter()
        .map(|v| format!("            <wscn:FormatValue>{v}</wscn:FormatValue>\n"))
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
    let platen_colors = color_entries_xml(profile, "            ");
    let adf_colors = color_entries_xml(profile, "              ");

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
{platen_colors}          </wscn:PlatenColor>
          <wscn:PlatenMinimumSize>
            <wscn:Width>556</wscn:Width>
            <wscn:Height>556</wscn:Height>
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
        let duplex = if profile.feeder.duplex { "1" } else { "0" };
        // ADFBack mirrors ADFFront when duplex is offered (WSD: back optional but
        // clients treat Front+Back as duplex even if SupportsDuplex is wrong).
        let adf_side = |tag: &str| {
            format!(
                r#"          <wscn:{tag}>
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
{colors}            </wscn:ADFColor>
            <wscn:ADFMinimumSize>
              <wscn:Width>556</wscn:Width>
              <wscn:Height>556</wscn:Height>
            </wscn:ADFMinimumSize>
            <wscn:ADFMaximumSize>
              <wscn:Width>{LETTER_W_THOU}</wscn:Width>
              <wscn:Height>14000</wscn:Height>
            </wscn:ADFMaximumSize>
          </wscn:{tag}>
"#,
                colors = adf_colors,
            )
        };
        let back = if profile.feeder.duplex {
            adf_side("ADFBack")
        } else {
            String::new()
        };
        format!(
            r#"        <wscn:ADF>
          <wscn:ADFSupportsDuplex>{duplex}</wscn:ADFSupportsDuplex>
{front}{back}        </wscn:ADF>
"#,
            front = adf_side("ADFFront"),
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
              <wscn:MinValue>100</wscn:MinValue>
              <wscn:MaxValue>100</wscn:MaxValue>
            </wscn:CompressionQualityFactorSupported>
            <wscn:ContentTypesSupported>
              <wscn:ContentTypeValue>Auto</wscn:ContentTypeValue>
            </wscn:ContentTypesSupported>
            <wscn:DocumentSizeAutoDetectSupported>0</wscn:DocumentSizeAutoDetectSupported>
            <wscn:AutoExposureSupported>0</wscn:AutoExposureSupported>
            <wscn:BrightnessSupported>1</wscn:BrightnessSupported>
            <wscn:ContrastSupported>1</wscn:ContrastSupported>
            <wscn:ScalingRangeSupported>
              <wscn:ScalingWidth>
                <wscn:MinValue>100</wscn:MinValue>
                <wscn:MaxValue>100</wscn:MaxValue>
              </wscn:ScalingWidth>
              <wscn:ScalingHeight>
                <wscn:MinValue>100</wscn:MinValue>
                <wscn:MaxValue>100</wscn:MaxValue>
              </wscn:ScalingHeight>
            </wscn:ScalingRangeSupported>
            <wscn:RotationsSupported>
              <wscn:RotationValue>0</wscn:RotationValue>
            </wscn:RotationsSupported>
          </wscn:DeviceSettings>
{platen}{adf}        </wscn:ScannerConfiguration>"#
    )
}

fn now_iso() -> String {
    crate::timeutil::utc_now_iso()
}

/// Same Description block as GetScannerElements (`ScannerName` / Info / Location).
fn scanner_description(profile: &Profile) -> String {
    format!(
        r#"        <wscn:ScannerDescription>
          <wscn:ScannerName xml:lang="en-US">{label}</wscn:ScannerName>
          <wscn:ScannerInfo xml:lang="en-US">fakescan profile {id}</wscn:ScannerInfo>
          <wscn:ScannerLocation xml:lang="en-US">lab</wscn:ScannerLocation>
        </wscn:ScannerDescription>"#,
        label = soap::esc(&profile.label),
        id = soap::esc(&profile.id),
    )
}

/// ElementChanges children for ScannerElementsChangeEvent.
///
/// Order: [`scanner_description`], full [`scanner_configuration`], then
/// [`default_scan_ticket`] — same shapes as GetScannerElements — so a NotifyTo
/// subscriber sees the advertised identity and capability set without re-polling.
pub(crate) fn element_changes_xml(profile: &Profile) -> String {
    format!(
        "{}\n{}\n{}",
        scanner_description(profile),
        scanner_configuration(profile),
        default_scan_ticket(profile)
    )
}

fn default_scan_ticket(profile: &Profile) -> String {
    // Brother default ticket (live): jfif @ 300, InputMediaSize 8500×11700,
    // Exposure/Scaling/Rotation/ScanRegion filled — not DocumentSizeAutoDetect.
    let dpi = profile
        .dpi
        .iter()
        .find(|&&d| d == 300)
        .copied()
        .or_else(|| profile.dpi.iter().find(|&&d| d == 200).copied())
        .unwrap_or(profile.dpi.first().copied().unwrap_or(200));
    let formats = encodable_formats(profile);
    let format = formats
        .iter()
        .find(|f| *f == "jfif")
        .cloned()
        .or_else(|| formats.into_iter().next())
        .unwrap_or_else(|| "jfif".into());
    let color = default_color_processing(profile);
    format!(
        r#"        <wscn:DefaultScanTicket>
          <wscn:JobDescription>
            <wscn:JobName>fakescan</wscn:JobName>
            <wscn:JobOriginatingUserName>fakescan</wscn:JobOriginatingUserName>
            <wscn:JobInformation>fakescan</wscn:JobInformation>
          </wscn:JobDescription>
          <wscn:DocumentParameters>
            <wscn:Format>{format}</wscn:Format>
            <wscn:CompressionQualityFactor>100</wscn:CompressionQualityFactor>
            <wscn:ImagesToTransfer>1</wscn:ImagesToTransfer>
            <wscn:InputSource>Platen</wscn:InputSource>
            <wscn:ContentType>Auto</wscn:ContentType>
            <wscn:InputSize>
              <wscn:InputMediaSize>
                <wscn:Width>{LETTER_W_THOU}</wscn:Width>
                <wscn:Height>{LETTER_H_THOU}</wscn:Height>
              </wscn:InputMediaSize>
            </wscn:InputSize>
            <wscn:Exposure>
              <wscn:ExposureSettings>
                <wscn:Contrast>0</wscn:Contrast>
                <wscn:Brightness>0</wscn:Brightness>
                <wscn:Sharpness>0</wscn:Sharpness>
              </wscn:ExposureSettings>
            </wscn:Exposure>
            <wscn:Scaling>
              <wscn:ScalingWidth>100</wscn:ScalingWidth>
              <wscn:ScalingHeight>100</wscn:ScalingHeight>
            </wscn:Scaling>
            <wscn:Rotation>0</wscn:Rotation>
            <wscn:MediaSides>
              <wscn:MediaFront>
                <wscn:ScanRegion>
                  <wscn:ScanRegionXOffset>0</wscn:ScanRegionXOffset>
                  <wscn:ScanRegionYOffset>0</wscn:ScanRegionYOffset>
                  <wscn:ScanRegionWidth>{LETTER_W_THOU}</wscn:ScanRegionWidth>
                  <wscn:ScanRegionHeight>{LETTER_H_THOU}</wscn:ScanRegionHeight>
                </wscn:ScanRegion>
                <wscn:ColorProcessing>{color}</wscn:ColorProcessing>
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
    use std::path::PathBuf;

    fn profile() -> Profile {
        load_profile("brother-wsd")
    }

    fn load_profile(id: &str) -> Profile {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
        Profile::load(&dir, id).expect("profile")
    }

    #[test]
    fn scanner_capabilities_follow_the_profile_and_validate() {
        let brother = load_profile("brother-wsd");
        let dib = load_profile("accept-dib");
        let ask = r#"<wscn:Name>wscn:ScannerConfiguration</wscn:Name>"#;
        let brother_xml = get_scanner_elements(idle_view(&brother), ask);
        let dib_xml = get_scanner_elements(idle_view(&dib), ask);
        assert!(brother_xml.contains("<wscn:FormatValue>jfif</wscn:FormatValue>"), "{brother_xml}");
        assert!(brother_xml.contains("tiff-single-uncompressed"), "{brother_xml}");
        assert!(!brother_xml.contains("<wscn:FormatValue>dib</wscn:FormatValue>"), "{brother_xml}");
        assert!(dib_xml.contains("<wscn:FormatValue>dib</wscn:FormatValue>"), "{dib_xml}");
        assert!(!dib_xml.contains("<wscn:FormatValue>jfif</wscn:FormatValue>"), "{dib_xml}");
        for xml in [&brother_xml, &dib_xml] {
            let wrapped = format!(
                r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/08/wdp/scan"><soap:Body>{xml}</soap:Body></soap:Envelope>"#
            );
            let problems = crate::wsd::scan_schema::body_problems(&wrapped);
            assert!(problems.is_empty(), "{problems:?}\n{xml}");
        }
    }

    fn idle_view<'a>(profile: &'a Profile) -> ScannerView<'a> {
        ScannerView {
            profile,
            jam: false,
            cover_open: false,
            multi_feed: false,
            lamp_error: false,
            busy: false,
            offline: false,
            calibrating: false,
            interlock_open: false,
            storage_full: false,
            paused: false,
            lamp_warning: false,
            lamp_warming: false,
            feeder_ready: true,
            pages_remaining: 0,
            scanning: false,
            condition_history: &[],
        }
    }

    const STATUS_REQ: &str = r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#;

    fn status_xml(jam: bool, cover_open: bool) -> String {
        let p = profile();
        let mut v = idle_view(&p);
        v.jam = jam;
        v.cover_open = cover_open;
        get_scanner_elements(v, STATUS_REQ)
    }

    #[test]
    fn scanner_status_processing_while_scanning() {
        let xml = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: true,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            xml.contains("<wscn:ScannerState>Processing</wscn:ScannerState>"),
            "{xml}"
        );
    }

    #[test]
    fn scanner_status_idle_reason_none_not_attention() {
        let xml = status_xml(false, false);
        assert!(
            xml.contains("<wscn:ScannerState>Idle</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>None</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("AttentionRequired"), "{xml}");
    }

    #[test]
    fn scanner_status_jam_is_media_jam() {
        let xml = status_xml(true, false);
        assert!(
            xml.contains("<wscn:ScannerState>Stopped</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>MediaJam</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("CoverOpen"), "{xml}");
    }

    #[test]
    fn scanner_status_jam_emits_active_condition() {
        let xml = status_xml(true, false);
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="1">"#), "{xml}");
        assert!(xml.contains("<wscn:Name>MediaJam</wscn:Name>"), "{xml}");
        assert!(
            xml.contains("<wscn:Component>ADF</wscn:Component>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:Severity>Critical</wscn:Severity>"),
            "{xml}"
        );
        let time = xml
            .split_once("<wscn:Time>")
            .and_then(|(_, rest)| rest.split_once("</wscn:Time>"))
            .map(|(t, _)| t)
            .expect("Time element");
        assert!(
            crate::timeutil::is_utc_iso(time),
            "Time should be YYYY-MM-DDTHH:MM:SSZ, got {time:?} in {xml}"
        );
        assert!(!xml.contains(r#"Id="2""#), "{xml}");
        let idle = status_xml(false, false);
        assert!(
            idle.contains("<wscn:ActiveConditions/>"),
            "empty faults stay self-closing: {idle}"
        );
        assert!(!idle.contains("DeviceCondition"), "{idle}");
        assert!(
            !idle.contains("ConditionHistory"),
            "empty history omits ConditionHistory: {idle}"
        );
    }

    #[test]
    fn scanner_status_emits_condition_history_when_present() {
        let hist = [ConditionHistoryEntry {
            id: 1,
            name: "MediaJam",
            component: "ADF",
            severity: "Critical",
            time_iso: "2026-03-15T12:30:00Z".into(),
            clear_time_iso: "2026-03-15T12:31:05Z".into(),
        }];
        let xml = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &hist,
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(xml.contains("<wscn:ConditionHistory>"), "{xml}");
        assert!(
            xml.contains(r#"<wscn:ConditionHistoryEntry Id="1">"#),
            "{xml}"
        );
        assert!(xml.contains("<wscn:Name>MediaJam</wscn:Name>"), "{xml}");
        let clear = xml
            .split_once("<wscn:ClearTime>")
            .and_then(|(_, rest)| rest.split_once("</wscn:ClearTime>"))
            .map(|(t, _)| t)
            .expect("ClearTime element");
        assert!(
            crate::timeutil::is_utc_iso(clear),
            "ClearTime should be YYYY-MM-DDTHH:MM:SSZ, got {clear:?} in {xml}"
        );
        assert_eq!(clear, hist[0].clear_time_iso);
    }

    #[test]
    fn scanner_status_cover_open() {
        let xml = status_xml(false, true);
        assert!(
            xml.contains("<wscn:ScannerState>Stopped</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>CoverOpen</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("MediaJam"), "{xml}");
    }

    #[test]
    fn scanner_status_multi_feed_is_multiple_feed_error() {
        let xml = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: true,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            xml.contains("<wscn:ScannerState>Stopped</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>MultipleFeedError</wscn:ScannerStateReason>"),
            "{xml}"
        );
    }

    #[test]
    fn scanner_status_lamp_error_and_busy_processing() {
        let lamp = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: true,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            lamp.contains("<wscn:ScannerState>Stopped</wscn:ScannerState>"),
            "{lamp}"
        );
        assert!(
            lamp.contains("<wscn:ScannerStateReason>LampError</wscn:ScannerStateReason>"),
            "{lamp}"
        );
        let busy = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: true,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            busy.contains("<wscn:ScannerState>Processing</wscn:ScannerState>"),
            "{busy}"
        );
        assert!(
            busy.contains("<wscn:ScannerStateReason>None</wscn:ScannerStateReason>"),
            "{busy}"
        );
    }

    #[test]
    fn scanner_status_calibrating_active_condition() {
        let p = profile();
        let mut v = idle_view(&p);
        v.calibrating = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerState>Processing</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>Calibrating</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="5">"#), "{xml}");
        assert!(xml.contains("<wscn:Name>Calibrating</wscn:Name>"), "{xml}");
        assert!(
            xml.contains("<wscn:Severity>Informational</wscn:Severity>"),
            "{xml}"
        );
    }

    #[test]
    fn scanner_status_interlock_and_storage_full() {
        let p = profile();
        let mut v = idle_view(&p);
        v.interlock_open = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerStateReason>InterlockOpen</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="6">"#), "{xml}");
        assert!(
            xml.contains("<wscn:Component>MediaPath</wscn:Component>"),
            "{xml}"
        );
        let mut v = idle_view(&p);
        v.storage_full = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerStateReason>InternalStorageFull</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="7">"#), "{xml}");
    }

    #[test]
    fn scanner_status_paused_and_lamp_warming_reasons_only() {
        let p = profile();
        let mut v = idle_view(&p);
        v.paused = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerState>Processing</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>Paused</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("DeviceCondition"), "{xml}");
        let mut v = idle_view(&p);
        v.lamp_warming = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerStateReason>LampWarming</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("DeviceCondition"), "{xml}");
    }

    #[test]
    fn scanner_status_lamp_warning_attention_and_condition() {
        let p = profile();
        let mut v = idle_view(&p);
        v.lamp_warning = true;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerState>Stopped</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>AttentionRequired</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="8">"#), "{xml}");
        assert!(xml.contains("<wscn:Name>LampWarning</wscn:Name>"), "{xml}");
        assert!(
            xml.contains("<wscn:Severity>Warning</wscn:Severity>"),
            "{xml}"
        );
        assert!(!xml.contains("LampError"), "{xml}");
    }

    #[test]
    fn scanner_status_input_tray_empty_without_attention() {
        let p = profile();
        assert!(
            p.sources.iter().any(|s| s == "feeder"),
            "brother-wsd should advertise feeder"
        );
        let mut v = idle_view(&p);
        v.feeder_ready = false;
        v.pages_remaining = 0;
        let xml = get_scanner_elements(v, STATUS_REQ);
        assert!(
            xml.contains("<wscn:ScannerState>Idle</wscn:ScannerState>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScannerStateReason>None</wscn:ScannerStateReason>"),
            "{xml}"
        );
        assert!(!xml.contains("AttentionRequired"), "{xml}");
        assert!(xml.contains(r#"<wscn:DeviceCondition Id="9">"#), "{xml}");
        assert!(
            xml.contains("<wscn:Name>InputTrayEmpty</wscn:Name>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:Component>ADF</wscn:Component>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:Severity>Warning</wscn:Severity>"),
            "{xml}"
        );
    }

    #[test]
    fn default_ticket_matches_brother_shape() {
        let xml = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:DefaultScanTicket</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(xml.contains("<wscn:InputMediaSize>"), "{xml}");
        assert!(xml.contains("<wscn:Height>11700</wscn:Height>"), "{xml}");
        assert!(
            xml.contains("<wscn:ScanRegionWidth>8500</wscn:ScanRegionWidth>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:CompressionQualityFactor>100</wscn:CompressionQualityFactor>"),
            "{xml}"
        );
        assert!(!xml.contains("DocumentSizeAutoDetect>"), "{xml}");
    }

    #[test]
    fn configuration_matches_brother_device_settings() {
        let xml = get_scanner_elements(
            ScannerView {
                profile: &profile(),
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerConfiguration</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            xml.contains(
                "<wscn:DocumentSizeAutoDetectSupported>0</wscn:DocumentSizeAutoDetectSupported>"
            ),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:BrightnessSupported>1</wscn:BrightnessSupported>"),
            "{xml}"
        );
        assert!(xml.contains("<wscn:Height>11700</wscn:Height>"), "{xml}");
        assert!(xml.contains("<wscn:MinValue>100</wscn:MinValue>"), "{xml}");
        assert!(
            xml.contains("<wscn:ColorEntry>BlackAndWhite1</wscn:ColorEntry>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ColorEntry>Grayscale8</wscn:ColorEntry>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ColorEntry>RGB24</wscn:ColorEntry>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ADFSupportsDuplex>1</wscn:ADFSupportsDuplex>"),
            "{xml}"
        );
        assert!(xml.contains("<wscn:ADFFront>"), "{xml}");
        assert!(xml.contains("<wscn:ADFBack>"), "{xml}");
    }

    #[test]
    fn configuration_color_entries_follow_profile_colors() {
        let mut p = profile();
        p.colors = vec!["RGB24".into()];
        let xml = get_scanner_elements(
            ScannerView {
                profile: &p,
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:ScannerConfiguration</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            xml.contains("<wscn:ColorEntry>RGB24</wscn:ColorEntry>"),
            "{xml}"
        );
        assert!(
            !xml.contains("<wscn:ColorEntry>BlackAndWhite1</wscn:ColorEntry>"),
            "BW omitted when not in profile colors: {xml}"
        );
        assert!(
            !xml.contains("<wscn:ColorEntry>Grayscale8</wscn:ColorEntry>"),
            "Gray omitted when not in profile colors: {xml}"
        );
        let ticket = get_scanner_elements(
            ScannerView {
                profile: &p,
                jam: false,
                cover_open: false,
                multi_feed: false,
                lamp_error: false,
                busy: false,
                offline: false,
                calibrating: false,
                interlock_open: false,
                storage_full: false,
                paused: false,
                lamp_warning: false,
                lamp_warming: false,
                feeder_ready: true,
                pages_remaining: 0,
                scanning: false,
                condition_history: &[],
            },
            r#"<wscn:RequestedElements><wscn:Name>wscn:DefaultScanTicket</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            ticket.contains("<wscn:ColorProcessing>RGB24</wscn:ColorProcessing>"),
            "{ticket}"
        );
    }

    fn sample_job() -> ScanJob {
        use std::time::Instant;
        ScanJob {
            id: 7,
            token: "tok-7".into(),
            format: "jfif".into(),
            input_source: "Platen".into(),
            dpi: 200,
            color: crate::effects::ColorMode::Rgb24,
            width_px: 1700,
            height_px: 2200,
            pages_left: 2,
            page_index: 1,
            duplex: false,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            state_reason: "JobScanning".into(),
            aborted: false,
            format_used_default: false,
            format_overridden: false,
            dpi_used_default: false,
            dpi_overridden: false,
        }
    }

    #[test]
    fn job_elements_status_includes_reason_and_scans() {
        let xml = get_job_elements(
            &sample_job(),
            r#"<wscn:RequestedElements><wscn:Name>wscn:JobStatus</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(
            xml.contains("<wscn:JobStateReason>JobScanning</wscn:JobStateReason>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:ScansCompleted>1</wscn:ScansCompleted>"),
            "{xml}"
        );
        assert!(
            xml.contains("<wscn:JobState>Processing</wscn:JobState>"),
            "{xml}"
        );
    }

    #[test]
    fn job_elements_document_final_parameters() {
        let xml = get_job_elements(
            &sample_job(),
            r#"<wscn:RequestedElements><wscn:Name>wscn:DocumentFinalParameters</wscn:Name></wscn:RequestedElements>"#,
        );
        assert!(xml.contains("<wscn:Format>jfif</wscn:Format>"), "{xml}");
        assert!(
            xml.contains("<wscn:InputSource>Platen</wscn:InputSource>"),
            "{xml}"
        );
        assert!(xml.contains("<wscn:Width>200</wscn:Width>"), "{xml}");
        assert!(
            xml.contains("<wscn:ImagesToTransfer>2</wscn:ImagesToTransfer>"),
            "{xml}"
        );
        assert!(!xml.contains("JobStatus"), "{xml}");
    }
}
