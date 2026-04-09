1. **Add `execute_and_check_success` to `session.rs`**
   - Create a new helper method on `Session`:
     ```rust
     pub(crate) async fn execute_and_check_success(
         &self,
         request: reqwest::Request,
         fallback_error_message: &str,
     ) -> crate::error::Result<reqwest::Response> {
         let response = self.execute_request(request).await?;
         if !response.status().is_success() {
             return Err(crate::http::response_to_force_error(response, fallback_error_message).await);
         }
         Ok(response)
     }
     ```
   - This avoids boilerplate of calling `execute_request`, checking `response.status().is_success()`, and generating an error with `crate::http::response_to_force_error`.

2. **Refactor `send_request_and_decode` to use the new helper**
   - In `session.rs`, refactor `send_request_and_decode` to use `execute_and_check_success` internally.

3. **Refactor REST CRUD delete/update methods**
   - In `crates/force/src/api/rest_operation.rs` (`update` and `delete` methods), use `execute_and_check_success` instead of the manual status check boilerplate. Note that since these methods return unit-like responses `UpdateResponse::success()` and `DeleteResponse::success()`, we just map the successful result.

4. **Refactor Bulk Ingest methods**
   - In `crates/force/src/api/bulk/ingest.rs` (`execute_job_request` method and others if any), replace the `is_success()` boilerplate with `execute_and_check_success`.

5. **Refactor Bulk Query methods**
   - In `crates/force/src/api/bulk/query.rs` (`execute_fetch_request` and `delete_query_job` methods), replace the `is_success()` boilerplate with `execute_and_check_success`.

6. **Refactor Bulk Smart Ingest methods**
   - In `crates/force/src/api/bulk/smart_ingest.rs` (`upload_batch` method), replace the `is_success()` boilerplate with `execute_and_check_success`.

7. **Refactor UI API methods**
   - In `crates/force/src/api/ui/mod.rs` (`execute_request` internal method or similar), replace the boilerplate.

8. **Refactor GraphQL API methods**
   - In `crates/force/src/api/graphql/mod.rs`, replace the boilerplate in `execute` methods.

9. **Refactor Apex REST API methods**
   - In `crates/force/src/api/apex_rest/mod.rs` (`delete` method), replace the boilerplate.

10. **Refactor `execute_request_with_retry_class` case**
    - Similar boilerplate exists for `execute_request_with_retry_class`? We can add an `execute_with_retry_class_and_check_success` if needed, but it seems there's only one use case in `rest_operation.rs::upsert_with_retry_class_impl` (which also checks for `204`). Wait, that one checks `204` manually, so we don't change it.

11. **Run tests & clippy**
    - Verify tests are still passing.

12. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
    - Run the required pre-commit checks.

13. **Finalize and submit the PR**
    - Submit the PR with the title "⚒️ Forge: Extract HTTP success checking into Session helper" and the required description elements.
