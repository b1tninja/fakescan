//! Compact XML tree for WSD SOAP: one parse path and one serialize path.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

const SCAN_NS_SUFFIX: &str = "/wdp/scan";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub local: String,
    pub ns: Option<String>,
    pub prefix: Option<String>,
    pub attrs: Vec<Attr>,
    pub children: Vec<Element>,
    pub text: String,
    pub xmlns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub local: String,
    pub ns: Option<String>,
    pub prefix: Option<String>,
    pub value: String,
}

/// Skip MIME/MTOM preamble; parse one root element (`trim_text(true)`).
pub fn parse(input: &str) -> Option<Element> {
    let xml = skip_to_xml(input);
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut prefixes: Vec<HashMap<String, String>> = vec![HashMap::new()];
    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(el) => {
                let mut map = prefixes.last().cloned().unwrap_or_default();
                take_xmlns_start(&el, &mut map);
                let node = start_element(&el, &map);
                prefixes.push(map);
                stack.push(node);
            }
            Event::Empty(el) => {
                let mut map = prefixes.last().cloned().unwrap_or_default();
                take_xmlns_start(&el, &mut map);
                let node = start_element(&el, &map);
                if stack.is_empty() {
                    return Some(node);
                }
                push_child(&mut stack, node);
            }
            Event::Text(text) => {
                if let Some(parent) = stack.last_mut() {
                    if let Ok(value) = text.unescape() {
                        parent.text.push_str(&value);
                    }
                }
            }
            Event::CData(text) => {
                if let Some(parent) = stack.last_mut() {
                    parent
                        .text
                        .push_str(&String::from_utf8_lossy(text.as_ref()));
                }
            }
            Event::End(_) => {
                prefixes.pop();
                let node = stack.pop()?;
                if stack.is_empty() {
                    return Some(node);
                }
                push_child(&mut stack, node);
            }
            Event::Eof => {
                while stack.len() > 1 {
                    let node = stack.pop().expect("stack");
                    push_child(&mut stack, node);
                }
                break;
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
            _ => {}
        }
        buf.clear();
    }
    stack.pop()
}

fn skip_to_xml(input: &str) -> &str {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == b'?' {
            return &input[i..];
        }
        if i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next.is_ascii_alphabetic() || next == b'_' || next == b':' {
                return &input[i..];
            }
        }
        i += 1;
    }
    input
}

impl Element {
    pub fn new(name: &str) -> Self {
        let (prefix, local) = split_name(name);
        Self {
            local: local.to_string(),
            ns: None,
            prefix: prefix.map(str::to_string),
            attrs: Vec::new(),
            children: Vec::new(),
            text: String::new(),
            xmlns: Vec::new(),
        }
    }

    pub fn with_attr(mut self, key: &str, value: impl Into<String>) -> Self {
        let (prefix, local) = split_name(key);
        self.attrs.push(Attr {
            local: local.to_string(),
            ns: None,
            prefix: prefix.map(str::to_string),
            value: value.into(),
        });
        self
    }

    pub fn with_text(mut self, s: impl Into<String>) -> Self {
        self.text = s.into();
        self
    }

    pub fn with(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    pub fn text_el(name: &str, s: impl Into<String>) -> Self {
        Self::new(name).with_text(s)
    }

    pub fn name(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.local),
            None => self.local.clone(),
        }
    }

    pub fn attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.local == local)
            .map(|a| a.value.as_str())
    }

    pub fn child(&self, local: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.local == local)
    }

    pub fn descendant(&self, local: &str) -> Option<&Element> {
        if self.local == local {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.descendant(local) {
                return Some(found);
            }
        }
        None
    }

    pub fn children_named(&self, local: &str) -> Vec<&Element> {
        self.children.iter().filter(|c| c.local == local).collect()
    }

    pub fn find_text(&self, local: &str) -> Option<&str> {
        if self.local == local {
            let t = self.text.trim();
            if !t.is_empty() {
                return Some(t);
            }
        }
        for child in &self.children {
            if let Some(t) = child.find_text(local) {
                return Some(t);
            }
        }
        None
    }

    pub fn all_text(&self, local: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_all_text(local, &mut out);
        out
    }

    fn collect_all_text(&self, local: &str, out: &mut Vec<String>) {
        if self.local == local {
            let t = self.text.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
        }
        for child in &self.children {
            child.collect_all_text(local, out);
        }
    }

    /// Pre-order walk; `f` sees every element.
    pub fn walk(&self, f: &mut dyn FnMut(&Element, &[&str])) {
        self.walk_stack(f, &[]);
    }

    fn walk_stack(&self, f: &mut dyn FnMut(&Element, &[&str]), stack: &[&str]) {
        f(self, stack);
        let mut next = stack.to_vec();
        next.push(self.local.as_str());
        for child in &self.children {
            child.walk_stack(f, &next);
        }
    }

    pub fn in_scan_ns(&self) -> bool {
        self.ns
            .as_deref()
            .is_some_and(|uri| uri.contains(SCAN_NS_SUFFIX))
    }

    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        self.write_xml(&mut out);
        out
    }

    fn write_xml(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name());
        for attr in &self.attrs {
            out.push(' ');
            write_attr_name(out, attr);
            out.push_str("=\"");
            out.push_str(&esc(&attr.value));
            out.push('"');
        }
        if self.children.is_empty() && self.text.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        if !self.text.is_empty() {
            out.push_str(&esc(&self.text));
        }
        for child in &self.children {
            child.write_xml(out);
        }
        out.push_str("</");
        out.push_str(&self.name());
        out.push('>');
    }
}

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn start_element(
    el: &quick_xml::events::BytesStart<'_>,
    prefixes: &HashMap<String, String>,
) -> Element {
    let raw = String::from_utf8_lossy(el.name().as_ref()).into_owned();
    let (prefix, local) = split_name(&raw);
    let ns = prefix
        .and_then(|name| prefixes.get(name))
        .cloned()
        .or_else(|| prefixes.get("").cloned());
    let mut xmlns = Vec::new();
    let mut attrs = Vec::new();
    for attr in el.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let Ok(value) = attr.unescape_value() else {
            continue;
        };
        let value = value.into_owned();
        if key == "xmlns" {
            xmlns.push(value.clone());
            attrs.push(Attr {
                local: "xmlns".into(),
                ns: None,
                prefix: None,
                value,
            });
            continue;
        }
        if let Some(p) = key.strip_prefix("xmlns:") {
            xmlns.push(value.clone());
            attrs.push(Attr {
                local: "xmlns".into(),
                ns: None,
                prefix: Some(p.to_string()),
                value,
            });
            continue;
        }
        let (attr_prefix, attr_local) = split_name(&key);
        let attr_ns = attr_prefix.and_then(|item| prefixes.get(item)).cloned();
        attrs.push(Attr {
            local: attr_local.to_string(),
            ns: attr_ns,
            prefix: attr_prefix.map(str::to_string),
            value,
        });
    }
    Element {
        local: local.to_string(),
        ns,
        prefix: prefix.map(str::to_string),
        attrs,
        children: Vec::new(),
        text: String::new(),
        xmlns,
    }
}

