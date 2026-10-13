//! Check a SOAP body against the Scan Service XSD in `docs/reference/schema`.
//!
//! The schema's target namespace is `2006/08`. Messages in `2006/01` use the same
//! local names, so either scan namespace is accepted. SOAP faults and WS-Eventing
//! wrappers are not scan elements; a scan element nested in one (for example
//! `DestinationResponses`) is still checked.
//!
//! Lab / Learn allowances (stock XSD does not model these; keep them here so flow
//! tests need no ad-hoc `problems.retain` filters):
//! - **JobStatusType:** optional same-NS `ImagesCompleted` / `ImagesToTransfer`
//!   after `ScansCompleted` (XSD `xs:any` is `##other` only).
//! - **JobElementDataType:** `Name=…JobToken` may be `Valid=false` empty, or carry
//!   a same-NS `JobToken` child (not in the XSD choice).
//! - **ValidationInfo** with `ValidTicket=false`: `PixelsPerLine` / `NumberOfLines`
//!   / `BytesPerLine` may be `0` (Learn; `IntOneExtType` is otherwise min 1).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::xml::{self, Element};

const SCAN_NS_SUFFIX: &str = "/wdp/scan";

/// Same-NS JobStatus trailers lab devices emit; XSD only allows `xs:any ##other`.
const JOB_STATUS_LAB_EXTENSIONS: &[&str] = &["ImagesCompleted", "ImagesToTransfer"];

pub fn body_problems(soap_xml: &str) -> Vec<String> {
    let schema = schema();
    let root = match xml::parse(xml_document(soap_xml)).map(|el| element_to_node(&el)) {
        Some(root) => root,
        None => return vec!["body is not XML".into()],
    };
    let mut problems = Vec::new();
    check_tree(&root, schema, &mut problems);
    problems
}

fn check_tree(node: &Node, schema: &Schema, problems: &mut Vec<String>) {
    if node.local == "Body" {
        for child in &node.children {
            check_body_child(child, schema, problems);
        }
        return;
    }
    if node.local == "Envelope" {
        for child in &node.children {
            check_tree(child, schema, problems);
        }
    }
}

fn check_body_child(node: &Node, schema: &Schema, problems: &mut Vec<String>) {
    if node.in_scan {
        if schema.elements.contains_key(&node.local) {
            validate_element(node, schema, problems);
        } else if problems.len() < 12 {
            problems.push(format!("{} is not a scan schema element", node.local));
        }
        return;
    }
    for child in &node.children {
        if child.in_scan && schema.elements.contains_key(&child.local) {
            validate_element(child, schema, problems);
        }
    }
}

fn validate_element(node: &Node, schema: &Schema, problems: &mut Vec<String>) {
    let Some(type_name) = schema.elements.get(&node.local) else {
        return;
    };
    let mut stack = Vec::new();
    let model = model_of(schema, type_name, &mut stack);
    check_model(node, &model, schema, &node.local, Ctx::default(), problems);
}

#[derive(Clone, Copy, Default)]
struct Ctx {
    /// Ancestor `ValidationInfo` has `ValidTicket=false` (Learn zero ImageInformation).
    invalid_ticket: bool,
}

fn check_model(
    node: &Node,
    model: &Model,
    schema: &Schema,
    path: &str,
    ctx: Ctx,
    problems: &mut Vec<String>,
) {
    if problems.len() >= 12 {
        return;
    }
    let ctx = validation_ctx(node, ctx);
    check_attrs(node, model, schema, path, problems);
    if let Some(simple) = &model.simple {
        if let Some(foreign) = node.children.iter().find(|child| !child.in_scan) {
            if node.children.iter().any(|child| child.in_scan) && problems.len() < 12 {
                problems.push(format!("{path} mixes scan children into a simple value"));
            }
            let _ = foreign;
        } else if learn_zero_image(node, ctx) {
            // IntOneExtType is min 1; Learn invalid-ticket ImageInformation uses 0.
        } else if !value_ok(schema, simple, node.text.trim()) && problems.len() < 12 {
            problems.push(format!(
                "{path} value '{}' does not match {}",
                node.text.trim(),
                simple.label()
            ));
        }
    }
    if element_data_is_job_token(node) {
        check_job_token_element_data(node, schema, path, problems);
        return;
    }
    if !model.particles.is_empty() || model.simple.is_none() {
        walk_particles(
            &model.particles,
            &node.children,
            schema,
            path,
            ctx,
            problems,
        );
    }
}

fn validation_ctx(node: &Node, ctx: Ctx) -> Ctx {
    if node.local != "ValidationInfo" {
        return ctx;
    }
    let invalid = node
        .children
        .iter()
        .any(|child| child.local == "ValidTicket" && matches!(child.text.trim(), "false" | "0"));
    Ctx {
        invalid_ticket: invalid,
    }
}

