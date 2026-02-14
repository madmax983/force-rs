# Warden's Journal

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder]
**Threat:** The `SearchQueryBuilder` blindly concatenates user input into the SOSL query string without escaping reserved characters. This allows an attacker to inject arbitrary SOSL clauses (e.g., breaking out of the `FIND` clause to modify `RETURNING` objects or access unauthorized fields).
**Defense:** Implement proper escaping for SOSL reserved characters (`? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -`) in the `find` method or during `build`.

## 2024-06-25 - [DoS via Unbounded Allocation in CSV Processing]
**Threat:** The `process_csv_batches` function used `Vec::with_capacity(batch_size)` with user-supplied `batch_size`. An attacker could supply a huge `batch_size` (e.g., via configuration) to trigger a massive memory allocation, causing a panic or OOM crash (Denial of Service).
**Defense:** Capped the initial allocation of the batch vector to `min(batch_size, 1024)`. The vector will grow dynamically as needed, preventing immediate OOM while maintaining performance for small batches.
