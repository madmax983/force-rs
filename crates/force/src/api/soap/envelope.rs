//! SOAP request envelope construction for the Partner API.
//!
//! Envelopes are built by direct string assembly with every caller-supplied
//! text value XML-escaped via [`quick_xml::escape::escape`]. The `SessionHeader`
//! carries the OAuth access token as the `sessionId`.

use super::types::SObject;

/// The SOAP envelope namespace.
const NS_ENVELOPE: &str = "http://schemas.xmlsoap.org/soap/envelope/";
/// The Partner API operations namespace.
const NS_PARTNER: &str = "urn:partner.soap.sforce.com";
/// The Partner sObject namespace.
const NS_SOBJECT: &str = "urn:sobject.partner.soap.sforce.com";
/// The XML Schema instance namespace (for `xsi:type`/`xsi:nil`).
const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// XML-escapes a caller-supplied text value for safe inclusion as element content.
pub fn escape_text(value: &str) -> String {
    quick_xml::escape::escape(value).into_owned()
}

/// Wraps an operation body in a full SOAP envelope with a `SessionHeader`.
///
/// * `session_id` — the OAuth access token, placed in `<urn:sessionId>`.
/// * `client` — an optional `CallOptions` client string (≤ 32 chars).
/// * `body` — the pre-rendered `<urn:operation>…</urn:operation>` markup.
pub fn build_envelope(session_id: &str, client: Option<&str>, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + session_id.len() + 512);
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    out.push_str("<soapenv:Envelope xmlns:soapenv=\"");
    out.push_str(NS_ENVELOPE);
    out.push_str("\" xmlns:urn=\"");
    out.push_str(NS_PARTNER);
    out.push_str("\" xmlns:sf=\"");
    out.push_str(NS_SOBJECT);
    out.push_str("\" xmlns:xsi=\"");
    out.push_str(NS_XSI);
    out.push_str("\"><soapenv:Header><urn:SessionHeader><urn:sessionId>");
    out.push_str(&escape_text(session_id));
    out.push_str("</urn:sessionId></urn:SessionHeader>");
    if let Some(client) = client {
        out.push_str("<urn:CallOptions><urn:client>");
        out.push_str(&escape_text(client));
        out.push_str("</urn:client></urn:CallOptions>");
    }
    out.push_str("</soapenv:Header><soapenv:Body>");
    out.push_str(body);
    out.push_str("</soapenv:Body></soapenv:Envelope>");
    out
}

/// Serializes a single record as a `<urn:sObjects>` element.
///
/// Emits `<sf:type>` first, then each non-null field, then any `fieldsToNull`
/// entries. Field and type values are XML-escaped; field API names are used as
/// element names and are assumed to be valid Salesforce identifiers.
pub fn serialize_sobject(out: &mut String, obj: &SObject) {
    out.push_str("<urn:sObjects><sf:type>");
    out.push_str(&escape_text(&obj.sobject_type));
    out.push_str("</sf:type>");
    for (name, value) in &obj.fields {
        let Some(value) = value else {
            continue;
        };
        out.push_str("<sf:");
        out.push_str(name);
        out.push('>');
        out.push_str(&escape_text(value));
        out.push_str("</sf:");
        out.push_str(name);
        out.push('>');
    }
    for field in &obj.fields_to_null {
        out.push_str("<sf:fieldsToNull>");
        out.push_str(&escape_text(field));
        out.push_str("</sf:fieldsToNull>");
    }
    out.push_str("</urn:sObjects>");
}