fn element_data_is_job_token(node: &Node) -> bool {
    node.local == "ElementData"
        && node.attrs.iter().any(|attr| {
            attr.name == "Name"
                && (attr.value == "JobToken" || attr.value.rsplit(':').next() == Some("JobToken"))
        })
}

fn check_job_token_element_data(
    node: &Node,
    schema: &Schema,
    path: &str,
    problems: &mut Vec<String>,
) {
    // JobElementDataType choice is JobStatus | ScanTicket | Documents | ##other.
    // Lab: Name=JobToken with a JobToken child, or Valid=false empty.
    match node.children.as_slice() {
        [] => {}
        [child] if child.local == "JobToken" && child.in_scan => {
            let mut stack = Vec::new();
            let model = model_of(schema, "String255ExtType", &mut stack);
            check_model(
                child,
                &model,
                schema,
                &format!("{path}/JobToken"),
                Ctx::default(),
                problems,
            );
        }
        _ if problems.len() < 12 => {
            let names: Vec<_> = node
                .children
                .iter()
                .map(|child| child.local.as_str())
                .collect();
            problems.push(format!(
                "{path} JobToken ElementData expected empty or JobToken, found {}",
                names.join(", ")
            ));
        }
        _ => {}
    }
}

fn check_attrs(
    node: &Node,
    model: &Model,
    schema: &Schema,
    path: &str,
    problems: &mut Vec<String>,
) {
    for attr in &model.attrs {
        let found = node
            .attrs
            .iter()
            .find(|item| item.name == attr.name && item.in_scan == attr.in_schema_ns);
        if attr.required && found.is_none() && problems.len() < 12 {
            problems.push(format!("{path} is missing attribute {}", attr.name));
        }
        if let Some(found) = found {
            if !value_ok(schema, &attr.simple, &found.value) && problems.len() < 12 {
                problems.push(format!(
                    "{path}/@{} value '{}' does not match {}",
                    attr.name,
                    found.value,
                    attr.simple.label()
                ));
            }
        }
    }
    for attr in &node.attrs {
        if attr.in_scan {
            let known = model
                .attrs
                .iter()
                .any(|item| item.name == attr.name && item.in_schema_ns);
            if !known && problems.len() < 12 {
                problems.push(format!("{path} has unexpected attribute {}", attr.name));
            }
        } else {
            let known = model
                .attrs
                .iter()
                .any(|item| item.name == attr.name && !item.in_schema_ns);
            let allowed = known || model.any_attr || attr.name == "lang";
            if !allowed && problems.len() < 12 {
                problems.push(format!("{path} has unexpected attribute {}", attr.name));
            }
        }
    }
}

fn walk_particles(
    particles: &[Particle],
    children: &[Node],
    schema: &Schema,
    path: &str,
    ctx: Ctx,
    problems: &mut Vec<String>,
) {
    let mut index = 0;
    for particle in particles {
        if problems.len() >= 12 {
            return;
        }
        index = consume(particle, children, index, schema, path, ctx, problems);
    }
    // JobStatusType ends with xs:any ##other. Lab replies also send same-NS
    // ImagesCompleted / ImagesToTransfer after ScansCompleted.
    if path.ends_with("JobStatus") {
        while index < children.len() && lab_job_status_extension(&children[index]) {
            index += 1;
        }
    }
    if index < children.len() && problems.len() < 12 {
        let extra: Vec<_> = children[index..]
            .iter()
            .map(|child| child.local.as_str())
            .collect();
        problems.push(format!("{path} has unexpected child {}", extra.join(", ")));
    }
}

fn lab_job_status_extension(node: &Node) -> bool {
    node.in_scan && JOB_STATUS_LAB_EXTENSIONS.contains(&node.local.as_str())
}