fn take_xmlns_start(el: &quick_xml::events::BytesStart<'_>, map: &mut HashMap<String, String>) {
    for attr in el.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let Ok(value) = attr.unescape_value() else {
            continue;
        };
        if key == "xmlns" {
            map.insert(String::new(), value.into_owned());
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            map.insert(prefix.to_string(), value.into_owned());
        }
    }
}

fn push_child(stack: &mut [Element], node: Element) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    }
}

fn split_name(name: &str) -> (Option<&str>, &str) {
    match name.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, name),
    }
}

fn write_attr_name(out: &mut String, attr: &Attr) {
    if attr.local == "xmlns" {
        out.push_str("xmlns");
        if let Some(p) = &attr.prefix {
            out.push(':');
            out.push_str(p);
        }
        return;
    }
    if let Some(p) = &attr.prefix {
        out.push_str(p);
        out.push(':');
    }
    out.push_str(&attr.local);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_mtom_preamble() {
        let raw =
            "Content-Type: application/xop+xml\r\n\r\n<?xml version=\"1.0\"?><a><b>x</b></a>tail";
        let root = parse(raw).expect("root");
        assert_eq!(root.local, "a");
        assert_eq!(root.child("b").and_then(|n| n.find_text("b")), Some("x"));
    }

    #[test]
    fn truncated_root_returns_partial_tree() {
        let raw =
            "<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\"><soap:Body><x>";
        let root = parse(raw).expect("partial");
        assert_eq!(root.local, "Envelope");
    }

    #[test]
    fn xmlns_resolves_prefixed_element() {
        let raw = r#"<wscn:Format xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">jfif</wscn:Format>"#;
        let root = parse(raw).expect("root");
        assert!(root.in_scan_ns());
        assert_eq!(root.find_text("Format"), Some("jfif"));
    }

    #[test]
    fn prefixed_attrs_round_trip() {
        let el = Element::new("wscn:ElementData")
            .with_attr("wscn:Name", "wscn:JobId")
            .with_attr("wscn:Valid", "true");
        let xml = el.to_xml();
        assert!(xml.contains("wscn:Name=\"wscn:JobId\""));
        let back = parse(&xml).expect("parse");
        assert_eq!(back.attr("Name"), Some("wscn:JobId"));
    }

    #[test]
    fn name_attr_is_not_job_id_element() {
        let raw = r#"<wscn:ElementData xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan" Name="wscn:JobId" Valid="true"/>"#;
        let root = parse(raw).expect("root");
        assert!(root.find_text("JobId").is_none());
        assert_eq!(root.attr("Name"), Some("wscn:JobId"));
    }

    #[test]
    fn all_text_collects_client_context() {
        let raw = r#"<root xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <wscn:ScanDestinations><wscn:ClientContext>a</wscn:ClientContext></wscn:ScanDestinations>
          <wscn:ScanDestinations><wscn:ClientContext>b</wscn:ClientContext></wscn:ScanDestinations>
        </root>"#;
        let root = parse(raw).expect("root");
        assert_eq!(root.all_text("ClientContext"), vec!["a", "b"]);
    }

    #[test]
    fn esc_handles_xml_chars() {
        assert_eq!(esc("a&b<c>\"d"), "a&amp;b&lt;c&gt;&quot;d");
    }
}
