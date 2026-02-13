# Warden's Journal

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder]
**Threat:** The `SearchQueryBuilder` blindly concatenates user input into the SOSL query string without escaping reserved characters. This allows an attacker to inject arbitrary SOSL clauses (e.g., breaking out of the `FIND` clause to modify `RETURNING` objects or access unauthorized fields).
**Defense:** Implement proper escaping for SOSL reserved characters (`? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -`) in the `find` method or during `build`.