fn consume(
    particle: &Particle,
    children: &[Node],
    index: usize,
    schema: &Schema,
    path: &str,
    ctx: Ctx,
    problems: &mut Vec<String>,
) -> usize {
    match particle {
        Particle::Element {
            name,
            type_name,
            min,
            max,
        } => {
            let mut count = 0;
            let mut index = index;
            while index < children.len()
                && &children[index].local == name
                && children[index].in_scan
            {
                if max.is_some_and(|limit| count >= limit) {
                    break;
                }
                let mut stack = Vec::new();
                let model = model_of(schema, type_name, &mut stack);
                let child_path = format!("{path}/{name}");
                check_model(&children[index], &model, schema, &child_path, ctx, problems);
                count += 1;
                index += 1;
            }
            if count < *min && problems.len() < 12 {
                problems.push(format!("{path} is missing {name}"));
            }
            index
        }
        Particle::Any { min, max } => {
            let mut count = 0;
            let mut index = index;
            while index < children.len() {
                let child = &children[index];
                let lab = path.ends_with("JobStatus") && lab_job_status_extension(child);
                if child.in_scan && !lab {
                    break;
                }
                if max.is_some_and(|limit| count >= limit) {
                    break;
                }
                count += 1;
                index += 1;
            }
            if count < *min && problems.len() < 12 {
                problems.push(format!("{path} is missing an extension element"));
            }
            index
        }
        Particle::Choice(options) => {
            let Some(next) = children.get(index) else {
                // An `xs:any` with minOccurs 0 can be the chosen branch, so an empty choice is valid.
                if options.iter().all(|option| particle_min(option) > 0) && problems.len() < 12 {
                    problems.push(format!("{path} is missing a choice"));
                }
                return index;
            };
            if let Some(option) = options
                .iter()
                .find(|option| particle_names(option).contains(&next.local.as_str()))
            {
                return consume(option, children, index, schema, path, ctx, problems);
            }
            if options
                .iter()
                .any(|option| matches!(option, Particle::Any { .. }))
                && !next.in_scan
            {
                return index + 1;
            }
            let names: Vec<_> = options.iter().flat_map(particle_names).collect();
            if problems.len() < 12 {
                problems.push(format!(
                    "{path} choice expected {}, found {}",
                    names.join("|"),
                    next.local
                ));
            }
            index + 1
        }
    }
}

fn particle_min(particle: &Particle) -> u32 {
    match particle {
        Particle::Element { min, .. } | Particle::Any { min, .. } => *min,
        Particle::Choice(options) => options.iter().map(particle_min).min().unwrap_or(0),
    }
}

fn particle_names(particle: &Particle) -> Vec<&str> {
    match particle {
        Particle::Element { name, .. } => vec![name.as_str()],
        Particle::Any { .. } => Vec::new(),
        Particle::Choice(options) => options.iter().flat_map(particle_names).collect(),
    }
}

fn learn_zero_image(node: &Node, ctx: Ctx) -> bool {
    // IntOneExtType is min 1; Learn invalid-ticket ImageInformation uses 0.
    // BytesPerLine is IntZeroExtType (already allows 0); listed for the retain cases.
    ctx.invalid_ticket
        && node.text.trim() == "0"
        && matches!(
            node.local.as_str(),
            "PixelsPerLine" | "NumberOfLines" | "BytesPerLine"
        )
}

fn value_ok(schema: &Schema, simple: &SimpleKind, text: &str) -> bool {
    match simple {
        SimpleKind::String { max } => max.is_none_or(|limit| text.chars().count() <= limit),
        SimpleKind::Boolean => matches!(text, "true" | "false" | "1" | "0"),
        SimpleKind::Int { min } => text
            .parse::<i64>()
            .ok()
            .is_some_and(|value| min.is_none_or(|floor| value >= floor)),
        SimpleKind::DateTime => {
            let bytes = text.as_bytes();
            bytes.len() >= 20
                && bytes.get(10) == Some(&b'T')
                && text
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | 'T' | 'Z' | '+' | '.'))
        }
        SimpleKind::AnyUri => crate::wsd::envelope::absolute_iri(text),
        SimpleKind::QName => !text.is_empty() && !text.chars().any(char::is_whitespace),
        SimpleKind::Token { max, values } => {
            max.is_none_or(|limit| text.chars().count() <= limit)
                && (values.is_empty() || values.iter().any(|value| value == text))
        }
        SimpleKind::Pattern(kind) => match kind {
            Pattern::KeywordExt => keyword_ext(text),
            Pattern::MimeExt => mime_ext(text),
        },
        SimpleKind::Union(members) => members.iter().any(|member| {
            let mut stack = Vec::new();
            model_of(schema, member, &mut stack)
                .simple
                .is_some_and(|simple| value_ok(schema, &simple, text))
        }),
        SimpleKind::NmTokens => !text.split_whitespace().any(|token| token.is_empty()),
        SimpleKind::Base64 => text.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '\n' | '\r' | ' ' | '\t')
        }),
    }
}

fn keyword_ext(text: &str) -> bool {
    let Some((left, right)) = text.split_once(':') else {
        return false;
    };
    !left.is_empty()
        && left.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !right.is_empty()
        && right
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn mime_ext(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '/' | ';' | '-' | '\\' | '+' | '.')
    })
}

