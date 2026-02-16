# Warden's Journal

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder]
**Threat:** The `SearchQueryBuilder` blindly concatenates user input into the SOSL query string without escaping reserved characters. This allows an attacker to inject arbitrary SOSL clauses (e.g., breaking out of the `FIND` clause to modify `RETURNING` objects or access unauthorized fields).
**Defense:** Implement proper escaping for SOSL reserved characters (`? & | ! { } [ ] ( ) ^ ~ * : \ " ' + -`) in the `find` method or during `build`.

## 2024-06-25 - [SOSL Injection in SearchQueryBuilder::returning]
**Threat:** The `SearchQueryBuilder::returning` method accepted arbitrary strings for `sobject` and `fields`, allowing injection of SOSL clauses (e.g., `Account) OR (Contact`) or breaking out of the object scope to query unauthorized objects.
**Defense:** Added `is_valid_sobject` and `is_valid_field` validators that enforce alphanumeric names for SObjects and balanced parentheses/safe structure for fields, panicking on invalid input.
