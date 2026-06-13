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

/// Formats a relative bulk ingest job path.
///
/// Returns `jobs/ingest/{job_id}`.
pub fn format_ingest_job_path(job_id: &str) -> String {
    let mut path = String::with_capacity(12 + job_id.len());
    path.push_str("jobs/ingest/");
    path.push_str(job_id);
    path
}

/// Formats a relative bulk query job path, optionally including a suffix.
///
/// If `suffix` is provided, returns `jobs/query/{job_id}/{suffix}`.
/// If `suffix` is not provided, returns `jobs/query/{job_id}`.
pub fn format_query_job_path(job_id: &str, suffix: Option<&str>) -> String {
    let mut capacity = 11 + job_id.len();
    if let Some(s) = suffix {
        capacity += 1 + s.len();
    }

    let mut path = String::with_capacity(capacity);
    path.push_str("jobs/query/");
    path.push_str(job_id);

    if let Some(s) = suffix {
        path.push('/');
        path.push_str(s);
    }

    path
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
