//! SOAP response bodies built as [`xml::Element`] trees.

use crate::image_gen::{LETTER_H_THOU, LETTER_W_THOU};
use crate::state::{ScanJob, TicketValidation};
use crate::wsd::soap::{self, SCAN_NS_AIRSCAN, WSA_NS, WSE_NS};
use crate::wsd::xml::Element;

pub(crate) fn envelope_body(
    scan_ns: &str,
    action_uri: &str,
    relates: Option<&str>,
    body: Element,
) -> String {
    let mut resp = soap::Response::new().scan_ns(scan_ns).action(action_uri);
    if let Some(rel) = relates.filter(|s| !s.is_empty()) {
        resp = resp.relates_to(rel);
    }
    resp.body(body).to_xml()
}

fn param_el(name: &str, value: impl Into<String>, overridden: bool, used_default: bool) -> Element {
    let mut el = Element::text_el(name, value);
    if used_default {
        el = el.with_attr("wscn:UsedDefault", "true");
    } else if overridden {
        el = el.with_attr("wscn:Override", "true");
    }
    el
}

pub(crate) fn image_information(width_px: u32, height_px: u32, bytes_per_line: u32) -> Element {
    Element::new("wscn:ImageInformation").with(
        Element::new("wscn:MediaFrontImageInfo")
            .with(Element::text_el("wscn:PixelsPerLine", width_px.to_string()))
            .with(Element::text_el(
                "wscn:NumberOfLines",
                height_px.to_string(),
            ))
            .with(Element::text_el(
                "wscn:BytesPerLine",
                bytes_per_line.to_string(),
            )),
    )
}

pub(crate) fn validate_scan_ticket_response(
    valid_ticket: bool,
    width_px: u32,
    height_px: u32,
    bpl: u32,
    valid_scan_ticket: Option<Element>,
) -> Element {
    let mut info = Element::new("wscn:ValidationInfo")
        .with(Element::text_el(
            "wscn:ValidTicket",
            valid_ticket.to_string(),
        ))
        .with(image_information(width_px, height_px, bpl));
    if let Some(ticket) = valid_scan_ticket {
        info = info.with(ticket);
    }
    Element::new("wscn:ValidateScanTicketResponse").with(info)
}

pub(crate) fn valid_scan_ticket_element(
    request_body: &str,
    check: &TicketValidation,
    dpi: u32,
    format: &str,
    jfif_gray: crate::effects::JfifGray,
    jfif_bw: crate::effects::JfifBw,
) -> Element {
    let job_name =
        crate::state::extract_tag(request_body, "JobName").unwrap_or_else(|| "fakescan".into());
    let job_user = crate::state::extract_tag(request_body, "JobOriginatingUserName")
        .unwrap_or_else(|| "fakescan".into());
    let job_info = crate::state::extract_tag(request_body, "JobInformation");
    let input_source =
        crate::state::extract_tag(request_body, "InputSource").unwrap_or_else(|| "Platen".into());
    let color = jfif_wire_color(format, check.color.wsd_token(), jfif_gray, jfif_bw);
    let mut job_desc = Element::new("wscn:JobDescription")
        .with(Element::text_el("wscn:JobName", job_name))
        .with(Element::text_el("wscn:JobOriginatingUserName", job_user));
    if let Some(info) = job_info {
        job_desc = job_desc.with(Element::text_el("wscn:JobInformation", info));
    }
    let media_front = Element::new("wscn:MediaFront")
        .with(Element::text_el("wscn:ColorProcessing", color))
        .with(
            Element::new("wscn:Resolution")
                .with(Element::text_el("wscn:Width", dpi.to_string()))
                .with(Element::text_el("wscn:Height", dpi.to_string())),
        );
    Element::new("wscn:ValidScanTicket").with(job_desc).with(
        Element::new("wscn:DocumentParameters")
            .with(Element::text_el("wscn:Format", format))
            .with(Element::text_el("wscn:InputSource", input_source))
            .with(Element::new("wscn:MediaSides").with(media_front)),
    )
}