fn model_of(schema: &Schema, type_name: &str, stack: &mut Vec<String>) -> Model {
    if stack.iter().any(|name| name == type_name) {
        return Model::default();
    }
    if let Some(builtin) = builtin(type_name) {
        return Model {
            simple: Some(builtin),
            ..Model::default()
        };
    }
    let Some(body) = schema.types.get(type_name) else {
        return Model::default();
    };
    stack.push(type_name.to_string());
    let model = match body {
        TypeBody::Simple(simple) => Model {
            simple: Some(simple.clone()),
            ..Model::default()
        },
        TypeBody::Complex {
            base,
            mode,
            particles,
            attrs,
        } => {
            let parent = base
                .as_deref()
                .map(|name| model_of(schema, name, stack))
                .unwrap_or_default();
            match mode {
                Mode::Sequence => Model {
                    particles: particles.clone(),
                    attrs: attrs.clone(),
                    any_attr: true,
                    ..Model::default()
                },
                Mode::SimpleContent => Model {
                    simple: parent.simple.clone(),
                    attrs: merge_attrs(&parent.attrs, attrs),
                    any_attr: true,
                    ..Model::default()
                },
                Mode::ComplexContent => {
                    let mut combined = parent.particles.clone();
                    combined.extend(particles.iter().cloned());
                    Model {
                        particles: combined,
                        simple: parent.simple.clone(),
                        attrs: merge_attrs(&parent.attrs, attrs),
                        any_attr: true,
                    }
                }
            }
        }
    };
    stack.pop();
    model
}

fn merge_attrs(parent: &[AttrUse], own: &[AttrUse]) -> Vec<AttrUse> {
    let mut attrs = parent.to_vec();
    for attr in own {
        if let Some(slot) = attrs
            .iter_mut()
            .find(|item| item.name == attr.name && item.in_schema_ns == attr.in_schema_ns)
        {
            *slot = attr.clone();
        } else {
            attrs.push(attr.clone());
        }
    }
    attrs
}

fn builtin(type_name: &str) -> Option<SimpleKind> {
    Some(match type_name {
        "xs:boolean" => SimpleKind::Boolean,
        "xs:int" => SimpleKind::Int { min: None },
        "xs:string" => SimpleKind::String { max: None },
        "xs:dateTime" => SimpleKind::DateTime,
        "xs:anyURI" => SimpleKind::AnyUri,
        "xs:QName" => SimpleKind::QName,
        "xs:NMTOKEN" => SimpleKind::Token {
            max: None,
            values: Vec::new(),
        },
        "xs:NMTOKENS" => SimpleKind::NmTokens,
        "xs:base64Binary" => SimpleKind::Base64,
        _ => return None,
    })
}

#[derive(Clone, Default)]
struct Model {
    particles: Vec<Particle>,
    simple: Option<SimpleKind>,
    attrs: Vec<AttrUse>,
    any_attr: bool,
}

#[derive(Clone)]
enum Particle {
    Element {
        name: String,
        type_name: String,
        min: u32,
        max: Option<u32>,
    },
    Any {
        min: u32,
        max: Option<u32>,
    },
    Choice(Vec<Particle>),
}

#[derive(Clone)]
struct AttrUse {
    name: String,
    in_schema_ns: bool,
    required: bool,
    simple: SimpleKind,
}

#[derive(Clone)]
enum SimpleKind {
    String {
        max: Option<usize>,
    },
    Boolean,
    Int {
        min: Option<i64>,
    },
    DateTime,
    AnyUri,
    QName,
    Token {
        max: Option<usize>,
        values: Vec<String>,
    },
    Pattern(Pattern),
    Union(Vec<String>),
    NmTokens,
    Base64,
}

impl SimpleKind {
    fn label(&self) -> String {
        match self {
            SimpleKind::String { max } => match max {
                Some(limit) => format!("string({limit})"),
                None => "string".into(),
            },
            SimpleKind::Boolean => "boolean".into(),
            SimpleKind::Int { min } => match min {
                Some(floor) => format!("int>={floor}"),
                None => "int".into(),
            },
            SimpleKind::DateTime => "dateTime".into(),
            SimpleKind::AnyUri => "anyURI".into(),
            SimpleKind::QName => "QName".into(),
            SimpleKind::Token { values, .. } if !values.is_empty() => values.join("|"),
            SimpleKind::Token { .. } => "NMTOKEN".into(),
            SimpleKind::Pattern(Pattern::KeywordExt) => "namespace keyword".into(),
            SimpleKind::Pattern(Pattern::MimeExt) => "mime token".into(),
            SimpleKind::Union(members) => members.join(" or "),
            SimpleKind::NmTokens => "NMTOKENS".into(),
            SimpleKind::Base64 => "base64".into(),
        }
    }
}

#[derive(Clone, Copy)]
enum Pattern {
    KeywordExt,
    MimeExt,
}

enum TypeBody {
    Simple(SimpleKind),
    Complex {
        base: Option<String>,
        mode: Mode,
        particles: Vec<Particle>,
        attrs: Vec<AttrUse>,
    },
}

#[derive(Clone, Copy)]
enum Mode {
    Sequence,
    SimpleContent,
    ComplexContent,
}

struct Schema {
    elements: HashMap<String, String>,
    types: HashMap<String, TypeBody>,
    attributes: HashMap<String, SimpleKind>,
}

fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(load_schema)
}

