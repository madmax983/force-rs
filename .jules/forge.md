**Unify Sorting Logic**
**Learning:** `schema` generator modules frequently use a standard way to sort fields so `Id` comes first. `type_generator.rs` and `protobuf_generator.rs` were doing this manually.
**Action:** Always use `crate::schema::cmp_field_names` for sorting `describe.fields`.

**Flatten Match in Tests**
**Learning:** Checking errors using `match` in tests can be deeply nested and messy.
**Action:** Use `let Err(e) = result else { panic!("Expected error") };` instead of `match` for concise test assertions.

**Unify Sorting Logic**
**Learning:** `schema` generator modules frequently use a standard way to sort fields so `Id` comes first. `type_generator.rs` and `protobuf_generator.rs` were doing this manually.
**Action:** Always use `crate::schema::cmp_field_names` for sorting `describe.fields`.

**Flatten Match in Tests**
**Learning:** Checking errors using `match` in tests can be deeply nested and messy.
**Action:** Use `let Err(e) = result else { panic!("Expected error") };` instead of `match` for concise test assertions.

**Unify Sorting Logic**
**Learning:** `schema` generator modules frequently use a standard way to sort fields so `Id` comes first. `type_generator.rs` and `protobuf_generator.rs` were doing this manually.
**Action:** Always use `crate::schema::cmp_field_names` for sorting `describe.fields`.

**Flatten Match in Tests**
**Learning:** Checking errors using `match` in tests can be deeply nested and messy.
**Action:** Use `let Err(e) = result else { panic!("Expected error") };` instead of `match` for concise test assertions.