pub(crate) fn create_scan_job_response(
    job: &ScanJob,
    bytes_per_line: u32,
    final_color: &str,
) -> Element {
    let media_w = LETTER_W_THOU;
    let media_h = LETTER_H_THOU;
    let mut color_el = Element::text_el("wscn:ColorProcessing", final_color);
    if final_color != job.color.wsd_token() {
        color_el = color_el.with_attr("wscn:Override", "true");
    }
    let final_params = Element::new("wscn:DocumentFinalParameters")
        .with(param_el(
            "wscn:Format",
            &job.format,
            job.format_overridden,
            job.format_used_default,
        ))
        .with(
            Element::new("wscn:CompressionQualityFactor")
                .with_attr("wscn:UsedDefault", "true")
                .with_text("100"),
        )
        .with(Element::text_el(
            "wscn:ImagesToTransfer",
            job.pages_left.to_string(),
        ))
        .with(Element::text_el("wscn:InputSource", &job.input_source))
        .with(
            Element::new("wscn:ContentType")
                .with_attr("wscn:UsedDefault", "true")
                .with_text("Auto"),
        )
        .with(
            Element::new("wscn:InputSize").with(
                Element::new("wscn:InputMediaSize")
                    .with(Element::text_el("wscn:Width", media_w.to_string()))
                    .with(Element::text_el("wscn:Height", media_h.to_string())),
            ),
        )
        .with(
            Element::new("wscn:Exposure").with(
                Element::new("wscn:ExposureSettings")
                    .with(Element::text_el("wscn:Contrast", "0"))
                    .with(Element::text_el("wscn:Brightness", "0"))
                    .with(
                        Element::new("wscn:Sharpness")
                            .with_attr("wscn:UsedDefault", "true")
                            .with_text("0"),
                    ),
            ),
        )
        .with(
            Element::new("wscn:Scaling")
                .with(Element::text_el("wscn:ScalingWidth", "100"))
                .with(Element::text_el("wscn:ScalingHeight", "100")),
        )
        .with(Element::text_el("wscn:Rotation", "0"))
        .with(
            Element::new("wscn:MediaSides").with(
                Element::new("wscn:MediaFront")
                    .with(
                        Element::new("wscn:ScanRegion")
                            .with(Element::text_el("wscn:ScanRegionXOffset", "0"))
                            .with(Element::text_el("wscn:ScanRegionYOffset", "0"))
                            .with(Element::text_el(
                                "wscn:ScanRegionWidth",
                                media_w.to_string(),
                            ))
                            .with(Element::text_el(
                                "wscn:ScanRegionHeight",
                                media_h.to_string(),
                            )),
                    )
                    .with(color_el)
                    .with(
                        Element::new("wscn:Resolution")
                            .with(param_el(
                                "wscn:Width",
                                job.dpi.to_string(),
                                job.dpi_overridden,
                                job.dpi_used_default,
                            ))
                            .with(param_el(
                                "wscn:Height",
                                job.dpi.to_string(),
                                job.dpi_overridden,
                                job.dpi_used_default,
                            )),
                    ),
            ),
        );
    Element::new("wscn:CreateScanJobResponse")
        .with(Element::text_el("wscn:JobId", job.id.to_string()))
        .with(Element::text_el("wscn:JobToken", &job.token))
        .with(image_information(
            job.width_px,
            job.height_px,
            bytes_per_line,
        ))
        .with(final_params)
}

pub(crate) fn retrieve_image_response(content_id: &str) -> Element {
    Element::new("wscn:RetrieveImageResponse").with(
        Element::new("wscn:ScanData")
            .with(Element::new("xop:Include").with_attr("href", format!("cid:{content_id}"))),
    )
}

pub(crate) fn cancel_job_response() -> Element {
    Element::new("wscn:CancelJobResponse")
}

pub(crate) fn job_summary(job: &ScanJob) -> Element {
    Element::new("wscn:JobSummary")
        .with(Element::text_el("wscn:JobId", job.id.to_string()))
        .with(Element::text_el("wscn:JobName", "fakescan"))
        .with(Element::text_el("wscn:JobOriginatingUserName", "fakescan"))
        .with(Element::text_el("wscn:JobState", job.job_state()))
        .with(
            Element::new("wscn:JobStateReasons")
                .with(Element::text_el("wscn:JobStateReason", &job.state_reason)),
        )
        .with(Element::text_el(
            "wscn:ScansCompleted",
            job.page_index.to_string(),
        ))
}

pub(crate) fn get_active_jobs_response(jobs: &[ScanJob]) -> Element {
    let mut active = Element::new("wscn:ActiveJobs");
    for job in jobs {
        active = active.with(job_summary(job));
    }
    Element::new("wscn:GetActiveJobsResponse").with(active)
}