fn load_schema() -> Schema {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/schema");
    let mut roots = Vec::new();
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|error| panic!("scan schema {}: {error}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("xsd") {
            continue;
        }
        let text =
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if let Some(root) = xml::parse(&text).map(|el| element_to_node(&el)) {
            roots.push(root);
        }
    }
    let mut schema = Schema {
        elements: HashMap::new(),
        types: HashMap::new(),
        attributes: HashMap::new(),
    };
    for root in &roots {
        for child in &root.children {
            match child.local.as_str() {
                "simpleType" => {
                    if let Some(name) = child.attr("name") {
                        schema
                            .types
                            .insert(name.to_string(), TypeBody::Simple(simple_kind(child)));
                    }
                }
                "attribute" => {
                    if let Some(name) = child.attr("name") {
                        let simple = child
                            .attr("type")
                            .map(simple_from_type)
                            .unwrap_or(SimpleKind::String { max: None });
                        schema.attributes.insert(name.to_string(), simple);
                    }
                }
                _ => {}
            }
        }
    }
    let mut anon = 0u32;
    for root in &roots {
        for child in &root.children {
            match child.local.as_str() {
                "complexType" => {
                    if let Some(name) = child.attr("name") {
                        let body = complex_body(child, &mut schema, &mut anon);
                        schema.types.insert(name.to_string(), body);
                    }
                }
                "element" => {
                    declare_element(child, &mut schema, &mut anon, true);
                }
                _ => {}
            }
        }
    }
    schema
}

fn declare_element(node: &Node, schema: &mut Schema, anon: &mut u32, global: bool) -> String {
    let name = node.attr("name").unwrap_or("").to_string();
    let type_name = if let Some(explicit) = node.attr("type") {
        type_key(explicit)
    } else if let Some(inline) = node.child("complexType") {
        *anon += 1;
        let type_name = format!("anon{anon}");
        let body = complex_body(inline, schema, anon);
        schema.types.insert(type_name.clone(), body);
        type_name
    } else {
        "xs:string".into()
    };
    if global && !name.is_empty() {
        schema.elements.insert(name, type_name.clone());
    }
    type_name
}

fn complex_body(node: &Node, schema: &mut Schema, anon: &mut u32) -> TypeBody {
    if let Some(simple) = node
        .child("simpleContent")
        .and_then(|content| content.child("extension"))
    {
        return TypeBody::Complex {
            base: simple.attr("base").map(type_key),
            mode: Mode::SimpleContent,
            particles: Vec::new(),
            attrs: attrs_of(simple, schema),
        };
    }
    if let Some(complex) = node
        .child("complexContent")
        .and_then(|content| content.child("extension"))
    {
        let particles = complex
            .child("sequence")
            .map(|seq| particles_of(seq, schema, anon))
            .unwrap_or_default();
        return TypeBody::Complex {
            base: complex.attr("base").map(type_key),
            mode: Mode::ComplexContent,
            particles,
            attrs: attrs_of(complex, schema),
        };
    }
    let particles = node
        .child("sequence")
        .or_else(|| node.child("choice"))
        .map(|content| particles_of(content, schema, anon))
        .unwrap_or_default();
    let mut attrs = attrs_of(node, schema);
    if node.child("choice").is_some() && node.child("sequence").is_none() {
        return TypeBody::Complex {
            base: None,
            mode: Mode::Sequence,
            particles: vec![Particle::Choice(particles)],
            attrs,
        };
    }
    let _ = &mut attrs;
    TypeBody::Complex {
        base: None,
        mode: Mode::Sequence,
        particles,
        attrs,
    }
}

fn particles_of(node: &Node, schema: &mut Schema, anon: &mut u32) -> Vec<Particle> {
    let mut particles = Vec::new();
    for child in &node.children {
        match child.local.as_str() {
            "element" => {
                let name = child.attr("name").unwrap_or("").to_string();
                let type_name = declare_element(child, schema, anon, false);
                particles.push(Particle::Element {
                    name,
                    type_name,
                    min: occurs(child, "minOccurs", 1),
                    max: max_occurs(child),
                });
            }
            "any" => particles.push(Particle::Any {
                min: occurs(child, "minOccurs", 1),
                max: max_occurs(child),
            }),
            "choice" => particles.push(Particle::Choice(particles_of(child, schema, anon))),
            _ => {}
        }
    }
    particles
}

fn attrs_of(node: &Node, schema: &Schema) -> Vec<AttrUse> {
    let mut attrs = Vec::new();
    for child in &node.children {
        if child.local != "attribute" {
            continue;
        }
        if let Some(reference) = child.attr("ref") {
            let name = reference
                .rsplit(':')
                .next()
                .unwrap_or(reference)
                .to_string();
            let simple = schema
                .attributes
                .get(&name)
                .cloned()
                .unwrap_or(SimpleKind::String { max: None });
            attrs.push(AttrUse {
                name,
                in_schema_ns: true,
                required: child.attr("use") == Some("required"),
                simple,
            });
        } else if let Some(name) = child.attr("name") {
            let simple = child
                .attr("type")
                .map(|value| simple_from_name(schema, value))
                .unwrap_or(SimpleKind::String { max: None });
            attrs.push(AttrUse {
                name: name.to_string(),
                in_schema_ns: false,
                required: child.attr("use") == Some("required"),
                simple,
            });
        }
    }
    attrs
}

