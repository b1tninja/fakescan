//! SOAP 1.2 envelope. Header fields are parsed; the body markup is kept as it arrived.

use super::xml::{self, Element};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Envelope {
    pub action: Option<String>,
    pub message_id: Option<String>,
    pub relates_to: Option<String>,
    pub to: Option<String>,
    /// `wse:Identifier` outside the body (header `ReferenceParameters`).
    pub identifier: Option<String>,
    /// Local name of the first `soap:Body` child (`CreateScanJobRequest`).
    pub body_name: Option<String>,
    /// Document element when the input is not wrapped in `soap:Envelope`.
    pub root: Option<String>,
    /// Markup inside `soap:Body`, unchanged.
    pub body: String,
    /// `xmlns` values declared on the envelope and its header.
    pub xmlns: Vec<String>,
    /// Direct children of `soap:Header`.
    pub headers: Vec<HeaderBlock>,
    /// Parsed tree when the input was XML.
    pub tree: Option<Element>,
}

/// One header block. `must_understand` is the SOAP 1.2 attribute (`true` or `1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderBlock {
    pub name: String,
    pub must_understand: bool,
}

/// Who the envelope is speaking to. A reply uses the anonymous role; an event uses `NotifyTo`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReplyRole {
    Reply,
    Event,
}

/// What a header value does, and whether a bad value stops the exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub field: &'static str,
    /// `true` when SOAP says the receiver must fault.
    pub reject: bool,
    pub note: &'static str,
}

/// Anonymous role on a SOAP response (`wsa:To`).
pub const ANONYMOUS_ROLE: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous";

const SOAP_NS: &str = "http://www.w3.org/2003/05/soap-envelope";
const WSA_NS: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing";

/// Header blocks this service understands. `mustUnderstand="true"` on any other faults.
const UNDERSTOOD: &[&str] = &[
    "Action",
    "MessageID",
    "MessageId",
    "RelatesTo",
    "To",
    "ReplyTo",
    "From",
    "FaultTo",
];

impl Envelope {
    /// Parse `input`, or the first SOAP envelope inside it (an MTOM part).
    /// A truncated or non-XML tail is ignored once the header fields are read.
    pub fn parse(input: &str) -> Self {
        let slice = envelope_slice(input);
        let Some(root) = xml::parse(slice) else {
            return Self::default();
        };
        let mut out = Envelope {
            tree: Some(root.clone()),
            root: Some(root.local.clone()),
            ..Self::default()
        };
        let env = find_envelope(&root).unwrap_or(&root);
        if env.local == "Envelope" {
            out.root = Some("Envelope".into());
        }
        collect_xmlns(env, &mut out.xmlns);
        if let Some(header) = env.child("Header") {
            for child in &header.children {
                out.headers.push(HeaderBlock {
                    name: child.local.clone(),
                    must_understand: must_understand_el(child),
                });
            }
            out.action = header_text(header, "Action");
            out.message_id =
                header_text(header, "MessageID").or_else(|| header_text(header, "MessageId"));
            out.relates_to = header_text(header, "RelatesTo");
            out.to = header_text(header, "To");
            out.identifier = header_identifier(header);
        }
        if let Some(body) = env.child("Body") {
            out.body = body.children.iter().map(|c| c.to_xml()).collect::<String>();
            out.body_name = body.children.first().map(|c| c.local.clone());
        } else if env.local != "Envelope" {
            out.body = env.to_xml();
            out.body_name = Some(env.local.clone());
            if env.local == "Action" {
                out.action = text_or_child(env, "Action");
            }
            out.message_id =
                text_or_child(env, "MessageID").or_else(|| text_or_child(env, "MessageId"));
            out.identifier = header_identifier(env);
        }
        out.action = out.action.filter(|s| !s.is_empty());
        out.message_id = out.message_id.filter(|s| !s.is_empty());
        out.relates_to = out.relates_to.filter(|s| !s.is_empty());
        out.to = out.to.filter(|s| !s.is_empty());
        out.identifier = out.identifier.filter(|s| !s.is_empty());
        out
    }

    /// Header `Action`, or the body element's operation when the header omitted it.
    pub fn action_uri(&self, ops: &[&str], event_ns: &str, default_ns: &str) -> Option<String> {
        if let Some(action) = &self.action {
            if absolute_iri(action) {
                return Some(action.clone());
            }
        }
        let name = self.body_name.as_deref().or_else(|| {
            self.root
                .as_deref()
                .filter(|n| !matches!(*n, "Envelope" | "Header" | "Body"))
        })?;
        let op = name.strip_suffix("Request").unwrap_or(name);
        if ops.contains(&op) {
            let ns = self
                .xmlns
                .iter()
                .find(|ns| ns.contains("/2006/08/wdp/"))
                .or_else(|| self.xmlns.iter().find(|ns| ns.contains("/2006/01/wdp/")))
                .map(String::as_str)
                .unwrap_or(default_ns);
            return Some(format!("{ns}/{op}"));
        }
        if op == "Get" {
            return Some("http://schemas.xmlsoap.org/ws/2004/09/transfer/Get".into());
        }
        if matches!(op, "Subscribe" | "Unsubscribe" | "Renew" | "GetStatus") {
            return Some(format!("{event_ns}/{op}"));
        }
        None
    }