pub(crate) fn job_history_summary(id: i32, state: &str, reason: &str) -> Element {
    Element::new("wscn:JobSummary")
        .with(Element::text_el("wscn:JobId", id.to_string()))
        .with(Element::text_el("wscn:JobState", state))
        .with(
            Element::new("wscn:JobStateReasons")
                .with(Element::text_el("wscn:JobStateReason", reason)),
        )
        .with(Element::text_el("wscn:JobName", "fakescan"))
        .with(Element::text_el("wscn:JobOriginatingUserName", "fakescan"))
}

pub(crate) fn get_job_history_response(summaries: Vec<Element>) -> Element {
    let mut hist = Element::new("wscn:JobHistory");
    for s in summaries {
        hist = hist.with(s);
    }
    Element::new("wscn:GetJobHistoryResponse").with(hist)
}

pub(crate) fn subscribe_response(
    manager_address: &str,
    identifier: &str,
    expires: &str,
    destination_responses: Vec<Element>,
) -> Element {
    let mut resp = Element::new("wse:SubscribeResponse")
        .with_attr("xmlns:wse", WSE_NS)
        .with(
            Element::new("wse:SubscriptionManager")
                .with(Element::text_el("wsa:Address", manager_address))
                .with(
                    Element::new("wsa:ReferenceParameters")
                        .with(Element::text_el("wse:Identifier", identifier)),
                ),
        )
        .with(Element::text_el("wse:Expires", expires));
    if !destination_responses.is_empty() {
        let mut dests = Element::new("wscn:DestinationResponses");
        for block in destination_responses {
            dests = dests.with(block);
        }
        resp = resp.with(dests);
    }
    resp
}

pub(crate) fn destination_response(client_context: &str, token: &str) -> Element {
    Element::new("wscn:DestinationResponse")
        .with(Element::text_el("wscn:ClientContext", client_context))
        .with(Element::text_el("wscn:DestinationToken", token))
}

pub(crate) fn renew_response(expires: &str) -> Element {
    Element::new("wse:RenewResponse")
        .with_attr("xmlns:wse", WSE_NS)
        .with(Element::text_el("wse:Expires", expires))
}

pub(crate) fn unsubscribe_response() -> Element {
    Element::new("wse:UnsubscribeResponse").with_attr("xmlns:wse", WSE_NS)
}

pub(crate) fn get_status_response(expires: &str) -> Element {
    Element::new("wse:GetStatusResponse")
        .with_attr("xmlns:wse", WSE_NS)
        .with(Element::text_el("wse:Expires", expires))
}

const WSX_NS: &str = "http://schemas.xmlsoap.org/ws/2004/09/mex";
const WSDP_NS: &str = "http://schemas.xmlsoap.org/ws/2006/02/devprof";
const PNPX_NS: &str = "http://schemas.microsoft.com/windows/pnpx/2005/10";
const DF_NS: &str = "http://schemas.microsoft.com/windows/2008/09/devicefoundation";

fn lang_text_el(name: &str, lang: &str, text: impl Into<String>) -> Element {
    Element::text_el(name, text).with_attr("xml:lang", lang)
}

/// Full WS-Transfer `GetResponse` envelope around [`transfer_get_metadata`].
///
/// WSDAPI mints and expects `urn:uuid:` message ids (DPWS), not the WS-Discovery-era `uuid:`
/// form the scan replies use, so this one reply carries a `urn:uuid:` MessageID. `RelatesTo`
/// still echoes the request verbatim.
pub(crate) fn transfer_get_response(relates: Option<&str>, metadata: Element) -> String {
    let mut resp = soap::Response::new()
        .scan_ns(soap::SCAN_NS)
        .action("http://schemas.xmlsoap.org/ws/2004/09/transfer/GetResponse")
        .message_id(format!("urn:uuid:{}", uuid::Uuid::new_v4()));
    if let Some(rel) = relates.filter(|s| !s.is_empty()) {
        resp = resp.relates_to(rel);
    }
    resp.body(metadata).to_xml()
}

