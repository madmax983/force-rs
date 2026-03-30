**Query Pagination Edge Case**
**Learning:** A Salesforce `QueryResult` may incorrectly report `done: false` without providing a `next_records_url`. Previously, the code's `has_more` blindly trusted `done: false`, causing downstream functions to potentially panic upon `unwrap()` of the missing URL.
**Action:** Always strictly validate interdependent API response fields. We updated `QueryResult::has_more` to verify `!self.done && self.next_records_url.is_some()` safely instead of trusting `done` alone.
