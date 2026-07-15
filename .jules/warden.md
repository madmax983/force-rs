**Path Traversal in DataArchiver**
**Threat:** Path Traversal vulnerability allowing arbitrary file writes via unsanitized user-provided paths containing parent directory traversal (`../`).
**Defense:** Validated all file paths using `path.as_ref().components().any(|c| matches!(c, std::path::Component::ParentDir))` and returned `ForceError::InvalidInput` if found.
**Severity:** High - Could allow overwriting critical system files or configurations if the process runs with sufficient privileges.
**Verification:** Added test cases `test_export_to_jsonl_path_traversal` and `test_export_masked_to_jsonl_path_traversal` sending paths with `../` to ensure they are blocked. Verified via `cargo test`.