/// WS-Transfer GetResponse body: MEX metadata (ThisModel, ThisDevice, Relationship).
pub(crate) fn transfer_get_metadata(
    friendly_name: &str,
    device_uuid: &str,
    service_endpoint: &str,
) -> Element {
    let compat = format!("{SCAN_NS_AIRSCAN}/ScannerServiceType");
    let host_service_id = format!("uri:{device_uuid}");
    let scanner_service_id = format!("uri:{device_uuid}/ScannerService");

    let this_model = Element::new("wsdp:ThisModel")
        .with_attr("xmlns:wsdp", WSDP_NS)
        .with_attr("xmlns:pnpx", PNPX_NS)
        .with_attr("xmlns:df", DF_NS)
        .with(lang_text_el("wsdp:Manufacturer", "en-US", "fakescan"))
        .with(Element::text_el(
            "wsdp:ManufacturerUrl",
            "http://localhost/",
        ))
        .with(lang_text_el("wsdp:ModelName", "en-US", "Fake WSD Scanner"))
        .with(Element::text_el("wsdp:ModelNumber", "1"))
        .with(Element::text_el("wsdp:ModelUrl", "http://localhost/"))
        .with(Element::text_el(
            "wsdp:PresentationUrl",
            "http://localhost/",
        ))
        .with(Element::text_el("pnpx:DeviceCategory", "Scanners"))
        .with(Element::text_el("df:DeviceCategory", "Imaging.Scanner"));

    let this_device = Element::new("wsdp:ThisDevice")
        .with_attr("xmlns:wsdp", WSDP_NS)
        .with(lang_text_el("wsdp:FriendlyName", "en-US", friendly_name))
        .with(Element::text_el("wsdp:FirmwareVersion", "0.1.0"))
        .with(Element::text_el("wsdp:SerialNumber", "FS-0001"));

    let host_epr = Element::new("wsa:EndpointReference").with(Element::text_el(
        "wsa:Address",
        format!("urn:uuid:{device_uuid}"),
    ));
    let host_types = Element::new("wsdp:Types")
        .with_attr("xmlns:wscn", SCAN_NS_AIRSCAN)
        .with_text("wsdp:Device wscn:ScanDeviceType");

    let hosted_epr = Element::new("wsa:EndpointReference")
        .with(Element::text_el("wsa:Address", service_endpoint));
    let hosted_types = Element::new("wsdp:Types")
        .with_attr("xmlns:wscn", SCAN_NS_AIRSCAN)
        .with_text("wscn:ScannerServiceType");

    let relationship = Element::new("wsdp:Relationship")
        .with_attr("Type", "http://schemas.xmlsoap.org/ws/2006/02/devprof/host")
        .with_attr("xmlns:wsdp", WSDP_NS)
        .with_attr("xmlns:wsa", WSA_NS)
        .with(
            Element::new("wsdp:Host")
                .with(host_epr)
                .with(host_types)
                .with(Element::text_el("wsdp:ServiceId", host_service_id)),
        )
        .with(
            Element::new("wsdp:Hosted")
                .with(hosted_epr)
                .with(hosted_types)
                .with(Element::text_el("wsdp:ServiceId", scanner_service_id))
                .with(
                    Element::new("pnpx:HardwareId")
                        .with_attr("xmlns:pnpx", PNPX_NS)
                        .with_text("VEN_FAKE&DEV_WSDSCAN"),
                )
                .with(
                    Element::new("pnpx:CompatibleId")
                        .with_attr("xmlns:pnpx", PNPX_NS)
                        .with_text(compat),
                ),
        );

    Element::new("wsx:Metadata")
        .with_attr("xmlns:wsx", WSX_NS)
        .with(
            Element::new("wsx:MetadataSection")
                .with_attr(
                    "Dialect",
                    "http://schemas.xmlsoap.org/ws/2006/02/devprof/ThisModel",
                )
                .with(this_model),
        )
        .with(
            Element::new("wsx:MetadataSection")
                .with_attr(
                    "Dialect",
                    "http://schemas.xmlsoap.org/ws/2006/02/devprof/ThisDevice",
                )
                .with(this_device),
        )
        .with(
            Element::new("wsx:MetadataSection")
                .with_attr(
                    "Dialect",
                    "http://schemas.xmlsoap.org/ws/2006/02/devprof/Relationship",
                )
                .with(relationship),
        )
}