fn simple_from_name(schema: &Schema, type_name: &str) -> SimpleKind {
    let key = type_key(type_name);
    if let Some(builtin) = builtin(&key) {
        return builtin;
    }
    if let Some(TypeBody::Simple(simple)) = schema.types.get(&key) {
        return simple.clone();
    }
    SimpleKind::String { max: None }
}

fn simple_from_type(type_name: &str) -> SimpleKind {
    builtin(&type_key(type_name)).unwrap_or(SimpleKind::String { max: None })
}

fn simple_kind(node: &Node) -> SimpleKind {
    if let Some(union_node) = node.child("union") {
        let members = union_node
            .attr("memberTypes")
            .unwrap_or("")
            .split_whitespace()
            .map(type_key)
            .collect();
        return SimpleKind::Union(members);
    }
    let Some(restriction) = node.child("restriction") else {
        return SimpleKind::String { max: None };
    };
    let base = restriction
        .attr("base")
        .map(type_key)
        .unwrap_or_else(|| "xs:string".into());
    let enums: Vec<String> = restriction
        .children
        .iter()
        .filter(|child| child.local == "enumeration")
        .filter_map(|child| child.attr("value").map(str::to_string))
        .collect();
    let max = restriction
        .child_attr("maxLength", "value")
        .and_then(|value| value.parse().ok());
    let min = restriction
        .child_attr("minInclusive", "value")
        .and_then(|value| value.parse().ok());
    if let Some(pattern) = restriction.child_attr("pattern", "value") {
        if pattern.contains(':') {
            return SimpleKind::Pattern(Pattern::KeywordExt);
        }
        return SimpleKind::Pattern(Pattern::MimeExt);
    }
    if !enums.is_empty() || base == "xs:NMTOKEN" {
        return SimpleKind::Token { max, values: enums };
    }
    if base == "xs:int" || min.is_some() {
        return SimpleKind::Int { min };
    }
    if base == "xs:boolean" {
        return SimpleKind::Boolean;
    }
    SimpleKind::String { max }
}

fn occurs(node: &Node, name: &str, default: u32) -> u32 {
    node.attr(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn max_occurs(node: &Node) -> Option<u32> {
    match node.attr("maxOccurs") {
        None => Some(1),
        Some("unbounded") => None,
        Some(value) => value.parse().ok(),
    }
}

fn type_key(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("xs:") {
        format!("xs:{rest}")
    } else {
        value.rsplit(':').next().unwrap_or(value).to_string()
    }
}

struct Node {
    local: String,
    in_scan: bool,
    attrs: Vec<Attr>,
    children: Vec<Node>,
    text: String,
}

struct Attr {
    name: String,
    in_scan: bool,
    value: String,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|attr| attr.name == name)
            .map(|attr| attr.value.as_str())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.local == name)
    }

    fn child_attr(&self, child: &str, name: &str) -> Option<&str> {
        self.child(child).and_then(|node| node.attr(name))
    }
}

fn xml_document(input: &str) -> &str {
    if let Some(end) = input.find("</soap:Envelope>") {
        &input[..end + "</soap:Envelope>".len()]
    } else if let Some(end) = input.find("</Envelope>") {
        &input[..end + "</Envelope>".len()]
    } else {
        input
    }
}

