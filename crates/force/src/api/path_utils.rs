//! Common path formatting utilities.

/// Formats a relative SObject path, optionally including an ID.
///
/// If `id` is provided, returns `sobjects/{sobject}/{id}`.
/// If `id` is not provided, returns `sobjects/{sobject}`.
pub fn format_sobject_path(sobject: &str, id: Option<&str>) -> String {
    match id {
        Some(record_id) => format!("sobjects/{}/{}", sobject, record_id),
        None => format!("sobjects/{}", sobject),
    }
}

/// Formats an absolute SObject path, optionally including an ID.
///
/// If `id` is provided, returns `/sobjects/{sobject}/{id}`.
/// If `id` is not provided, returns `/sobjects/{sobject}`.
pub fn format_absolute_sobject_path(sobject: &str, id: Option<&str>) -> String {
    match id {
        Some(record_id) => format!("/sobjects/{}/{}", sobject, record_id),
        None => format!("/sobjects/{}", sobject),
    }
}

/// Formats an absolute SObject path for a describe request.
pub fn format_absolute_describe_path(sobject: &str) -> String {
    format!("/sobjects/{}/describe", sobject)
}