/// ColorProcessing token FinalParameters / ValidScanTicket carry for `format`.
///
/// JFIF/JPEG/EXIF coerce non-gray tickets to RGB24. A Grayscale8 ticket stays
/// Grayscale8 (default `jfif_gray=gray8`; live proof 2026-10-12 that Windows
/// rejects the RGB24 coercion). Only the legacy opt-in `jfif_gray=rgb24` still
/// coerces gray to RGB24. BlackAndWhite1 follows `jfif_bw` the same way (default
/// `gray8` -> Grayscale8; legacy `rgb24` -> RGB24, the Brother-style fallback).
pub(crate) fn jfif_wire_color<'a>(
    format: &str,
    ticket_color: &'a str,
    jfif_gray: crate::effects::JfifGray,
    jfif_bw: crate::effects::JfifBw,
) -> &'a str {
    if format.eq_ignore_ascii_case("jfif")
        || format.eq_ignore_ascii_case("jpeg")
        || format.eq_ignore_ascii_case("exif")
    {
        if ticket_color == "Grayscale8" && jfif_gray.keeps_gray_token() {
            ticket_color
        } else if ticket_color == "BlackAndWhite1" && jfif_bw == crate::effects::JfifBw::Gray8 {
            // JPEG cannot carry 1-bit: answer the conforming Grayscale8 instead.
            "Grayscale8"
        } else {
            "RGB24"
        }
    } else {
        ticket_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "00000000-0000-4000-8000-0000000000e4";

    fn get_xml() -> String {
        transfer_get_response(
            Some("urn:uuid:11111111-2222-3333-4444-555555555555"),
            transfer_get_metadata("fakescan", UUID, "http://192.0.2.10:3920/ScannerService"),
        )
    }

    #[test]
    fn get_response_uses_urn_uuid_message_id_and_echoes_relates_to() {
        let xml = get_xml();
        assert!(xml.contains("<wsa:MessageID>urn:uuid:"), "{xml}");
        assert!(
            xml.contains(
                "<wsa:RelatesTo>urn:uuid:11111111-2222-3333-4444-555555555555</wsa:RelatesTo>"
            ),
            "{xml}"
        );
        assert!(
            xml.contains("<wsa:Action>http://schemas.xmlsoap.org/ws/2004/09/transfer/GetResponse</wsa:Action>"),
            "{xml}"
        );
        assert!(crate::wsd::soap::reply_problems(&xml).is_empty(), "{xml}");
    }

    /// Pins the PnP-X placement Microsoft documents (Rally Vertical Pairing PnP-X support):
    /// DeviceCategory in ThisModel (Vista pnpx + Win7 df), HardwareId/CompatibleId inside
    /// Hosted, Host carrying EPR + Types + ServiceId.
    #[test]
    fn get_metadata_places_pnpx_elements_where_wsdapi_reads_them() {
        let xml = get_xml();
        let model =
            &xml[xml.find("<wsdp:ThisModel").unwrap()..xml.find("</wsdp:ThisModel>").unwrap()];
        assert!(
            model.contains("<pnpx:DeviceCategory>Scanners</pnpx:DeviceCategory>"),
            "{model}"
        );
        assert!(
            model.contains("<df:DeviceCategory>Imaging.Scanner</df:DeviceCategory>"),
            "{model}"
        );
        let host = &xml[xml.find("<wsdp:Host>").unwrap()..xml.find("</wsdp:Host>").unwrap()];
        assert!(
            host.contains(&format!("<wsa:Address>urn:uuid:{UUID}</wsa:Address>")),
            "{host}"
        );
        assert!(host.contains("wsdp:Device wscn:ScanDeviceType"), "{host}");
        assert!(host.contains("<wsdp:ServiceId>"), "{host}");
        let hosted = &xml[xml.find("<wsdp:Hosted>").unwrap()..xml.find("</wsdp:Hosted>").unwrap()];
        assert!(
            hosted.contains("<wsa:Address>http://192.0.2.10:3920/ScannerService</wsa:Address>"),
            "{hosted}"
        );
        assert!(
            hosted.contains(">wscn:ScannerServiceType</wsdp:Types>"),
            "{hosted}"
        );
        assert!(
            hosted.contains(">VEN_FAKE&amp;DEV_WSDSCAN</pnpx:HardwareId>"),
            "{hosted}"
        );
        assert!(
            hosted.contains(">http://schemas.microsoft.com/windows/2006/08/wdp/scan/ScannerServiceType</pnpx:CompatibleId>"),
            "{hosted}"
        );
        // MetadataVersion belongs to discovery messages only.
        assert!(!xml.contains("MetadataVersion"), "{xml}");
    }
}
