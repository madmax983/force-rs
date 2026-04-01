//! Common path formatting utilities.

/// Formats a relative SObject path, optionally including an ID.
///
/// If `id` is provided, returns `sobjects/{sobject}/{id}`.
/// If `id` is not provided, returns `sobjects/{sobject}`.
pub fn format_sobject_path(sobject: &str, id: Option<&str>) -> String {
    // ⚡ Bolt: Avoid intermediate format! allocation
    let mut capacity = 9 + sobject.len(); // "sobjects/".len() == 9
    if let Some(record_id) = id {
        capacity += 1 + record_id.len(); // "/" + record_id
    }

    let mut path = String::with_capacity(capacity);
    path.push_str("sobjects/");
    path.push_str(sobject);

    if let Some(record_id) = id {
        path.push('/');
        path.push_str(record_id);
    }

    path
}