    /// Inbound header notes. `reject` is only an unknown `mustUnderstand` block.
    pub fn request_findings(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        match &self.action {
            Some(action) if !absolute_iri(action) => out.push(Finding {
                field: "Action",
                reject: false,
                note: "not an absolute IRI; the body element names the operation",
            }),
            None => out.push(Finding {
                field: "Action",
                reject: false,
                note: "absent; the body element names the operation",
            }),
            _ => {}
        }
        match &self.message_id {
            Some(id) if !absolute_iri(id) => out.push(Finding {
                field: "MessageID",
                reject: false,
                note: "not an absolute IRI; RelatesTo is omitted",
            }),
            None => out.push(Finding {
                field: "MessageID",
                reject: false,
                note:
                    "absent; RelatesTo is omitted. The HTTP response is still this request's reply",
            }),
            _ => {}
        }
        if self.to.as_deref().is_some_and(|to| !absolute_iri(to)) {
            out.push(Finding {
                field: "To",
                reject: false,
                note: "not an absolute IRI; HTTP already delivered the request",
            });
        }
        if self.relates_to.is_some() {
            out.push(Finding {
                field: "RelatesTo",
                reject: false,
                note: "a request RelatesTo is not the id the reply uses",
            });
        }
        for header in &self.headers {
            if header.must_understand && !UNDERSTOOD.contains(&header.name.as_str()) {
                out.push(Finding {
                    field: "mustUnderstand",
                    reject: true,
                    note: "unknown header block with mustUnderstand",
                });
            }
        }
        out
    }

    /// Problems in an envelope this service wrote. Empty means the header matches the schema.
    pub fn reply_problems(&self, role: ReplyRole) -> Vec<String> {
        let mut problems = Vec::new();
        if self.root.as_deref() != Some("Envelope") {
            problems.push("document element is not soap:Envelope".into());
        }
        if !self.xmlns.iter().any(|ns| ns == SOAP_NS) {
            problems.push("missing xmlns for the SOAP 1.2 envelope".into());
        }
        if !self.xmlns.iter().any(|ns| ns == WSA_NS) {
            problems.push("missing xmlns for WS-Addressing 2004/08".into());
        }
        if self
            .action
            .as_deref()
            .is_none_or(|action| !absolute_iri(action))
        {
            problems.push("Action must be an absolute IRI".into());
        }
        if self
            .message_id
            .as_deref()
            .is_none_or(|id| !reply_message_id(id))
        {
            problems.push(
                "MessageID must be a fresh uuid: or urn:uuid: IRI, not a reused fixed value".into(),
            );
        }
        match role {
            ReplyRole::Reply => {
                if self.to.as_deref() != Some(ANONYMOUS_ROLE) {
                    problems.push("reply To must be the anonymous role".into());
                }
            }
            ReplyRole::Event => {
                if self.to.as_deref().is_none_or(|to| !absolute_iri(to)) {
                    problems.push("event To must be an absolute IRI (NotifyTo)".into());
                }
            }
        }
        if self
            .relates_to
            .as_deref()
            .is_some_and(|id| !absolute_iri(id))
        {
            problems.push("RelatesTo must be an absolute IRI".into());
        }
        problems
    }

    /// Body element tree (first child of `soap:Body`), when present.
    pub fn body_tree(&self) -> Option<&Element> {
        let Some(tree) = &self.tree else {
            return None;
        };
        let env = find_envelope(tree).unwrap_or(tree);
        env.child("Body")?.children.first()
    }
}

fn find_envelope(el: &Element) -> Option<&Element> {
    if el.local == "Envelope" {
        return Some(el);
    }
    for child in &el.children {
        if let Some(found) = find_envelope(child) {
            return Some(found);
        }
    }
    None
}

fn collect_xmlns(el: &Element, out: &mut Vec<String>) {
    for uri in &el.xmlns {
        if !out.iter().any(|v| v == uri) {
            out.push(uri.clone());
        }
    }
    for child in &el.children {
        collect_xmlns(child, out);
    }
}

fn header_text(header: &Element, local: &str) -> Option<String> {
    text_or_child(header, local)
}

fn text_or_child(el: &Element, local: &str) -> Option<String> {
    if el.local == local {
        let t = el.text.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    el.find_text(local)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn header_identifier(header: &Element) -> Option<String> {
    fn walk(el: &Element, in_ref: bool) -> Option<String> {
        let in_ref = in_ref || el.local == "ReferenceParameters";
        if in_ref && el.local == "Identifier" {
            let t = el.text.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        for child in &el.children {
            if let Some(v) = walk(child, in_ref) {
                return Some(v);
            }
        }
        None
    }
    walk(header, false)
}

fn must_understand_el(el: &Element) -> bool {
    el.attr("mustUnderstand")
        .is_some_and(|v| v == "true" || v == "1")
}

/// `xs:anyURI` that is an absolute IRI: a scheme, no whitespace.
pub fn absolute_iri(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Message id this service emits: `uuid:{uuid}` or `urn:uuid:{uuid}`.
pub fn reply_message_id(value: &str) -> bool {
    let bare = value
        .strip_prefix("urn:uuid:")
        .or_else(|| value.strip_prefix("uuid:"));
    bare.is_some_and(is_uuid)
}

fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(parts)
            .all(|(n, part)| part.len() == *n && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Byte offset of the first `<*Envelope` start tag, if one exists.
fn envelope_slice(input: &str) -> &str {
    let bytes = input.as_bytes();
    let mut idx = 0;
    while let Some(rel) = input[idx..].find("Envelope") {
        let at = idx + rel;
        let before = bytes.get(at.wrapping_sub(1)).copied();
        if matches!(before, Some(b'<') | Some(b':')) {
            if let Some(start) = input[..at].rfind('<') {
                return &input[start..];
            }
        }
        idx = at + "Envelope".len();
    }
    input
}