fn element_to_node(el: &Element) -> Node {
    Node {
        local: el.local.clone(),
        in_scan: el.in_scan_ns(),
        attrs: el
            .attrs
            .iter()
            .filter(|a| a.local != "xmlns")
            .map(|a| Attr {
                name: a.local.clone(),
                in_scan: a
                    .ns
                    .as_deref()
                    .is_some_and(|uri| uri.contains(SCAN_NS_SUFFIX)),
                value: a.value.clone(),
            })
            .collect(),
        children: el.children.iter().map(element_to_node).collect(),
        text: el.text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_lists_the_scan_operations() {
        let schema = schema();
        for name in [
            "CreateScanJobRequest",
            "CreateScanJobResponse",
            "GetScannerElementsResponse",
            "ValidateScanTicketResponse",
        ] {
            assert!(schema.elements.contains_key(name), "{name}");
        }
    }

    #[test]
    fn rejects_a_scanner_state_outside_the_schema() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:GetScannerElementsResponse><wscn:ScannerElements><wscn:ElementData Name="wscn:ScannerStatus" Valid="true">
            <wscn:ScannerStatus>
              <wscn:ScannerCurrentTime>2026-10-12T15:20:11Z</wscn:ScannerCurrentTime>
              <wscn:ScannerState>Busy</wscn:ScannerState>
              <wscn:ScannerStateReasons><wscn:ScannerStateReason>None</wscn:ScannerStateReason></wscn:ScannerStateReasons>
              <wscn:ActiveConditions/>
            </wscn:ScannerStatus>
          </wscn:ElementData></wscn:ScannerElements></wscn:GetScannerElementsResponse></soap:Body>
        </soap:Envelope>"#;
        let problems = body_problems(xml);
        assert!(
            problems.iter().any(|problem| problem.contains("Busy")),
            "{problems:?}"
        );
    }

    #[test]
    fn accepts_idle_status_in_the_2006_01_namespace() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:GetScannerElementsResponse><wscn:ScannerElements><wscn:ElementData Name="wscn:ScannerStatus" Valid="true">
            <wscn:ScannerStatus>
              <wscn:ScannerCurrentTime>2026-10-12T15:20:11Z</wscn:ScannerCurrentTime>
              <wscn:ScannerState>Idle</wscn:ScannerState>
              <wscn:ScannerStateReasons><wscn:ScannerStateReason>None</wscn:ScannerStateReason></wscn:ScannerStateReasons>
              <wscn:ActiveConditions/>
            </wscn:ScannerStatus>
          </wscn:ElementData></wscn:ScannerElements></wscn:GetScannerElementsResponse></soap:Body>
        </soap:Envelope>"#;
        assert!(body_problems(xml).is_empty(), "{:?}", body_problems(xml));
    }

    #[test]
    fn wia_shaped_request_is_reported_and_not_rewritten() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:CreateScanJobRequest><wscn:ScanTicket><wscn:DocumentParameters>
            <wscn:Format>jfif</wscn:Format>
            <wscn:InputSource>Feeder</wscn:InputSource>
            <wscn:Resolution><wscn:Width>300</wscn:Width><wscn:Height>300</wscn:Height></wscn:Resolution>
          </wscn:DocumentParameters></wscn:ScanTicket></wscn:CreateScanJobRequest></soap:Body>
        </soap:Envelope>"#;
        let problems = body_problems(xml);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("missing JobDescription")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("Feeder")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("unexpected child Resolution")),
            "{problems:?}"
        );
    }

    #[test]
    fn retrieve_without_document_description_is_reported() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:RetrieveImageRequest>
            <wscn:JobId>1</wscn:JobId>
            <wscn:JobToken>urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</wscn:JobToken>
          </wscn:RetrieveImageRequest></soap:Body>
        </soap:Envelope>"#;
        let problems = body_problems(xml);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("missing DocumentDescription")),
            "{problems:?}"
        );
    }

    #[test]
    fn invalid_ticket_image_counts_may_be_zero() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:ValidateScanTicketResponse><wscn:ValidationInfo>
            <wscn:ValidTicket>false</wscn:ValidTicket>
            <wscn:ImageInformation><wscn:MediaFrontImageInfo>
              <wscn:PixelsPerLine>0</wscn:PixelsPerLine>
              <wscn:NumberOfLines>0</wscn:NumberOfLines>
              <wscn:BytesPerLine>0</wscn:BytesPerLine>
            </wscn:MediaFrontImageInfo></wscn:ImageInformation>
          </wscn:ValidationInfo></wscn:ValidateScanTicketResponse></soap:Body>
        </soap:Envelope>"#;
        assert!(body_problems(xml).is_empty(), "{:?}", body_problems(xml));
    }

    #[test]
    fn valid_ticket_rejects_zero_pixels_per_line() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:ValidateScanTicketResponse><wscn:ValidationInfo>
            <wscn:ValidTicket>true</wscn:ValidTicket>
            <wscn:ImageInformation><wscn:MediaFrontImageInfo>
              <wscn:PixelsPerLine>0</wscn:PixelsPerLine>
              <wscn:NumberOfLines>1</wscn:NumberOfLines>
              <wscn:BytesPerLine>0</wscn:BytesPerLine>
            </wscn:MediaFrontImageInfo></wscn:ImageInformation>
          </wscn:ValidationInfo></wscn:ValidateScanTicketResponse></soap:Body>
        </soap:Envelope>"#;
        let problems = body_problems(xml);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("PixelsPerLine") && p.contains("'0'")),
            "{problems:?}"
        );
    }

    #[test]
    fn unknown_job_element_may_be_an_empty_choice() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:GetJobElementsResponse><wscn:JobElements>
            <wscn:ElementData Name="wscn:JobToken" Valid="false"></wscn:ElementData>
          </wscn:JobElements></wscn:GetJobElementsResponse></soap:Body>
        </soap:Envelope>"#;
        assert!(body_problems(xml).is_empty(), "{:?}", body_problems(xml));
    }

    #[test]
    fn job_token_element_data_may_carry_a_job_token_child() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:GetJobElementsResponse><wscn:JobElements>
            <wscn:ElementData Name="wscn:JobToken" Valid="true">
              <wscn:JobToken>urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</wscn:JobToken>
            </wscn:ElementData>
          </wscn:JobElements></wscn:GetJobElementsResponse></soap:Body>
        </soap:Envelope>"#;
        assert!(body_problems(xml).is_empty(), "{:?}", body_problems(xml));
    }

    #[test]
    fn job_status_may_carry_lab_images_counters() {
        let xml = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
          <soap:Body><wscn:JobStatusEvent>
            <wscn:JobStatus>
              <wscn:JobId>1</wscn:JobId>
              <wscn:JobState>Processing</wscn:JobState>
              <wscn:JobStateReasons><wscn:JobStateReason>JobScanning</wscn:JobStateReason></wscn:JobStateReasons>
              <wscn:ScansCompleted>1</wscn:ScansCompleted>
              <wscn:ImagesCompleted>1</wscn:ImagesCompleted>
              <wscn:ImagesToTransfer>2</wscn:ImagesToTransfer>
            </wscn:JobStatus>
          </wscn:JobStatusEvent></soap:Body>
        </soap:Envelope>"#;
        assert!(body_problems(xml).is_empty(), "{:?}", body_problems(xml));
    }

    #[test]
    fn resolution_override_on_the_pair_is_rejected() {
        // ResolutionPairOverrideType is Width and Height. Override sits on those values.
        let bad = r#"<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope" xmlns:wscn="http://schemas.microsoft.com/windows/2006/08/wdp/scan">
          <soap:Body><wscn:CreateScanJobResponse>
            <wscn:JobId>1</wscn:JobId><wscn:JobToken>urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</wscn:JobToken>
            <wscn:ImageInformation><wscn:MediaFrontImageInfo>
              <wscn:PixelsPerLine>1</wscn:PixelsPerLine><wscn:NumberOfLines>1</wscn:NumberOfLines><wscn:BytesPerLine>0</wscn:BytesPerLine>
            </wscn:MediaFrontImageInfo></wscn:ImageInformation>
            <wscn:DocumentFinalParameters>
              <wscn:Format>jfif</wscn:Format>
              <wscn:CompressionQualityFactor wscn:UsedDefault="true">100</wscn:CompressionQualityFactor>
              <wscn:ImagesToTransfer>1</wscn:ImagesToTransfer>
              <wscn:InputSource>Platen</wscn:InputSource>
              <wscn:ContentType wscn:UsedDefault="true">Auto</wscn:ContentType>
              <wscn:InputSize><wscn:InputMediaSize><wscn:Width>8500</wscn:Width><wscn:Height>11000</wscn:Height></wscn:InputMediaSize></wscn:InputSize>
              <wscn:Exposure><wscn:ExposureSettings><wscn:Contrast>0</wscn:Contrast><wscn:Brightness>0</wscn:Brightness><wscn:Sharpness wscn:UsedDefault="true">0</wscn:Sharpness></wscn:ExposureSettings></wscn:Exposure>
              <wscn:Scaling><wscn:ScalingWidth>100</wscn:ScalingWidth><wscn:ScalingHeight>100</wscn:ScalingHeight></wscn:Scaling>
              <wscn:Rotation>0</wscn:Rotation>
              <wscn:MediaSides><wscn:MediaFront>
                <wscn:ScanRegion><wscn:ScanRegionXOffset>0</wscn:ScanRegionXOffset><wscn:ScanRegionYOffset>0</wscn:ScanRegionYOffset><wscn:ScanRegionWidth>8500</wscn:ScanRegionWidth><wscn:ScanRegionHeight>11000</wscn:ScanRegionHeight></wscn:ScanRegion>
                <wscn:ColorProcessing>RGB24</wscn:ColorProcessing>
                <wscn:Resolution wscn:Override="true"><wscn:Width>300</wscn:Width><wscn:Height>300</wscn:Height></wscn:Resolution>
              </wscn:MediaFront></wscn:MediaSides>
            </wscn:DocumentFinalParameters>
          </wscn:CreateScanJobResponse></soap:Body>
        </soap:Envelope>"#;
        let bad_problems = body_problems(bad);
        assert!(
            bad_problems
                .iter()
                .any(|p| p.contains("unexpected attribute Override")),
            "{bad_problems:?}"
        );
        let good = bad.replace(
            r#"<wscn:Resolution wscn:Override="true"><wscn:Width>300</wscn:Width><wscn:Height>300</wscn:Height>"#,
            r#"<wscn:Resolution><wscn:Width wscn:Override="true">300</wscn:Width><wscn:Height wscn:Override="true">300</wscn:Height>"#,
        );
        assert!(
            body_problems(&good).is_empty(),
            "{:?}",
            body_problems(&good)
        );
    }
}
