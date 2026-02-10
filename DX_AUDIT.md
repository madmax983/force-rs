# 🗣️ Echo: DX Audit Report

I tried to follow the `README.md` instructions, and everything exploded. Here is what happened.

## 1. Quick Start Example (`echo_quick_start.rs`)

**Status:** ❌ FAILED TO COMPILE

**The Confusion:**
I copied the "Quick Start" code exactly. It failed with:
1. `ClientCredentials::new` expects 3 arguments, but only 2 were provided.
2. `client.rest().query_typed` method not found.

**The Reality:**
- `ClientCredentials::new` requires `token_url` as the 3rd argument.
- `RestHandler` (returned by `client.rest()`) does not have `query_typed`. The method `query` is directly on `ForceClient` (imported via trait extension or inherent impl).

**The Fix:**
Update the example to:
- Provide `token_url`.
- Use `client.query::<Account>(soql)`.

## 2. Bulk Insert Example (`echo_bulk_insert.rs`)

**Status:** ❌ FAILED TO COMPILE

**The Confusion:**
I copied the "Bulk Insert" code. It failed with:
1. `ClientCredentials::new` expects 3 arguments.

**The Fix:**
- Provide `token_url` to `ClientCredentials::new`.

## 3. Bulk Query Example (`echo_bulk_query.rs`)

**Status:** ❌ FAILED TO COMPILE

**The Confusion:**
I copied the "Bulk Query" code. It failed with:
1. `ClientCredentials::new` expects 3 arguments.
2. `bulk_query_typed` method not found.
3. Type annotations needed for `stream`.

**The Reality:**
- `bulk_query_typed` does not exist. It seems it should be `bulk_query`.

**The Fix:**
- Provide `token_url`.
- Change `bulk_query_typed` to `bulk_query`.
- Ensure type inference works or provide explicit types.

## Recommendation

The documentation is significantly out of sync with the codebase.
- **IMMEDIATE ACTION:** Fix `README.md` examples.
- **SUGGESTION:** Add a CI step that extracts and tests `README.md` examples to prevent regression.
