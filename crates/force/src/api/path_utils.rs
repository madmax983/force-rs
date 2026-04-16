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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sobject_path_with_id() {
        assert_eq!(
            format_sobject_path("Account", Some("001000000000000")),
            "sobjects/Account/001000000000000"
        );
    }

    #[test]
    fn test_format_sobject_path_without_id() {
        assert_eq!(format_sobject_path("Account", None), "sobjects/Account");
    }
}
