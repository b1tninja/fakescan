//! WSD Scan Service HTTP endpoint (SOAP + MTOM RetrieveImage).

mod elements;
mod soap;

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::World;
use crate::wsd::soap::SCAN_NS;

pub async fn scanner_service(State(world): State<Arc<RwLock<World>>>, body: Bytes) -> Response {
    let text = String::from_utf8_lossy(&body);
    let action = soap::action(&text).unwrap_or_else(|| "(unknown)".into());
    let short = soap::short_action(&action);
    let relates = soap::message_id(&text);
    info!(%short, bytes = body.len(), "WSD Scan Service request");

    let result = dispatch(&world, &action, &text, relates.as_deref()).await;
    match result {
        Ok(SoapOut::Xml(xml)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
            xml,
        )
            .into_response(),
        Ok(SoapOut::Mtom {
            soap_xml,
            image,
            content_type,
        }) => mtom_response(&soap_xml, &image, &content_type),
        Err(msg) => {
            warn!(%short, %msg, "WSD fault");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
                soap::fault(&msg),
            )
                .into_response()
        }
    }
}

enum SoapOut {
    Xml(String),
    Mtom {
        soap_xml: String,
        image: Vec<u8>,
        content_type: String,
    },
}

async fn dispatch(
    world: &Arc<RwLock<World>>,
    action: &str,
    body: &str,
    relates: Option<&str>,
) -> Result<SoapOut, String> {
    let short = soap::short_action(action);
    match short {
        "GetScannerElements" => {
            let w = world.read().await;
            let inner = elements::get_scanner_elements(w.profile(), w.feeder_ready(), body);
            Ok(SoapOut::Xml(soap::envelope(
                &format!("{SCAN_NS}/GetScannerElements"),
                relates,
                &inner,
            )))
        }
        "ValidateScanTicket" => {
            let inner = r#"    <wscn:ValidateScanTicketResponse>
      <wscn:ValidationInfo>
        <wscn:ValidTicket>true</wscn:ValidTicket>
      </wscn:ValidationInfo>
    </wscn:ValidateScanTicketResponse>"#;
            Ok(SoapOut::Xml(soap::envelope(
                &format!("{SCAN_NS}/ValidateScanTicket"),
                relates,
                inner,
            )))
        }
        "CreateScanJob" => {
            let mut w = world.write().await;
            let job = w.create_job(body).map_err(|e| e.to_string())?;
            let bytes_per_line = job.width_px * 3;
            let inner = format!(
                r#"    <wscn:CreateScanJobResponse>
      <wscn:JobId>{id}</wscn:JobId>
      <wscn:JobToken>{token}</wscn:JobToken>
      <wscn:ImageInformation>
        <wscn:MediaFrontImageInfo>
          <wscn:PixelsPerLine>{w}</wscn:PixelsPerLine>
          <wscn:NumberOfLines>{h}</wscn:NumberOfLines>
          <wscn:BytesPerLine>{bpl}</wscn:BytesPerLine>
        </wscn:MediaFrontImageInfo>
      </wscn:ImageInformation>
      <wscn:DocumentFinalParameters>
        <wscn:Format>{fmt}</wscn:Format>
        <wscn:CompressionQualityFactor>80</wscn:CompressionQualityFactor>
        <wscn:ImagesToTransfer>{pages}</wscn:ImagesToTransfer>
        <wscn:InputSource>{src}</wscn:InputSource>
        <wscn:ContentType>Auto</wscn:ContentType>
        <wscn:InputSize>
          <wscn:InputMediaSize>
            <wscn:Width>8500</wscn:Width>
            <wscn:Height>11000</wscn:Height>
          </wscn:InputMediaSize>
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
      </wscn:DocumentFinalParameters>
    </wscn:CreateScanJobResponse>"#,
                id = job.id,
                token = soap::esc(&job.token),
                w = job.width_px,
                h = job.height_px,
                bpl = bytes_per_line,
                fmt = job.format,
                pages = job.pages_left,
                src = job.input_source,
                dpi = job.dpi,
            );
            Ok(SoapOut::Xml(soap::envelope(
                &format!("{SCAN_NS}/CreateScanJob"),
                relates,
                &inner,
            )))
        }
        "RetrieveImage" => {
            let job_id: i32 = soap::extract_tagged(body, "JobId")
                .ok_or_else(|| "JobId missing".to_string())?
                .parse()
                .map_err(|_| "JobId invalid".to_string())?;
            let token = soap::extract_tagged(body, "JobToken")
                .ok_or_else(|| "JobToken missing".to_string())?;
            let mut w = world.write().await;
            let (job, image) = w
                .retrieve_image(job_id, &token)
                .map_err(|e| e.to_string())?;
            let cid = format!("{}@fakescan", Uuid::new_v4());
            let inner = format!(
                r#"    <wscn:RetrieveImageResponse>
      <wscn:ScanData>
        <xop:Include xmlns:xop="http://www.w3.org/2003/12/xop/include" href="cid:{cid}" />
      </wscn:ScanData>
    </wscn:RetrieveImageResponse>"#
            );
            let soap_xml = soap::envelope(&format!("{SCAN_NS}/RetrieveImage"), relates, &inner);
            // Always JPEG for now (profile may list tiff/bmp; encode path is jpeg).
            let _ = job;
            Ok(SoapOut::Mtom {
                soap_xml,
                image,
                content_type: "image/jpeg".into(),
            })
        }
        "CancelJob" => {
            let job_id: i32 = soap::extract_tagged(body, "JobId")
                .ok_or_else(|| "JobId missing".to_string())?
                .parse()
                .map_err(|_| "JobId invalid".to_string())?;
            world
                .write()
                .await
                .cancel_job(job_id)
                .map_err(|e| e.to_string())?;
            let inner = r#"    <wscn:CancelJobResponse/>"#;
            Ok(SoapOut::Xml(soap::envelope(
                &format!("{SCAN_NS}/CancelJob"),
                relates,
                inner,
            )))
        }
        "GetActiveJobs" => {
            let inner = r#"    <wscn:GetActiveJobsResponse>
      <wscn:ActiveJobs/>
    </wscn:GetActiveJobsResponse>"#;
            Ok(SoapOut::Xml(soap::envelope(
                &format!("{SCAN_NS}/GetActiveJobs"),
                relates,
                inner,
            )))
        }
        "Get" => {
            // Minimal WS-Transfer Get so discovery clients get a ThisDevice stub.
            let inner = r#"    <wsx:Metadata xmlns:wsx="http://schemas.xmlsoap.org/ws/2004/09/mex"
                   xmlns:wsdp="http://schemas.xmlsoap.org/ws/2006/02/devprof"
                   xmlns:this="http://schemas.xmlsoap.org/ws/2005/08/wsdl">
      <wsx:MetadataSection Dialect="http://schemas.xmlsoap.org/ws/2006/02/devprof/ThisDevice">
        <wsdp:ThisDevice>
          <wsdp:FriendlyName xml:lang="en-US">fakescan</wsdp:FriendlyName>
          <wsdp:FirmwareVersion>0.1.0</wsdp:FirmwareVersion>
          <wsdp:SerialNumber>FS-0001</wsdp:SerialNumber>
        </wsdp:ThisDevice>
      </wsx:MetadataSection>
      <wsx:MetadataSection Dialect="http://schemas.xmlsoap.org/ws/2006/02/devprof/ThisModel">
        <wsdp:ThisModel>
          <wsdp:Manufacturer xml:lang="en-US">fakescan</wsdp:Manufacturer>
          <wsdp:ManufacturerUrl>http://localhost/</wsdp:ManufacturerUrl>
          <wsdp:ModelName xml:lang="en-US">Fake WSD Scanner</wsdp:ModelName>
          <wsdp:ModelNumber>1</wsdp:ModelNumber>
        </wsdp:ThisModel>
      </wsx:MetadataSection>
    </wsx:Metadata>"#;
            Ok(SoapOut::Xml(soap::envelope(
                "http://schemas.xmlsoap.org/ws/2004/09/transfer/GetResponse",
                relates,
                inner,
            )))
        }
        other => Err(format!("action not implemented: {other}")),
    }
}

fn mtom_response(soap_xml: &str, image: &[u8], image_ct: &str) -> Response {
    let boundary = format!("uuid:{}", Uuid::new_v4());
    let soap_cid = format!("soap@{}", Uuid::new_v4());
    // Extract cid from soap
    let image_cid = soap_xml
        .split("cid:")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("image@fakescan")
        .to_string();

    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Type: application/xop+xml; charset=utf-8; type=\"application/soap+xml\"\r\n\
Content-Transfer-Encoding: binary\r\n\
Content-ID: <{soap_cid}>\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(soap_xml.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Type: {image_ct}\r\n\
Content-Transfer-Encoding: binary\r\n\
Content-ID: <{image_cid}>\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(image);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!(
            "multipart/related; type=\"application/xop+xml\"; boundary=\"{boundary}\"; start=\"<{soap_cid}>\"; start-info=\"application/soap+xml\""
        )
        .parse()
        .expect("content-type"),
    );

    (StatusCode::OK, headers, body).into_response()
}
